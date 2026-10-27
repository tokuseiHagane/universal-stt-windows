pub mod ffi;
pub mod json;

pub use ffi::{AsrEngine, ParakeetLib, VadEngine};
pub use json::AsrJson;

#[derive(Debug, thiserror::Error)]
pub enum AsrError {
    #[error("parakeet library: {0}")]
    Lib(String),
    #[error("parakeet model: {0}")]
    Model(String),
    #[error("invalid asr json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
