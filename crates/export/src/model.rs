use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextOutput {
    Docx,
    Pdf,
    Srt,
    Vtt,
    Txt,
    Markdown,
}

impl TextOutput {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Docx => "docx",
            Self::Pdf => "pdf",
            Self::Srt => "srt",
            Self::Vtt => "vtt",
            Self::Txt => "txt",
            Self::Markdown => "md",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "docx" => Some(Self::Docx),
            "pdf" => Some(Self::Pdf),
            "srt" => Some(Self::Srt),
            "vtt" => Some(Self::Vtt),
            "txt" => Some(Self::Txt),
            "md" | "markdown" => Some(Self::Markdown),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioMode {
    None,
    Speakers,
    Segments,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExportLine {
    pub start: f64,
    pub end: f64,
    pub speaker: String,
    pub text: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("docx error: {0}")]
    Docx(String),
    #[error("pdf error: {0}")]
    Pdf(String),
    #[error("audio error: {0}")]
    Audio(String),
    #[error("project media is missing")]
    MissingMedia,
    #[error("cancelled")]
    Cancelled,
}
