//! User-triggered, read-only artwork inventory. No game/runtime dependencies.
mod ifs;
mod shape;
#[cfg(test)]
mod tests;
mod xml;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
pub type Result<T> = std::result::Result<T, String>;
const SCHEMA: &str = "scanner-v2";
const MAX_INPUT: u64 = 256 * 1024 * 1024;

/// Search/display hints for already analysed names. These are not ABI/build guards.
pub fn mapping(archive: &str, texture: &str) -> &'static str {
    if archive == "@movie" {
        "游戏主视频 Gameplay movie"
    } else if archive == "@file" {
        "独立图片 Loose image"
    } else if texture.starts_with("@shape/") {
        "原生形状填充 Shape fill"
    } else if texture.starts_with("notes_") {
        "音符 Note skin"
    } else if matches!(
        texture,
        "center_frame_sp" | "center_frame_2pp" | "unit_body_base"
    ) {
        "游戏框体 Gameplay shell"
    } else if texture == "34decide_bg" {
        "决定画面 Decision"
    } else if texture.starts_with("34decide_zinrai_") {
        "过渡动画 Transition"
    } else if texture.starts_with("frame_chara_") {
        "选曲角色 Selection character"
    } else {
        "纹理区域 Texture region"
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub archive: String,
    pub texture: String,
    pub width: u32,
    pub height: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
}
#[derive(Serialize, Deserialize)]
struct Record {
    path: String,
    sha256: String,
    entries: Vec<Entry>,
    warnings: Vec<String>,
}
#[derive(Serialize)]
struct Report<'a> {
    schema: &'a str,
    source: String,
    files: usize,
    assets: usize,
    warnings: Vec<String>,
    inputs: Vec<(String, String)>,
}
pub enum Event {
    Progress(String),
    Finished(Result<PathBuf>),
}
fn error(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn check(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err("扫描已取消；保留原来的素材目录。".into())
    } else {
        Ok(())
    }
}
pub fn detect(config: &Path) -> Option<PathBuf> {
    config
        .ancestors()
        .skip(1)
        .take(3)
        .find(|p| p.join("data/graphic").is_dir())
        .map(Path::to_path_buf)
}
fn graphic_root(root: &Path) -> Result<PathBuf> {
    for p in [
        root.join("data/graphic"),
        root.join("graphic"),
        root.to_path_buf(),
    ] {
        if p.is_dir()
            && (p
                .file_name()
                .is_some_and(|n| n.eq_ignore_ascii_case("graphic"))
                || p.join("system").is_dir())
        {
            return p.canonicalize().map_err(error);
        }
    }
    Err("请选择 IIDX 安装目录（包含 data/graphic），或 graphic 目录。".into())
}
fn collect(root: &Path, out: &mut Vec<PathBuf>, cancel: &AtomicBool, depth: usize) -> Result<()> {
    check(cancel)?;
    if depth > 64 {
        return Err("directory nesting limit".into());
    }
    for e in fs::read_dir(root).map_err(error)? {
        check(cancel)?;
        let e = e.map_err(error)?;
        let ty = e.file_type().map_err(error)?;
        // Do not follow symlinks, junctions, or other reparse points out of the installation.
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if e.metadata().map_err(error)?.file_attributes() & 0x400 != 0 {
                continue;
            }
        }
        if ty.is_dir() {
            collect(&e.path(), out, cancel, depth + 1)?;
        } else if ty.is_file()
            && e.path().extension().is_some_and(|s| {
                matches!(
                    s.to_string_lossy().to_ascii_lowercase().as_str(),
                    "ifs" | "png" | "jpg" | "jpeg" | "bmp" | "dds"
                )
            })
        {
            out.push(e.path());
        }
        if out.len() > 100_000 {
            return Err("too many input files".into());
        }
    }
    Ok(())
}
fn thumbnail(image: image::RgbaImage, cache: &Path, key: &str) -> Result<String> {
    let name = format!("thumbnails/{key}.png");
    let path = cache.join(&name);
    if !path.exists() {
        let width = image.width().min(256);
        let height = image.height().min(256);
        let image = image::DynamicImage::ImageRgba8(image).thumbnail(width, height);
        image.save(&path).map_err(error)?;
    }
    Ok(name)
}
fn archive(
    bytes: &[u8],
    name: &str,
    cache: &Path,
    digest: &str,
    cancel: &AtomicBool,
) -> Result<Record> {
    let a = ifs::Archive::read(bytes)?;
    let mut out = Record {
        path: name.into(),
        sha256: digest.into(),
        entries: Vec::new(),
        warnings: a.warnings.clone(),
    };
    let maps: Vec<_> = a
        .names()
        .filter(|p| p.ends_with("texturelist.xml"))
        .cloned()
        .collect();
    for map in maps {
        let (root, encoding) = xml::parse(a.get(&map)?)?;
        let compressed = root.attrs.get("compress").is_some_and(|v| v == "avslz");
        for tex in root.children.iter().filter(|n| n.name == "texture") {
            let format = tex.attr("format")?;
            for item in tex.children.iter().filter(|n| n.name == "image") {
                check(cancel)?;
                let name = item.attr("name")?;
                let uv: Vec<_> = item
                    .child("uvrect")?
                    .ints()?
                    .iter()
                    .map(|n| n / 2)
                    .collect();
                let rect: Vec<_> = item
                    .child("imgrect")?
                    .ints()?
                    .iter()
                    .map(|n| n / 2)
                    .collect();
                if uv.len() != 4
                    || uv.iter().any(|v| !(-32768..=32768).contains(v))
                    || uv[1] <= uv[0]
                    || uv[3] <= uv[2]
                    || uv[1] - uv[0] > 16384
                    || uv[3] - uv[2] > 16384
                {
                    out.warnings.push(format!("{name}: invalid UV"));
                    continue;
                }
                let preview = a.texture(name, encoding).and_then(|data| {
                    let key = hash(
                        format!("{} {format} {compressed} {rect:?} {uv:?}", hash(data)).as_bytes(),
                    );
                    let path = format!("thumbnails/{key}.png");
                    if cache.join(&path).is_file() {
                        return Ok(path);
                    }
                    ifs::texture_image(data, format, compressed, &rect, &uv)
                        .and_then(|im| thumbnail(im, cache, &key))
                });
                let preview = match preview {
                    Ok(p) => Some(p),
                    Err(e) => {
                        out.warnings.push(format!("{name}: {e}"));
                        None
                    }
                };
                out.entries.push(Entry {
                    archive: out.path.clone(),
                    texture: name.into(),
                    width: (uv[1] - uv[0]) as u32,
                    height: (uv[3] - uv[2]) as u32,
                    preview,
                });
            }
        }
    }
    for name in a.names().filter(|p| p.starts_with("geo/")) {
        check(cancel)?;
        let result = (|| -> Result<Option<Entry>> {
            let bytes = a.get(name)?;
            let key = format!("shape-{}", hash(bytes));
            let render = !cache.join(format!("thumbnails/{key}.png")).is_file();
            let Some((width, height, image)) = shape::preview(bytes, render)? else {
                return Ok(None);
            };
            let preview = thumbnail(image, cache, &key)?;
            Ok(Some(Entry {
                archive: out.path.clone(),
                texture: format!("@shape/{}", name.trim_start_matches("geo/")),
                width,
                height,
                preview: Some(preview),
            }))
        })();
        match result {
            Ok(Some(e)) => out.entries.push(e),
            Ok(None) => (),
            Err(e) => out.warnings.push(format!("{name}: {e}")),
        }
    }
    Ok(out)
}
pub fn cache_root() -> Result<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA unavailable")?;
    Ok(PathBuf::from(local).join("iidxbg").join("asset-cache"))
}
pub fn run(root: &Path, cancel: &AtomicBool, progress: impl Fn(String)) -> Result<PathBuf> {
    scan(root, &cache_root()?, cancel, progress)
}
fn scan(
    root: &Path,
    base: &Path,
    cancel: &AtomicBool,
    progress: impl Fn(String),
) -> Result<PathBuf> {
    let source = graphic_root(root)?;
    // Resolve the existing ancestor before creating anything: even a rejected
    // cache path must not leave a new directory in the game installation.
    let mut ancestor = base;
    let mut missing = Vec::new();
    while !ancestor.exists() {
        missing.push(ancestor.file_name().ok_or("invalid cache path")?);
        ancestor = ancestor.parent().ok_or("invalid cache parent")?;
    }
    let mut destination = ancestor.canonicalize().map_err(error)?;
    for part in missing.into_iter().rev() {
        destination.push(part);
    }
    let protected = source
        .parent()
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.eq_ignore_ascii_case("data"))
        })
        .and_then(Path::parent)
        .unwrap_or(&source);
    if destination.starts_with(protected) {
        return Err("cache must be outside the game installation".into());
    }
    fs::create_dir_all(base).map_err(error)?;
    let base = base.canonicalize().map_err(error)?;
    if base.starts_with(protected) {
        return Err("cache must be outside game assets".into());
    }
    let cache = base
        .join(hash(source.to_string_lossy().as_bytes()))
        .join(SCHEMA);
    fs::create_dir_all(cache.join("thumbnails")).map_err(error)?;
    fs::create_dir_all(cache.join("records")).map_err(error)?;
    progress("正在查找素材文件…".into());
    let mut files = Vec::new();
    collect(&source, &mut files, cancel, 0)?;
    files.sort();
    if files.is_empty() {
        return Err("没有找到 IFS 或图片文件。".into());
    }
    let mut entries = Vec::new();
    let mut warnings = Vec::new();
    let mut inputs = Vec::new();
    for (index, path) in files.iter().enumerate() {
        check(cancel)?;
        let name = path
            .strip_prefix(&source)
            .map_err(error)?
            .to_string_lossy()
            .replace('\\', "/");
        progress(format!(
            "{}/{} · {} 项 · {}",
            index + 1,
            files.len(),
            entries.len(),
            name
        ));
        let result = (|| -> Result<Record> {
            use std::io::Read;
            let f = fs::File::open(path).map_err(error)?;
            if f.metadata().map_err(error)?.len() > MAX_INPUT {
                return Err("input exceeds 256 MiB limit".into());
            }
            let mut bytes = Vec::new();
            f.take(MAX_INPUT + 1)
                .read_to_end(&mut bytes)
                .map_err(error)?;
            if bytes.len() as u64 > MAX_INPUT {
                return Err("input exceeds 256 MiB limit".into());
            }
            let digest = hash(&bytes);
            inputs.push((name.clone(), digest.clone()));
            let key = hash(format!("{name}\0{digest}").as_bytes());
            let record_path = cache.join("records").join(format!("{key}.json"));
            if let Ok(raw) = fs::read(&record_path)
                && let Ok(record) = serde_json::from_slice::<Record>(&raw)
                && record.sha256 == digest
                && record.path == name
                && record
                    .entries
                    .iter()
                    .all(|e| e.preview.as_ref().is_none_or(|p| cache.join(p).is_file()))
            {
                return Ok(record);
            }
            let record = if path
                .extension()
                .is_some_and(|s| s.eq_ignore_ascii_case("ifs"))
            {
                archive(&bytes, &name, &cache, &digest, cancel)?
            } else {
                let mut reader = image::ImageReader::new(std::io::Cursor::new(&bytes))
                    .with_guessed_format()
                    .map_err(error)?;
                let mut limits = image::Limits::default();
                limits.max_image_width = Some(16384);
                limits.max_image_height = Some(16384);
                limits.max_alloc = Some(256 * 1024 * 1024);
                reader.limits(limits);
                let image = reader.decode().map_err(error)?.to_rgba8();
                let (width, height) = image.dimensions();
                let preview = thumbnail(image, &cache, &format!("loose-{digest}"))?;
                Record {
                    path: name.clone(),
                    sha256: digest,
                    entries: vec![Entry {
                        archive: "@file".into(),
                        texture: name.clone(),
                        width,
                        height,
                        preview: Some(preview),
                    }],
                    warnings: Vec::new(),
                }
            };
            atomic_json(&record_path, &record)?;
            Ok(record)
        })();
        match result {
            Ok(record) => {
                warnings.extend(record.warnings.into_iter().map(|w| format!("{name}: {w}")));
                entries.extend(record.entries);
            }
            Err(e) => warnings.push(format!("{name}: {e}")),
        }
        if entries.len() > 199_999 {
            return Err("catalog exceeds 200,000 entries".into());
        }
    }
    check(cancel)?;
    if entries.is_empty() {
        return Err(format!(
            "没有可读素材。{}",
            warnings.first().cloned().unwrap_or_default()
        ));
    }
    entries.push(Entry {
        archive: "@movie".into(),
        texture: "main".into(),
        width: 1920,
        height: 1080,
        preview: None,
    });
    entries.sort_by(|a, b| (&a.archive, &a.texture).cmp(&(&b.archive, &b.texture)));
    entries.dedup_by(|a, b| a.archive == b.archive && a.texture == b.texture);
    let count = warnings.len();
    atomic_json(
        &cache.join("scan-report.json"),
        &Report {
            schema: SCHEMA,
            source: source.to_string_lossy().into(),
            files: files.len(),
            assets: entries.len(),
            warnings,
            inputs,
        },
    )?;
    let catalog = cache.join("catalog.json");
    atomic_json(&catalog, &entries)?;
    progress(format!(
        "扫描完成：{} 项；{} 条提示。",
        entries.len(),
        count
    ));
    Ok(catalog)
}
fn atomic_json(path: &Path, value: &impl Serialize) -> Result<()> {
    use std::io::Write;
    let mut file =
        tempfile::NamedTempFile::new_in(path.parent().ok_or("cache path")?).map_err(error)?;
    serde_json::to_writer(file.as_file_mut(), value).map_err(error)?;
    file.flush().map_err(error)?;
    file.persist(path).map_err(error)?;
    Ok(())
}
