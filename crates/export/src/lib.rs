#[cfg(feature = "audio")]
pub mod audio;
#[cfg(feature = "office")]
pub mod docx;
pub mod lines;
pub mod model;
#[cfg(feature = "office")]
pub mod pdf;
pub mod text;

#[cfg(feature = "audio")]
pub use audio::{AudioPlan, AudioTrack, extract, plan_segments, plan_speakers};
#[cfg(feature = "office")]
pub use docx::render_docx;
pub use lines::lines;
pub use model::AudioMode;
pub use model::{ExportError, ExportLine, TextOutput};
#[cfg(feature = "office")]
pub use pdf::render_pdf;
pub use text::render_text;
