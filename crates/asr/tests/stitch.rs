use ustt_asr::{AsrJson, TimeRange, stitch};
use ustt_core::Word;

#[test]
fn stitch_offsets_words_and_joins_text() {
    let a = AsrJson {
        text: "hello".into(),
        language: None,
        words: vec![Word {
            start: 0.2,
            end: 0.4,
            text: "hello".into(),
            confidence: None,
        }],
    };
    let b = AsrJson {
        text: "world".into(),
        language: Some("en".into()),
        words: vec![Word {
            start: 0.1,
            end: 0.3,
            text: "world".into(),
            confidence: None,
        }],
    };
    let out = stitch(&[
        (
            TimeRange {
                start: 10.0,
                end: 12.0,
            },
            a,
        ),
        (
            TimeRange {
                start: 20.0,
                end: 22.0,
            },
            b,
        ),
    ]);
    assert_eq!(out.text, "hello world");
    assert_eq!(out.words[0].start, 10.2);
    assert_eq!(out.words[1].start, 20.1);
    assert_eq!(out.language.as_deref(), Some("en"));
}
