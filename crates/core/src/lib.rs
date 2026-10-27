pub mod color;
pub mod history;
pub mod model;
pub mod paths;
pub mod recent;
pub mod settings;
pub mod store;

pub use color::{SPEAKER_COLORS, color_for_index};
pub use history::History;
pub use model::{
    AudioFormat, Diarization, ExportDefaults, MediaKind, Project, Segment, SourceRef, Speaker,
    TextFormat, Turn, Word, speakers_from_diarization,
};
pub use paths::config_dir_from;
pub use recent::{RecentEntry, RecentStore};
pub use settings::{BackendPreference, Settings};
pub use store::{ProjectStore, StoreError, slugify};
