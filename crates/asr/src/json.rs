use serde::{Deserialize, Serialize};
use ustt_core::Word;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "RawDoc")]
pub struct AsrJson {
    pub text: String,
    pub language: Option<String>,
    pub words: Vec<Word>,
}

#[derive(Debug, Deserialize)]
struct RawDoc {
    #[serde(default)]
    text: String,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    words: Vec<RawWord>,
}

#[derive(Debug, Deserialize)]
struct RawWord {
    #[serde(rename = "w")]
    text: String,
    start: f64,
    end: f64,
    #[serde(default)]
    conf: Option<f32>,
}

impl From<RawDoc> for AsrJson {
    fn from(doc: RawDoc) -> Self {
        Self {
            text: doc.text,
            language: doc.language,
            words: doc
                .words
                .into_iter()
                .map(|word| Word {
                    start: word.start,
                    end: word.end,
                    text: word.text,
                    confidence: word.conf,
                })
                .collect(),
        }
    }
}
