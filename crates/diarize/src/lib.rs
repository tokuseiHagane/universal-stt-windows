pub mod banks;
pub mod dft;
pub mod frontend;
pub mod logits;
pub mod meta;
pub mod session;

pub use banks::parse_banks;
pub use frontend::{Frontend, FrontendGeometry};
pub use logits::{WhoLogits, compute_logits, concat_mask_chunks};
pub use meta::Meta;
pub use session::{Sessions, Tensor};

#[derive(Debug, thiserror::Error)]
pub enum DiarizeError {
    #[error("invalid model data: {0}")]
    Model(String),
    #[error("onnx runtime: {0}")]
    Ort(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
