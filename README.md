# Universal STT

Local, offline speech transcription and speaker diarization for Windows.
Drop in an audio or video file, get a speaker-labelled transcript, edit it,
play it back, and export it.

Universal STT is a free, non-commercial personal project. The application
source is MIT licensed; the speech models keep their own licenses and are
attributed in the app and in `NOTICE`.

## Status

Core pipeline under construction. The workspace currently contains the
domain model (`ustt-core`). UI, exporters, packaging, and CI follow in
separate plans.

Design: `docs/superpowers/specs/2026-10-27-universal-stt-windows-design.md`.

## Build

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

Windows release builds are produced by GitHub Actions once the application
shell lands.

## Stack

- Rust (2024 edition)
- Who (Desert Ant Labs) for diarization
- Parakeet Redux via `parakeet.cpp` for speech recognition
- FFmpeg for media decoding and export
- Dioxus desktop for the UI (WebView2)

## License

MIT for this repository's code. Model and third-party licenses are listed in
`NOTICE` as they are added.
