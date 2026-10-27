pub mod banks;
pub mod dft;
pub mod frontend;

pub use banks::parse_banks;
pub use frontend::{Frontend, FrontendGeometry};

#[derive(Debug, thiserror::Error)]
pub enum DiarizeError {
    #[error("invalid model data: {0}")]
    Model(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
