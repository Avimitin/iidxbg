//! Read-only IFS spans and the texture/AFP name tables; never extracts files.
use super::{
    Result,
    xml::{self, Node, slice, u32be},
};
use std::collections::BTreeMap;

pub struct Archive<'a> {
    data: &'a [u8],
    files: BTreeMap<String, (usize, usize)>,
    pub warnings: Vec<String>,
}
fn unescape(name: &str) -> Result<String> {
    let mut out = String::new();
    let mut chars = name.chars();
    while let Some(c) = chars.next() {
        if c != '_' {
            out.push(c);
            continue;
        }
        out.push(match chars.next().ok_or("invalid IFS name")? {
            'A' => ' ',
            'B' => '$',
            'C' => '+',
            'D' => '-',
            'E' => '.',
            'F' => ':',
            'G' => '@',
            'H' => '~',
            '_' => '_',
            c if out.is_empty() && c.is_ascii_digit() => c,
            _ => return Err("invalid IFS name escape".into()),
        });
    }
    Ok(out)
}
impl<'a> Archive<'a> {
    pub fn read(bytes: &'a [u8]) -> Result<Self> {
        if u32be(bytes, 0)? != 0x6cad8f89 {
            return Err("not an IFS archive".into());
        }
        let flags = u16::from_be_bytes(slice(bytes, 4, 2)?.try_into().unwrap());
        if flags ^ u16::from_be_bytes(slice(bytes, 6, 2)?.try_into().unwrap()) != 65535 {
            return Err("IFS flag checksum".into());
        }
        let start = if flags & 2 != 0 { 36 } else { 20 };
        let end = u32be(bytes, 16)? as usize;
        let (root, _) = xml::parse(slice(
            bytes,
            start,
            end.checked_sub(start).ok_or("IFS manifest range")?,
        )?)?;
        let mut a = Self {
            data: slice(
                bytes,
                end,
                bytes.len().checked_sub(end).ok_or("IFS data range")?,
            )?,
            files: BTreeMap::new(),
            warnings: Vec::new(),
        };
        a.walk(&root, "")?;
        // AFP's geo list provides the actual native shape name, not its MD5 filename.
        let maps: Vec<_> = a
            .files
            .keys()
            .filter(|n| n.starts_with("afp/") && n.ends_with(".xml"))
            .cloned()
            .collect();
        for name in maps {
            let (root, encoding) = xml::parse(a.get(&name)?)?;
            a.map_geometry(&root, encoding)?;
        }
        Ok(a)
    }
    fn map_geometry(&mut self, n: &Node, encoding: u8) -> Result<()> {
        if n.name == "afp"
            && let Some(name) = n.attrs.get("name")
        {
            for geo in n.children.iter().filter(|n| n.name == "geo") {
                for id in geo.ints()? {
                    let plain = format!("{name}_shape{id}");
                    let hashed = format!("geo/{:x}", md5::compute(xml::encode(&plain, encoding)?));
                    if let Some(span) = self.files.remove(&hashed) {
                        self.files.insert(format!("geo/{plain}"), span);
                    }
                }
            }
        }
        for child in &n.children {
            self.map_geometry(child, encoding)?;
        }
        Ok(())
    }
    fn walk(&mut self, root: &Node, prefix: &str) -> Result<()> {
        for n in &root.children {
            if n.name == "_info_" {
                continue;
            }
            if n.name == "_super_" || n.children.iter().any(|c| c.name == "i") {
                self.warnings.push(format!(
                    "external archive reference: {prefix}/{} ({})",
                    n.name, n.text
                ));
                continue;
            }
            let name = unescape(&n.name)?;
            let path = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            let words: Vec<_> = n.text.split_whitespace().collect();
            if n.children.is_empty() && matches!(words.len(), 2 | 3) {
                let offset = words[0]
                    .parse::<usize>()
                    .map_err(|_| "invalid IFS offset")?;
                let len = words[1].parse::<usize>().map_err(|_| "invalid IFS size")?;
                slice(self.data, offset, len)?;
                self.files.insert(path, (offset, len));
            } else {
                self.walk(n, &path)?;
            }
        }
        Ok(())
    }
    pub fn get(&self, name: &str) -> Result<&'a [u8]> {
        let &(offset, len) = self
            .files
            .get(name)
            .ok_or_else(|| format!("missing archive member {name}"))?;
        slice(self.data, offset, len)
    }
    pub fn names(&self) -> impl Iterator<Item = &String> {
        self.files.keys()
    }
    pub fn texture(&self, name: &str, encoding: u8) -> Result<&'a [u8]> {
        let hashed = format!("tex/{:x}", md5::compute(xml::encode(name, encoding)?));
        self.get(&hashed)
            .or_else(|_| self.get(&format!("tex/{name}.png")))
    }
}

pub fn decompress(bytes: &[u8], expected: usize) -> Result<Vec<u8>> {
    if expected > 256 * 1024 * 1024 {
        return Err("texture decompression limit".into());
    }
    let mut out = Vec::with_capacity(expected);
    let mut pos = 0;
    loop {
        let flags = slice(bytes, pos, 1)?[0];
        pos += 1;
        for bit in 0..8 {
            if flags & (1 << bit) != 0 {
                if out.len() >= expected {
                    return Err("LZ77 output exceeds declared size".into());
                }
                out.push(slice(bytes, pos, 1)?[0]);
                pos += 1;
            } else {
                let code = u16::from_be_bytes(slice(bytes, pos, 2)?.try_into().unwrap());
                pos += 2;
                let distance = (code >> 4) as usize;
                if distance == 0 {
                    return if out.len() == expected {
                        Ok(out)
                    } else {
                        Err("LZ77 output length mismatch".into())
                    };
                }
                let len = (code & 15) as usize + 3;
                if out.len() + len > expected {
                    return Err("LZ77 output exceeds declared size".into());
                }
                for _ in 0..len {
                    out.push(if distance > out.len() {
                        0
                    } else {
                        out[out.len() - distance]
                    });
                }
            }
        }
    }
}
pub fn texture_image(
    bytes: &[u8],
    format: &str,
    compressed: bool,
    rect: &[i64],
    uv: &[i64],
) -> Result<image::RgbaImage> {
    if rect.len() != 4
        || uv.len() != 4
        || rect.iter().chain(uv).any(|v| !(-32768..=32768).contains(v))
    {
        return Err("invalid texture rectangle".into());
    }
    let width = rect[1] - rect[0];
    let height = rect[3] - rect[2];
    if width <= 0
        || height <= 0
        || width > 16384
        || height > 16384
        || width * height > 64 * 1024 * 1024
    {
        return Err("texture dimension limit".into());
    }
    let owned;
    let data = if compressed {
        let expected = u32be(bytes, 0)? as usize;
        let packed = u32be(bytes, 4)? as usize;
        owned = if packed.checked_add(8) == Some(bytes.len()) {
            decompress(&bytes[8..], expected)?
        } else {
            [
                slice(
                    bytes,
                    8,
                    bytes.len().checked_sub(8).ok_or("texture header")?,
                )?,
                &bytes[..8],
            ]
            .concat()
        };
        &owned
    } else {
        bytes
    };
    let count = (width * height) as usize;
    let mut pixels = vec![0u8; count * 4];
    match format {
        "argb8888rev" => {
            let src = slice(data, 0, count * 4)?;
            for (s, d) in src
                .as_chunks::<4>()
                .0
                .iter()
                .zip(pixels.as_chunks_mut::<4>().0.iter_mut())
            {
                d.copy_from_slice(&[s[2], s[1], s[0], s[3]]);
            }
        }
        "argb4444" => {
            let src = slice(data, 0, count * 2)?;
            for (s, d) in src
                .as_chunks::<2>()
                .0
                .iter()
                .zip(pixels.as_chunks_mut::<4>().0.iter_mut())
            {
                d.copy_from_slice(&[
                    (s[1] & 15) * 17,
                    (s[0] >> 4) * 17,
                    (s[0] & 15) * 17,
                    (s[1] >> 4) * 17,
                ]);
            }
        }
        "dxt1" | "dxt5" => {
            let f = if format == "dxt1" {
                texpresso::Format::Bc1
            } else {
                texpresso::Format::Bc3
            };
            let need = (width as usize).div_ceil(4)
                * (height as usize).div_ceil(4)
                * if format == "dxt1" { 8 } else { 16 };
            let mut canonical = slice(data, 0, need)?.to_vec();
            // IFS textures store DXT words in big endian; DDS uses little endian.
            for word in canonical.as_chunks_mut::<2>().0 {
                word.swap(0, 1);
            }
            f.decompress(&canonical, width as usize, height as usize, &mut pixels);
        }
        _ => return Err(format!("unsupported texture format {format}")),
    }
    let x = uv[0] - rect[0];
    let y = uv[2] - rect[2];
    let w = uv[1] - uv[0];
    let h = uv[3] - uv[2];
    if x < 0 || y < 0 || w <= 0 || h <= 0 || x + w > width || y + h > height {
        return Err("UV outside texture".into());
    }
    let image =
        image::RgbaImage::from_raw(width as u32, height as u32, pixels).ok_or("invalid pixels")?;
    Ok(image::imageops::crop_imm(&image, x as u32, y as u32, w as u32, h as u32).to_image())
}
