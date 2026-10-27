use crate::AsrJson;
use crate::vad::TimeRange;
use ustt_core::Word;

pub fn stitch(chunks: &[(TimeRange, AsrJson)]) -> AsrJson {
    let mut text_parts = Vec::new();
    let mut words = Vec::new();
    let mut language = None;
    for (range, chunk) in chunks {
        if !chunk.text.trim().is_empty() {
            text_parts.push(chunk.text.trim().to_string());
        }
        if language.is_none() {
            language = chunk.language.clone();
        }
        words.extend(chunk.words.iter().map(|word| Word {
            start: word.start + range.start,
            end: word.end + range.start,
            text: word.text.clone(),
            confidence: word.confidence,
        }));
    }
    AsrJson {
        text: text_parts.join(" "),
        language,
        words,
    }
}
