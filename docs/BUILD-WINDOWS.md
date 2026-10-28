# Windows build and portable package

This document is the manual counterpart of `.github/workflows/release.yml`.
It produces the same ZIP that the tag workflow publishes.

## Prerequisites

- Windows 10/11 x64.
- Rust stable with the MSVC toolchain and Visual Studio Build Tools
  (`Desktop development with C++`).
- Python 3.12.
- Dioxus CLI 0.7.10.
- No WebView2 or Visual C++ runtime installation is required: the ZIP carries
  the fixed-version WebView2 runtime and the app-local MSVC DLLs.

```powershell
cargo install dioxus-cli --version 0.7.10 --locked
```

## Build

```powershell
cd crates/app
dx build --platform desktop --release
cd ../..

python scripts/fetch-win-deps.py --dest vendor/win64
```

`fetch-win-deps.py` downloads the pinned parakeet.cpp CPU/Vulkan libraries,
ONNX Runtime, a pinned FFmpeg LGPL shared build (with SHA-256 check), yt-dlp
2026.10.26 and Deno 2.9.8 (both SHA-256 checked), the fixed-version WebView2
runtime, the VC++ redistributable and the models. Models are verified against
`models/manifest.json` (SHA-256 and size).

Package:

```powershell
$exe = Get-ChildItem target/dx -Recurse -Filter universal-stt.exe |
  Select-Object -First 1
python scripts/package_release.py `
  --deps vendor/win64 `
  --manifest models/manifest.json `
  --exe $exe.FullName `
  --out dist `
  --version 0.1.0
```

Result:

```
dist/UniversalSTT-0.1.0-win64-portable.zip
dist/SHA256SUMS
```

The ZIP root contains `UniversalSTT.exe`, the DLLs, `ffmpeg.exe`, `ffprobe.exe`,
`yt-dlp.exe`, `deno.exe`, `models/`, `WebView2Runtime/`, `WebView2Loader.dll`,
the VC DLLs, `assets/` (if the Dioxus build emits one), `LICENSE`, `NOTICE`,
`README.md`, `version.txt` and `models/manifest.json`.

The release tag must match the `ustt-app` version in `Cargo.toml`; the CI
workflow fails otherwise.

Expected size: roughly 1.5–1.8 GB depending on the FFmpeg/WebView2 builds.
Models are stored without compression; the executable and DLLs are deflated.

## Verify

```powershell
Get-FileHash dist\UniversalSTT-*-win64-portable.zip -Algorithm SHA256
Get-Content dist\SHA256SUMS
```

Manual checklist:

- [ ] Extract to a writable folder with a space and a Cyrillic character in the path.
- [ ] Run `UniversalSTT.exe` without administrator rights and without installing
      WebView2 or VC++ (both are inside the ZIP).
- [ ] Drag an mp4, mkv, mp3, flac and wav into the window.
- [ ] Paste or drop a direct media URL and one accessible YouTube/Yandex/VK
      link; only one video downloads, with no playlist expansion.
- [ ] Two to four speakers, rename, save, reopen from recents.
- [ ] Video preview and transcript seek together.
- [ ] Undo/redo, Ctrl+S, autosave.
- [ ] Export DOCX/PDF (Cyrillic), SRT/VTT, audio per speaker and per selection.
- [ ] Disconnect the network; first run and all features work (the ZIP carries the WebView2 offline installer and VC++ redistributable).
- [ ] Vulkan backend on a Vulkan-capable GPU and CPU fallback when the DLL is removed.

## Data locations

- Projects: `Documents\Universal STT\<slug>-<date>.ustt`
- Settings and recents: `%APPDATA%\UniversalSTT`
- WebView2 cache: `WebView2Data\` next to the executable.

## CI

- `.github/workflows/ci.yml` runs fmt, clippy, workspace tests, Playwright and
  the Python packaging tests on every push/PR.
- `.github/workflows/release.yml` runs on `v*` tags, builds the Windows app,
  fetches deps/models, packages the ZIP and publishes the GitHub release.
- `#[ignore]` model/FFmpeg tests are intentionally not run in CI.

## Known limitations

- The executable is **unsigned**. Windows SmartScreen may show
  “Windows protected your PC”; choose “More info” → “Run anyway”.
- CUDA is not included in v1. The required DLLs and the CUDA runtime are large;
  Vulkan + CPU are the supported accelerators.
- There is no slim archive yet: the app has no model downloader.
- Windows acceptance under Wine/Proton-GE has not been performed. That lane is
  still a release blocker until someone runs it on a Windows-capable machine.
- FFmpeg is LGPL and runs as a separate executable; see `NOTICE` for the
  pinned build and source link.
- The MSVC runtime DLLs are deployed app-local; no redistributable install is
  needed. The fixed WebView2 runtime requires AppContainer ACLs on Windows 10;
  the app applies `icacls` on first start.
