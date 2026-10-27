use crate::{DiarizeError, FrontendGeometry};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Meta {
    pub schema: u32,
    pub model: ModelInfo,
    pub audio: AudioInfo,
    pub frontend: FrontendInfo,
    pub frames: FramesInfo,
    pub io_dtype: String,
    pub compute_units: String,
    pub windows: Vec<Window>,
    pub pieces: Pieces,
    pub decision: Decision,
    pub voice: Voice,
    pub join: Join,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct ModelInfo {
    pub id: String,
    pub architecture: String,
    pub dim: usize,
    pub queries: usize,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct AudioInfo {
    pub sample_rate: u32,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct FrontendInfo {
    pub frame_samples: usize,
    pub shift_samples: usize,
    pub n_fft: usize,
    pub n_mels: usize,
    pub low_hz: f64,
    pub preemphasis: f32,
    pub scale: f32,
    pub window: String,
    pub log_floor: f32,
    pub mean: String,
    pub banks: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct FramesInfo {
    pub mel_frames_per_frame: usize,
    pub step_s: f64,
    pub downsample: usize,
    pub first_centre_s: f64,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Graph {
    pub compiled: String,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Window {
    pub seconds: f64,
    pub frames: usize,
    pub low: usize,
    pub chunks: Vec<usize>,
    pub encoder: Graph,
    pub queries: Graph,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Pieces {
    pub cut: String,
    pub pad: String,
    pub queries: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Decision {
    pub presence_threshold: f32,
    pub mask_threshold: f32,
    pub step_s: f64,
    pub rule: String,
    pub pool: usize,
    pub note: String,
    pub time: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Voice {
    pub compiled: String,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub mel_frames: usize,
    pub margin_mel_frames: usize,
    pub mel_frames_per_column: usize,
    pub columns_dim: usize,
    pub frontend: FrontendInfo,
    pub embedding: Embedding,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Embedding {
    pub weights: String,
    pub dim: usize,
    pub pooling: String,
    pub format: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Join {
    pub kind: String,
    pub linkage: String,
    pub metric: String,
    pub cosine: f32,
    pub min_solo_s: f64,
    pub solo: String,
    pub voiceless: String,
    pub no_voice: String,
    pub numbering: String,
}

impl Meta {
    pub fn load(dir: impl AsRef<Path>) -> Result<Self, DiarizeError> {
        let path = dir.as_ref().join("who_meta.json");
        let text = std::fs::read_to_string(&path)?;
        let meta: Self = serde_json::from_str(&text)
            .map_err(|e| DiarizeError::Model(format!("{}: {e}", path.display())))?;
        Ok(meta)
    }

    pub fn validate(&self) -> Result<(), DiarizeError> {
        if self.schema != 2 {
            return Err(DiarizeError::Model(format!(
                "unsupported who_meta schema {}",
                self.schema
            )));
        }
        if self.model.architecture != "eend_m2f" {
            return Err(DiarizeError::Model(format!(
                "unsupported architecture {}",
                self.model.architecture
            )));
        }
        if self.model.queries == 0 || self.model.dim == 0 {
            return Err(DiarizeError::Model("model dim/queries are zero".into()));
        }
        if self.audio.sample_rate == 0
            || self.frontend.frame_samples == 0
            || self.frontend.shift_samples == 0
            || !self.frontend.n_fft.is_power_of_two()
            || self.frontend.n_mels == 0
        {
            return Err(DiarizeError::Model(
                "invalid audio/frontend geometry".into(),
            ));
        }
        if self.windows.is_empty() {
            return Err(DiarizeError::Model("no windows in who_meta".into()));
        }
        let downsample = self.frames.downsample;
        for window in &self.windows {
            if window.frames == 0 || window.low != window.frames / downsample {
                return Err(DiarizeError::Model(format!(
                    "window {} s: frames {} / low {} do not match downsample {}",
                    window.seconds, window.frames, window.low, downsample
                )));
            }
            if window.chunks.is_empty() {
                return Err(DiarizeError::Model(format!(
                    "window {} s has no chunks",
                    window.seconds
                )));
            }
            if window.chunks.iter().sum::<usize>() != window.frames {
                return Err(DiarizeError::Model(format!(
                    "window {} s: chunks do not cover {} frames",
                    window.seconds, window.frames
                )));
            }
            if window.chunks.len() + 1 != window.queries.inputs.len() {
                return Err(DiarizeError::Model(format!(
                    "window {} s: {} chunks need {} query inputs",
                    window.seconds,
                    window.chunks.len(),
                    window.chunks.len() + 1
                )));
            }
            if window.encoder.inputs.len() != 1 || window.encoder.outputs.len() != 2 {
                return Err(DiarizeError::Model(format!(
                    "window {} s: encoder signature is not mel -> (memory, frames)",
                    window.seconds
                )));
            }
            let expected = window.chunks.len() + 2;
            if window.queries.outputs.len() != expected {
                return Err(DiarizeError::Model(format!(
                    "window {} s: queries must return presence, queries and {} masks, not {:?}",
                    window.seconds,
                    window.chunks.len(),
                    window.queries.outputs
                )));
            }
        }
        if self.voice.mel_frames == 0
            || self.voice.mel_frames_per_column == 0
            || !self
                .voice
                .mel_frames
                .is_multiple_of(self.voice.mel_frames_per_column)
        {
            return Err(DiarizeError::Model("invalid voice window".into()));
        }
        Ok(())
    }

    pub fn geometry(&self) -> FrontendGeometry {
        FrontendGeometry {
            sample_rate: self.audio.sample_rate,
            frame: self.frontend.frame_samples,
            shift: self.frontend.shift_samples,
            n_fft: self.frontend.n_fft,
            mels: self.frontend.n_mels,
            preemphasis: self.frontend.preemphasis,
            scale: self.frontend.scale,
            log_floor: self.frontend.log_floor,
        }
    }

    pub fn window_for(&self, frames: usize) -> &Window {
        self.windows
            .iter()
            .find(|w| w.frames >= frames)
            .unwrap_or_else(|| self.windows.last().expect("validated non-empty windows"))
    }
}
