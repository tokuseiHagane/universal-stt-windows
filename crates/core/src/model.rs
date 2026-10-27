use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::color::SPEAKER_COLORS;

const EDGE_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    Audio,
    Video,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRef {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub modified_unix: i64,
    pub fingerprint: String,
}

impl SourceRef {
    pub fn new(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let meta = std::fs::metadata(&path)?;
        let size_bytes = meta.len();
        let modified_unix = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let mut file = std::fs::File::open(&path)?;
        let mut hasher = Sha256::new();
        if size_bytes <= EDGE_BYTES * 2 {
            let mut buf = Vec::with_capacity(size_bytes as usize);
            file.read_to_end(&mut buf)?;
            hasher.update(&buf);
        } else {
            let mut head = vec![0u8; EDGE_BYTES as usize];
            file.read_exact(&mut head)?;
            hasher.update(&head);
            file.seek(SeekFrom::Start(size_bytes - EDGE_BYTES))?;
            let mut tail = vec![0u8; EDGE_BYTES as usize];
            file.read_exact(&mut tail)?;
            hasher.update(&tail);
        }

        Ok(Self {
            path,
            size_bytes,
            modified_unix,
            fingerprint: format!("{:x}", hasher.finalize()),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Speaker {
    pub id: Uuid,
    pub name: String,
    pub color: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Word {
    pub start: f64,
    pub end: f64,
    pub text: String,
    #[serde(default)]
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Segment {
    pub id: Uuid,
    pub start: f64,
    pub end: f64,
    pub text: String,
    #[serde(default)]
    pub speaker_id: Option<Uuid>,
    #[serde(default)]
    pub words: Vec<Word>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Turn {
    pub start: f64,
    pub end: f64,
    pub speaker_index: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diarization {
    pub turns: Vec<Turn>,
    pub speaker_count: u32,
    pub duration: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TextFormat {
    #[default]
    Docx,
    Pdf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AudioFormat {
    #[default]
    Wav,
    Mp3,
    M4a,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ExportDefaults {
    pub text_format: TextFormat,
    pub audio_format: AudioFormat,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub schema: u32,
    pub id: Uuid,
    pub title: String,
    pub source: SourceRef,
    pub kind: MediaKind,
    pub duration: f64,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub speakers: Vec<Speaker>,
    #[serde(default)]
    pub segments: Vec<Segment>,
    #[serde(default)]
    pub turns: Vec<Turn>,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    #[serde(default)]
    pub export_defaults: ExportDefaults,
}

impl Project {
    pub const SCHEMA: u32 = 1;

    pub fn new(
        title: impl Into<String>,
        source: SourceRef,
        kind: MediaKind,
        duration: f64,
    ) -> Self {
        let now = Utc::now();
        Self {
            schema: Self::SCHEMA,
            id: Uuid::new_v4(),
            title: title.into(),
            source,
            kind,
            duration,
            language: None,
            speakers: Vec::new(),
            segments: Vec::new(),
            turns: Vec::new(),
            created_at: now,
            modified_at: now,
            export_defaults: ExportDefaults::default(),
        }
    }
}

pub fn speakers_from_diarization(diarization: &Diarization) -> Vec<Speaker> {
    (0..diarization.speaker_count)
        .map(|index| Speaker {
            id: Uuid::new_v4(),
            name: format!("Speaker {}", index + 1),
            color: (index as usize % SPEAKER_COLORS.len()) as u8,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::{SPEAKER_COLORS, color_for_index};

    fn sample_source() -> SourceRef {
        SourceRef {
            path: PathBuf::from("clip.mp4"),
            size_bytes: 2048,
            modified_unix: 1_700_000_000,
            fingerprint: "abc".into(),
        }
    }

    #[test]
    fn project_json_roundtrip() {
        let mut p = Project::new("Talk", sample_source(), MediaKind::Video, 12.5);
        p.speakers = speakers_from_diarization(&Diarization {
            turns: vec![Turn {
                start: 0.0,
                end: 2.0,
                speaker_index: 1,
            }],
            speaker_count: 2,
            duration: 12.5,
        });
        p.segments.push(Segment {
            id: Uuid::new_v4(),
            start: 0.0,
            end: 2.0,
            text: "hello".into(),
            speaker_id: Some(p.speakers[1].id),
            words: vec![Word {
                start: 0.0,
                end: 0.4,
                text: "hello".into(),
                confidence: Some(0.9),
            }],
        });
        let json = serde_json::to_string(&p).unwrap();
        let back: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
        assert_eq!(back.schema, 1);
    }

    #[test]
    fn speakers_from_diarization_orders_by_first_appearance() {
        let d = Diarization {
            turns: vec![
                Turn {
                    start: 5.0,
                    end: 6.0,
                    speaker_index: 3,
                },
                Turn {
                    start: 1.0,
                    end: 2.0,
                    speaker_index: 3,
                },
                Turn {
                    start: 2.5,
                    end: 3.0,
                    speaker_index: 0,
                },
            ],
            speaker_count: 4,
            duration: 10.0,
        };
        let s = speakers_from_diarization(&d);
        assert_eq!(s.len(), 4);
        assert_eq!(s[0].name, "Speaker 1");
        assert_eq!(s[0].color, 0);
        assert_eq!(s[3].color, 3);
    }

    #[test]
    fn speaker_colors_cycle_after_eight() {
        assert_eq!(SPEAKER_COLORS.len(), 8);
        assert_eq!(color_for_index(8), SPEAKER_COLORS[0]);
    }
}
