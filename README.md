# Universal STT

Local, offline speech transcription and speaker diarization for Windows.
Drop in an audio or video file, get a speaker-labelled transcript, edit it,
play it back, and export it.

Universal STT is a free, non-commercial personal project. There is no paid
tier, no telemetry, and no account. It is published publicly so anyone can
build it and use it.

## Status

| Part | State |
|------|-------|
| Core domain, project store, undo | ready |
| FFmpeg probe/decode/waveform/playback proxy | ready |
| Who diarization (ONNX Runtime, CPU) | ready, golden-tested |
| Parakeet Redux transcription (parakeet.cpp) | ready |
| VAD chunking, progress, cancellation | ready |
| Segment building and speaker assignment | ready |
| CLI (`ustt-cli`) | ready |
| Desktop UI (Dioxus + WebView2) | next plan |
| DOCX/PDF export, audio extraction | next plan |
| Portable ZIP and GitHub Actions release | next plan |

The core pipeline runs end to end today:

```bash
cargo run -p ustt-cli -- process tests/fixtures/two_speakers.wav \
  --project /tmp/demo.ustt --models models --backend cpu
```

That produces a `.ustt` project with two speakers, word timings, a 16 kHz
WAV, a waveform, and a playback proxy.

## Requirements

- Windows 10 or 11 (the desktop shell will use the system WebView2 runtime).
- For development on Linux: Rust 2024 edition toolchain, FFmpeg, ONNX
  Runtime, and the parakeet.cpp shared library.
- No GPU is required. The CPU path is the reference; Vulkan and CUDA are
  optional accelerators.

## Models and libraries

Everything is downloaded by scripts into ignored directories. Nothing large
is committed.

```bash
./scripts/fetch-who.sh          # Who ONNX int8 model -> models/who
./scripts/fetch-parakeet.sh     # parakeet.cpp libs -> lib, Redux GGUFs -> models
./scripts/fetch-ort.sh          # ONNX Runtime CPU -> lib
./scripts/write-manifest.sh     # models/manifest.json with SHA-256
./scripts/fetch-fixtures.sh     # rebuild tests/fixtures/two_speakers.wav
```

At runtime the CLI looks for `libparakeet` next to the executable,
in `lib/`, or in `USTT_LIB_DIR`. ONNX Runtime is loaded dynamically with
`load-dynamic`; point `ORT_DYLIB_PATH` at it during development (on Windows
the DLL next to the executable is found automatically).

Accelerator backends:

| Backend | Library | Model |
|---------|---------|-------|
| CPU | `parakeet_cpu.dll` | `redux-packed.gguf` (213 MB) |
| Vulkan | `parakeet_vulkan.dll` | `redux-q8_0.gguf` (942 MB) |
| CUDA | `parakeet_cuda.dll` | `redux-q8_0.gguf` |

Who always runs on the ONNX Runtime CPU provider. Backend selection is
automatic (CUDA, then Vulkan, then CPU) and can be forced with
`USTT_BACKEND=cpu|vulkan|cuda` or the CLI flag.

## CLI

```bash
# media information
ustt-cli probe <file>

# backends, GPU, and model hashes
ustt-cli doctor --models models

# full import; progress is one JSON object per line
ustt-cli process <file> --project <dir>.ustt --models models \
  [--backend auto|cpu|vulkan|cuda] [--no-proxy]
```

## Build and test

```bash
cargo test --workspace
cargo test --workspace -- --include-ignored   # needs models and libparakeet
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

Model-backed tests are `#[ignore]`d so a clean checkout stays green. Set
`USTT_MODELS_DIR` (default `models`) and `ORT_DYLIB_PATH` to run them.

## Design

- Design: `docs/superpowers/specs/2026-10-27-universal-stt-windows-design.md`
- Core plan: `docs/superpowers/plans/2026-10-27-universal-stt-core-pipeline.md`

## Licenses and attribution

This repository's code is MIT (`LICENSE`). Third-party components keep their
own licenses:

- **Who** (`desert-ant-labs/who`) — DAL Source-Available 1.0. Embedding in an
  application is permitted, free below 100k monthly active users per platform
  per model, attribution required. Outputs must not be used to train a
  competing on-device model.
- **Parakeet Redux** (`moondream/parakeet-redux`) — CC-BY-4.0.
- **parakeet.cpp** — MIT (header vendored in `vendor/parakeet`).
- **Silero VAD** — MIT.
- **ONNX Runtime** — MIT.
- **FFmpeg** — the shipped build is LGPL; FFmpeg runs as a separate
  executable, so this repository's MIT code is unaffected.
- **Test audio** — LibriSpeech (CC-BY-4.0), LJSpeech (public domain), and the
  JFK sample from whisper.cpp (public domain). See `NOTICE`.

`Universal STT` is not affiliated with Desert Ant Labs, NVIDIA, or the
parakeet.cpp project.
