//! Read-only Konami binary XML, with bounded node/data cursors.
use super::Result;
use std::collections::BTreeMap;

#[derive(Debug, Default)]
pub struct Node {
    pub name: String,
    pub text: String,
    pub attrs: BTreeMap<String, String>,
    pub children: Vec<Node>,
}
impl Node {
    pub fn child(&self, name: &str) -> Result<&Node> {
        self.children
            .iter()
            .find(|n| n.name == name)
            .ok_or_else(|| format!("missing {name}"))
    }
    pub fn attr(&self, name: &str) -> Result<&str> {
        self.attrs
            .get(name)
            .map(String::as_str)
            .ok_or_else(|| format!("missing attribute {name}"))
    }
    pub fn ints(&self) -> Result<Vec<i64>> {
        self.text
            .split_whitespace()
            .map(|v| v.parse().map_err(|_| "invalid integer".into()))
            .collect()
    }
}
pub fn slice(b: &[u8], offset: usize, len: usize) -> Result<&[u8]> {
    b.get(offset..offset.checked_add(len).ok_or("offset overflow")?)
        .ok_or_else(|| "truncated data".into())
}
pub fn u32be(b: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_be_bytes(slice(b, offset, 4)?.try_into().unwrap()))
}
struct Cursor<'a> {
    b: &'a [u8],
    pos: usize,
}
impl<'a> Cursor<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let v = slice(self.b, self.pos, len)?;
        self.pos += len;
        Ok(v)
    }
    fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn auto(&mut self) -> Result<&'a [u8]> {
        let len = u32::from_be_bytes(self.take(4)?.try_into().unwrap()) as usize;
        let v = self.take(len)?;
        self.pos = self.pos.next_multiple_of(4);
        Ok(v)
    }
}
pub fn decode(b: &[u8], encoding: u8) -> Result<String> {
    let codec = match encoding {
        0 | 0x80 => encoding_rs::SHIFT_JIS,
        0x60 => encoding_rs::EUC_JP,
        0xa0 | 0x20 => encoding_rs::UTF_8,
        0x40 => return Ok(b.iter().map(|v| char::from(*v)).collect()),
        _ => return Err("unknown XML encoding".into()),
    };
    codec
        .decode_without_bom_handling_and_without_replacement(b)
        .map(|s| s.into_owned())
        .ok_or_else(|| "invalid XML string encoding".into())
}
pub fn encode(s: &str, encoding: u8) -> Result<Vec<u8>> {
    let codec = match encoding {
        0 | 0x80 => encoding_rs::SHIFT_JIS,
        0x60 => encoding_rs::EUC_JP,
        _ => encoding_rs::UTF_8,
    };
    let (bytes, _, errors) = codec.encode(s);
    if errors {
        Err("unencodable asset name".into())
    } else {
        Ok(bytes.into_owned())
    }
}
pub fn parse(b: &[u8]) -> Result<(Node, u8)> {
    match parse_inner(b) {
        Err(e)
            if e == "invalid XML string encoding"
                && b.first() == Some(&0xa0)
                && matches!(b.get(2), Some(0 | 0x80)) =>
        {
            // Some shipped metadata declares Shift-JIS but contains UTF-8.
            // Match ifstools' recovery, including UTF-8 for MD5 name lookup.
            let mut utf8 = b.to_vec();
            utf8[2] = 0xa0;
            utf8[3] = 0x5f;
            parse_inner(&utf8)
        }
        result => result,
    }
}
fn parse_inner(b: &[u8]) -> Result<(Node, u8)> {
    if !b.starts_with(&[0xa0]) {
        let text = std::str::from_utf8(b)
            .map_err(|e| e.to_string())?
            .trim_end_matches('\0');
        let doc = roxmltree::Document::parse(text).map_err(|e| e.to_string())?;
        fn convert(n: roxmltree::Node<'_, '_>, depth: usize) -> Result<Node> {
            if depth > 128 {
                return Err("XML nesting limit".into());
            }
            Ok(Node {
                name: n.tag_name().name().into(),
                text: n.text().unwrap_or("").into(),
                attrs: n
                    .attributes()
                    .map(|a| (a.name().into(), a.value().into()))
                    .collect(),
                children: n
                    .children()
                    .filter(|n| n.is_element())
                    .map(|n| convert(n, depth + 1))
                    .collect::<Result<_>>()?,
            })
        }
        return Ok((convert(doc.root_element(), 0)?, 0xa0));
    }
    let header = slice(b, 0, 8)?;
    if !matches!(header[1], 0x42 | 0x45) || header[2] ^ header[3] != 255 {
        return Err("invalid binary XML header".into());
    }
    let end = 8 + u32be(b, 4)? as usize;
    let mut nodes = Cursor {
        b: slice(b, 8, end - 8)?,
        pos: 0,
    };
    let mut data = Cursor {
        b: slice(b, end + 4, u32be(b, end)? as usize)?,
        pos: 0,
    };
    let mut byte_pos: usize = 0;
    let mut word_pos: usize = 0;
    let mut stack = vec![Node::default()];
    let mut count = 0;
    loop {
        let raw = nodes.byte()?;
        if raw == 0 {
            continue;
        }
        let id = raw & !64;
        if id == 191 {
            break;
        }
        if id == 190 {
            if stack.len() <= 1 {
                return Err("unbalanced XML".into());
            }
            let n = stack.pop().unwrap();
            stack.last_mut().unwrap().children.push(n);
            continue;
        }
        let len = nodes.byte()? as usize;
        let name = if header[1] == 0x42 {
            const CHARS: &[u8] =
                b"0123456789:ABCDEFGHIJKLMNOPQRSTUVWXYZ_abcdefghijklmnopqrstuvwxyz";
            let bytes = nodes.take((len * 6).div_ceil(8))?;
            (0..len)
                .map(|i| {
                    let mut v = 0;
                    for bit in i * 6..i * 6 + 6 {
                        v = (v << 1) | ((bytes[bit / 8] >> (7 - bit % 8)) & 1);
                    }
                    CHARS[v as usize] as char
                })
                .collect()
        } else {
            decode(nodes.take((len & !64) + 1)?, header[2])?
        };
        if id == 46 {
            let text = decode(data.auto()?, header[2])?
                .trim_end_matches('\0')
                .to_owned();
            stack.last_mut().unwrap().attrs.insert(name, text);
            continue;
        }
        count += 1;
        if count > 500_000 || stack.len() > 128 {
            return Err("XML node limit".into());
        }
        let mut node = Node {
            name,
            ..Node::default()
        };
        if id != 1 {
            let (size, components, kind) = match id {
                2..=9 => ([1, 1, 2, 2, 4, 4, 8, 8][(id - 2) as usize], 1, id),
                10 | 11 => (1, 1, id),
                12 | 13 => (4, 1, 7),
                14 => (4, 1, 14),
                15 => (8, 1, 15),
                16..=45 => {
                    let k = (id - 16) % 10;
                    (
                        [1, 1, 2, 2, 4, 4, 8, 8, 4, 8][k as usize],
                        (id - 16) as usize / 10 + 2,
                        if k < 8 { k + 2 } else { k + 6 },
                    )
                }
                48 | 49 => (1, 16, 3),
                50 | 51 => (2, 8, 5),
                52..=56 => (1, [1, 2, 3, 4, 16][(id - 52) as usize], 3),
                _ => return Err(format!("unsupported XML type {id}")),
            };
            let bytes = if raw & 64 != 0 || matches!(id, 10 | 11) {
                data.auto()?
            } else {
                if byte_pos.is_multiple_of(4) {
                    byte_pos = data.pos;
                }
                if word_pos.is_multiple_of(4) {
                    word_pos = data.pos;
                }
                let len = size * components;
                let v = match len {
                    1 => {
                        let v = slice(data.b, byte_pos, 1)?;
                        byte_pos += 1;
                        v
                    }
                    2 => {
                        let v = slice(data.b, word_pos, 2)?;
                        word_pos += 2;
                        v
                    }
                    _ => {
                        let v = data.take(len)?;
                        data.pos = data.pos.next_multiple_of(4);
                        v
                    }
                };
                data.pos = data.pos.max(byte_pos.max(word_pos).next_multiple_of(4));
                v
            };
            node.text = if id == 11 {
                decode(bytes, header[2])?.trim_end_matches('\0').into()
            } else if id == 10 {
                bytes.iter().map(|v| format!("{v:02x}")).collect()
            } else {
                if bytes.len() % size != 0 {
                    return Err("invalid XML array size".into());
                }
                bytes
                    .chunks_exact(size)
                    .map(|b| {
                        let value = b.iter().fold(0u64, |v, b| (v << 8) | u64::from(*b));
                        match kind {
                            2 | 4 | 6 | 8 => {
                                (((value << (64 - size * 8)) as i64) >> (64 - size * 8)).to_string()
                            }
                            14 => f32::from_bits(value as u32).to_string(),
                            15 => f64::from_bits(value).to_string(),
                            _ => value.to_string(),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            };
        }
        stack.push(node);
    }
    if stack.len() != 1 || stack[0].children.len() != 1 {
        return Err("unbalanced binary XML".into());
    }
    Ok((stack.pop().unwrap().children.remove(0), header[2]))
}
