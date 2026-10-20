pub mod character;
pub mod config;
pub mod hook_chain;
pub mod pixels;
pub mod profile;
pub mod source;
pub mod state;

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
}

pub type Result<T> = std::result::Result<T, Error>;
