use ustt_asr::AsrJson;

#[test]
fn parses_words_with_offsets() {
    let j: AsrJson = serde_json::from_str(
        r#"{"text":"hi there","frame_sec":0.08,
        "words":[{"w":"hi","start":0.48,"end":0.64,"conf":0.9},
                 {"w":"there","start":0.64,"end":0.96,"conf":0.8}],
        "tokens":[]}"#,
    )
    .unwrap();
    assert_eq!(j.words.len(), 2);
    assert_eq!(j.words[0].text, "hi");
    assert_eq!(j.words[1].start, 0.64);
    assert_eq!(j.language, None);
    assert_eq!(j.words[0].confidence, Some(0.9));
}

#[test]
fn empty_document_is_valid() {
    let j: AsrJson = serde_json::from_str(r#"{"text":"","words":[],"tokens":[]}"#).unwrap();
    assert!(j.words.is_empty());
    assert_eq!(j.text, "");
}
