pub mod bilibili;
pub mod character;
pub mod config;
pub mod hook_chain;
pub mod media;
pub mod mmd;
pub mod pixels;
mod presets;
pub mod profile;
pub mod source;
pub mod state;
pub mod video;

#[cfg(all(windows, target_arch = "x86_64"))]
mod runtime;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("TOML configuration: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("HTTP: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Image: {0}")]
    Image(#[from] image::ImageError),
    #[error("Invalid configuration: {0}")]
    Config(&'static str),
    #[error("Input exceeds configured size or dimension limit")]
    Limit,
    #[error("No usable images found")]
    NoImages,
    #[error("Unsupported image format (only PNG and JPEG)")]
    Format,
    #[error("Provider returned an unsupported image URL")]
    Url,
    #[error("Unsupported or malformed game DLL: {0}")]
    Build(&'static str),
    #[error(
        "Live game code differs at {function} (RVA {rva:#x}): expected {expected}, actual {actual}"
    )]
    EntryMismatch {
        function: String,
        rva: usize,
        expected: String,
        actual: String,
    },
    #[error(
        "Live game vtable differs at RVA {rva:#x}, slot {slot}: expected {expected:#x}, actual {actual:#x}"
    )]
    TableMismatch {
        rva: usize,
        slot: usize,
        expected: usize,
        actual: usize,
    },
    #[error("Native/graphics operation failed: {0}")]
    Native(&'static str),
    #[error("Video: {0}")]
    Video(String),
    #[error("MMD: {0}")]
    Mmd(String),
    #[error("Bilibili: {0}")]
    Bilibili(String),
}

pub type Result<T> = std::result::Result<T, Error>;
#[cfg(windows)]
#[path = "runtime/artwork_shader.rs"]
mod artwork_shader;
