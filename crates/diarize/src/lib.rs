pub mod banks;
pub mod cluster;
pub mod decision;
pub mod dft;
pub mod embedding;
pub mod frontend;
pub mod linker;
pub mod logits;
pub mod meta;
pub mod model;
pub mod pieces;
pub mod session;
pub mod voices;

pub use banks::parse_banks;
pub use decision::{Decision, FrameClock, Rule, Stretch};
pub use embedding::Embedding;
pub use frontend::{Frontend, FrontendGeometry};
pub use linker::LabelledTurn;
pub use logits::{WhoLogits, compute_logits, concat_mask_chunks};
pub use meta::Meta;
pub use model::WhoModel;
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
