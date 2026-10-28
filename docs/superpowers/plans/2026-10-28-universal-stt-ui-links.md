# Universal STT UI Stabilization and Link Import Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Repair native project layout/playback/navigation and add portable direct/YouTube/Yandex/VK URL import without changing the product stack.

**Architecture:** Fix the existing Dioxus UI in place. Replace buffered custom-protocol media with a localhost streaming HTTP server. Add a small downloader crate and feed its result into the existing single-file import pipeline.

**Tech Stack:** Rust, Dioxus 0.7.10, tiny_http 0.12, yt-dlp 2026.10.26, Deno 2.9.8, Playwright, existing FFmpeg/Who/Parakeet pipeline.

**Spec:** `docs/superpowers/specs/2026-10-28-universal-stt-ui-links-design.md`

## Global Constraints

- Preserve Dioxus, Inter, current tokens and dark/light themes; this is refinement, not redesign.
- One input and one import job at a time; no playlists or queue.
- Native media must not materialize the file or an open-ended range in `Vec<u8>`.
- Full portable ZIP must include `yt-dlp.exe` and `deno.exe`; no Python/Node/system downloader requirement.
- Only `http`/`https` URL input; no DRM/private-content bypass.
- Existing uncommitted runtime fixes are baseline; do not discard or rewrite them.
- No git mutations, push, tag or release without fresh explicit user confirmation. Task checkpoints record tests but do not commit.
- Native Linux test commands use `LIBRARY_PATH=/tmp/opencode/xdo` when linking `ustt-app`.

## Review Focus

1. Open-ended/suffix/out-of-bounds ranges and HEAD must not hang or allocate the whole proxy — Task 2 tests.
2. Switching projects must not leave the old media root or player source active — Tasks 3 and 4 tests.
3. Native text DnD may be unavailable; paste/manual URL input must remain a complete path — Task 8 Playwright test.
4. Cancel/error during download must remove partial files and never create a project — Tasks 6 and 7 tests.
5. Long text, narrow 1024 px layout, light theme and unsupported/login URLs must show useful states rather than blank UI — Tasks 1, 6 and 10 tests.

---

### Task 1: Repair project grid and visual states

**Files:**
- Modify: `crates/app/assets/main.css`
- Modify: `tests/ui/project.spec.ts`
- Create: `tests/ui/layout.spec.ts`

**Interfaces:**
- Consumes: existing `ProjectView`, `MediaKind`, web mock demo.
- Produces: stable `.project`, `.project-body`, `.transcript`, `.preview` layout used by Tasks 3–4.

- [ ] **Step 1: Write failing Playwright layout assertions**

Add `project body gets the remaining height` and `video keeps its aspect ratio inside the preview`. Open video demo at 1280×800; assert `.speaker-bar` height is below 60 px, `.project-body` height exceeds 500 px, `.transcript` scroll height is usable, and video `getBoundingClientRect()` stays inside `.preview`. Add a 1024×700 light-theme run with no horizontal overflow.

- [ ] **Step 2: Run the new tests before CSS changes**

Run: `npm test -- tests/ui/layout.spec.ts`
Expected: FAIL; current speaker bar receives the flexible row and body is compressed.

- [ ] **Step 3: Repair the grid**

Set `.project` to `grid-template-rows: auto auto minmax(0, 1fr) auto`. Add the necessary `min-width: 0`/`min-height: 0` to body, transcript and preview. Keep video `object-fit: contain`; do not change spacing outside the project screen.

- [ ] **Step 4: Verify layout and capture before/after screenshots**

Run: `npm test -- tests/ui/layout.spec.ts tests/ui/project.spec.ts`
Expected: PASS. Save 1280×800 video/audio screenshots in the workspace for visual inspection.

---

### Task 2: Streaming localhost media server

**Files:**
- Modify: `crates/app/Cargo.toml`
- Modify: `crates/app/src/service/media.rs`
- Create: `crates/app/tests/media_server.rs`

**Interfaces:**
- Consumes: existing `MediaRegistry::{insert_with_root, clear}` and shared registry.
- Produces: `MediaServer::start() -> std::io::Result<MediaServer>`, `MediaServer::url(&self, token: &str) -> String`, `MediaServer::port(&self) -> u16`; `parse_range(value: Option<&str>, total: u64) -> Result<Option<(u64, u64)>, RangeError>` (`None` = no Range header, tuple = inclusive valid range, `Err` = 416).

- [ ] **Step 1: Write failing HTTP tests**

Use a temporary registered file larger than 1 MiB. Against a server bound to `127.0.0.1:0`, assert:
- `GET bytes=100-199` → 206, exactly 100 correct bytes and `Content-Range`.
- `GET bytes=0-` → 206 with full `Content-Length`, read through a limited reader rather than a test helper requiring a buffered response.
- suffix range, HEAD, missing token 404, start-at-EOF 416 with `Content-Range: bytes */size`.
- No `Access-Control-Allow-Origin: *` header.

- [ ] **Step 2: Run tests and confirm the server is absent**

Run: `LIBRARY_PATH=/tmp/opencode/xdo cargo test -p ustt-app --test media_server`
Expected: FAIL to compile because `MediaServer` does not exist.

- [ ] **Step 3: Implement streaming responses**

Add native `tiny_http = "0.12"`. `MediaServer::start` binds only `127.0.0.1:0` and serves `/media/<token>` from the existing registry. Revalidate canonical path against root on every request. Stream `File::take(length)` with explicit content length; handle requests on worker threads so one stream cannot block seeks. Do not add directory listing or wildcard CORS.

- [ ] **Step 4: Run media tests**

Run: `LIBRARY_PATH=/tmp/opencode/xdo cargo test -p ustt-app --test media_server media`
Expected: all new and existing registry tests PASS.

---

### Task 3: Connect native player to streaming media

**Files:**
- Modify: `crates/app/src/main.rs`
- Modify: `crates/app/src/service/native.rs`
- Modify: `crates/app/src/service/media.rs`
- Modify: `crates/app/src/components/project.rs`
- Modify: `crates/app/src/components/player.rs`
- Modify: `crates/ui-model/src/i18n.rs`
- Modify: `tests/ui/playback.spec.ts`

**Interfaces:**
- Consumes: Task 2 `MediaServer` and existing `Service::media_url`.
- Produces: process-wide `media::start_shared() -> Result<&'static MediaServer, String>`; `media::shared_url(token) -> Option<String>`; localized `player_error`/`proxy_missing` states.

- [ ] **Step 1: Add failing integration tests**

Assert native `media_url` returns `http://127.0.0.1:<port>/media/proxy`, changes registered media after opening another project, and never returns `ustt://`. Add a browser assertion that a failed media element shows localized `player_error` instead of a blank preview.

- [ ] **Step 2: Run tests and confirm RED**

Run: `LIBRARY_PATH=/tmp/opencode/xdo cargo test -p ustt-app service::media` and `npm test -- tests/ui/playback.spec.ts`
Expected: URL/integration assertions fail; error-state test fails.

- [ ] **Step 3: Replace custom protocol wiring**

Start the shared server during native startup, remove the `ustt` custom protocol registration and buffered `respond` path, and return `shared_url` from `NativeService::media_url`. Server startup failure leaves media URL `None` while transcript remains usable. Preserve mock/blob behavior on wasm.

- [ ] **Step 4: Surface playback errors without replacing the bridge**

Listen for media `error`/`stalled` only to set a visible error state; keep existing play/pause/seek/timeupdate ownership. Reset error and seek to zero on project switch. Include proxy filename/MIME in diagnostic detail, not a raw Rust panic.

- [ ] **Step 5: Verify player and existing UI tests**

Run: `LIBRARY_PATH=/tmp/opencode/xdo cargo test -p ustt-app` and `npm test -- tests/ui/playback.spec.ts tests/ui/project.spec.ts`
Expected: PASS.

---

### Task 4: Speaker lane and transcript navigation

**Files:**
- Modify: `crates/app/src/components/timeline.rs`
- Modify: `crates/app/assets/timeline.js`
- Modify: `crates/app/assets/main.css`
- Modify: `crates/app/src/components/project.rs`
- Modify: `crates/app/src/components/transcript.rs`
- Modify: `crates/ui-model/src/timeline.rs`
- Modify: `crates/ui-model/src/edit.rs`
- Create: `tests/ui/navigation.spec.ts`

**Interfaces:**
- Consumes: `Project::{turns,segments}`, existing `Playback`, `TimelineLayout`.
- Produces: `best_segment_for_turn(segments, turn) -> Option<usize>`; `TimelineProps::on_turn: EventHandler<usize>` receiving the chosen segment index; project-level `scroll_request: Signal<Option<usize>>` consumed by `Transcript`.

- [ ] **Step 1: Write failing pure and browser tests**

Pure tests: maximum overlap wins, equal overlap picks the earlier segment, no overlap returns `None`. Browser test with >50 seeded segments: click a speaker-lane turn, assert media seeks to turn start and the chosen transcript row becomes visible/active while paused.

- [ ] **Step 2: Run tests before implementation**

Run: `cargo test -p ustt-ui-model best_segment_for_turn` and `npm test -- tests/ui/navigation.spec.ts`
Expected: FAIL because helper, lane and scroll request do not exist.

- [ ] **Step 3: Implement lane and scroll request**

Draw a separate 24–28 px speaker lane below waveform, with name/time tooltip and existing speaker colors. Canvas clicks in that lane resolve the turn; waveform clicks retain normal seek. `on_turn` seeks and sets `scroll_request`. Transcript reacts by scrolling to `index * ROW_HEIGHT` even when paused; explicit navigation re-enables following until the user wheels again.

- [ ] **Step 4: Verify navigation, rename and long transcript**

Run: `cargo test -p ustt-ui-model` and `npm test -- tests/ui/navigation.spec.ts tests/ui/project.spec.ts tests/ui/playback.spec.ts`
Expected: PASS, including existing rename+undo coverage.

---

### Task 5: Per-phase ETA in pipeline and Processing UI

**Files:**
- Modify: `crates/pipeline/src/progress.rs`
- Modify: `crates/pipeline/src/import.rs`
- Modify: `crates/pipeline/tests/import.rs`
- Modify: `crates/ui-model/src/progress.rs`
- Modify: `crates/app/src/service/native.rs`
- Modify: `crates/app/src/service/mock.rs`
- Modify: `crates/app/src/components/processing.rs`
- Modify: `crates/ui-model/src/i18n.rs`
- Create: `tests/ui/processing.spec.ts`

**Interfaces:**
- Produces: `Progress { phase_fraction, phase_eta_seconds, .. }`, `ProgressView { phase_fraction, phase_eta_seconds, .. }`, `with_phase_eta(progress, elapsed)`; existing `eta_seconds` remains overall.

- [ ] **Step 1: Write failing ETA tests**

Rust: for local fraction 0.25 after 10 s, phase ETA is 30 s; zero/done gives `None`; existing overall ETA is preserved. Browser: Processing shows distinct current-phase and overall ETA labels in RU and EN.

- [ ] **Step 2: Run focused tests and confirm RED**

Run: `cargo test -p ustt-pipeline progress` and `npm test -- tests/ui/processing.spec.ts`
Expected: fields/labels missing.

- [ ] **Step 3: Implement phase timing**

Record local `phase_fraction` in constructors, reset phase elapsed when `Phase` changes in central `emit`, and fill `phase_eta_seconds`. Map both new fields through native/mock service. Render two compact values without changing overall monotonic progress.

- [ ] **Step 4: Verify ETA and pipeline tests**

Run: `cargo test -p ustt-pipeline -p ustt-ui-model` and `npm test -- tests/ui/processing.spec.ts`
Expected: PASS.

---

### Task 6: Portable downloader crate

**Files:**
- Create: `crates/download/Cargo.toml`
- Create: `crates/download/src/lib.rs`
- Create: `crates/download/src/tools.rs`
- Create: `crates/download/src/progress.rs`
- Create: `crates/download/tests/download.rs`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`

**Interfaces:**
- Produces: `DownloadTools::locate(dir: Option<&Path>) -> Result<DownloadTools, DownloadError>`; `DownloadedMedia { path, title, temp }`; `download(tools, url, cancel, on_progress) -> Result<DownloadedMedia, DownloadError>`; `DownloadProgress { downloaded_bytes, total_bytes: Option<u64>, fraction: Option<f32> }`.

- [ ] **Step 1: Write fake-tool contract tests**

A fake `yt-dlp` executable/script verifies `--no-playlist`, `--newline`, `--js-runtimes deno:<path>`, final-file reporting and 1080p format selector. Tests cover progress parsing, direct result path, missing tools, nonzero exit, unsupported/login/DRM classification, cancellation and temp cleanup. Include a non-ASCII temp path case.

- [ ] **Step 2: Run tests and confirm RED**

Run: `cargo test -p ustt-download`
Expected: crate/tests fail because implementation is absent.

- [ ] **Step 3: Implement downloader wrapper**

Locate explicit dir → `USTT_DOWNLOAD_DIR` → executable dir. Spawn through `ustt_media::process::command`, stream stdout/stderr on reader threads, parse structured progress/final path, and own a `tempfile::TempDir` in `DownloadedMedia`. Kill on cancel and remove partials. Do not add Python/Node or network code beyond yt-dlp.

- [ ] **Step 4: Verify downloader crate**

Run: `cargo test -p ustt-download`
Expected: all contract tests PASS.

---

### Task 7: Remote source in pipeline, project schema and CLI

**Files:**
- Modify: `crates/core/src/model.rs`
- Modify: `crates/pipeline/Cargo.toml`
- Modify: `crates/pipeline/src/import.rs`
- Modify: `crates/pipeline/src/progress.rs`
- Modify: `crates/pipeline/tests/import.rs`
- Modify: `crates/pipeline/tests/e2e.rs`
- Modify: `crates/cli/src/main.rs`
- Modify: `crates/app/src/service/mock.rs` (SourceRef initializer only)

**Interfaces:**
- Consumes: Task 6 `download`/`DownloadedMedia`, Task 5 progress fields.
- Produces: `ImportSource::{File(PathBuf), Url(String)}`; `ImportRequest { input: ImportSource, .. }`; `Phase::Download`; `SourceRef { origin_url: Option<String>, .. }` with `#[serde(default)]`.

- [ ] **Step 1: Write compatibility and remote-import tests**

Old `project.json` without `origin_url` still deserializes. Unit tests cover progress scaling with `DOWNLOAD_WEIGHT = 0.25`, URL validation and download-guard cleanup on success/cancel. A gated model-available E2E proves URL input enters the same probe/decode/diarize/transcribe pipeline and records origin URL. Local import retains existing weights.

- [ ] **Step 2: Run tests and confirm RED**

Run: `cargo test -p ustt-core -p ustt-pipeline --test import`
Expected: missing enum/field/Download phase failures.

- [ ] **Step 3: Implement remote source lifecycle**

Resolve `ImportSource::Url` through Task 6 before probe; keep the download guard alive until import finishes. Set `origin_url`, use downloaded title/path for the local fingerprint, and clean up through RAII. Add `Progress::new_scaled(phase, fraction, detail, download_weight)`; remote scales remaining phases into 0.25–1.0. CLI accepts either an existing path or validated HTTP(S) URL.

- [ ] **Step 4: Verify core/pipeline/CLI**

Run: `cargo test -p ustt-core -p ustt-pipeline -p ustt-cli`
Expected: PASS.

---

### Task 8: URL drop/paste input and app service wiring

**Files:**
- Modify: `crates/app/src/service/mod.rs`
- Modify: `crates/app/src/service/native.rs`
- Modify: `crates/app/src/service/mock.rs`
- Modify: `crates/app/src/components/welcome.rs`
- Modify: `crates/app/src/app.rs`
- Modify: `crates/app/assets/main.css`
- Modify: `crates/ui-model/src/i18n.rs`
- Create: `tests/ui/url_import.spec.ts`

**Interfaces:**
- Consumes: Task 7 `ImportSource` and Dioxus `DragData::data_transfer()`.
- Produces: app-level `ImportInput::{File(PathBuf), Url(String)}`; `Service::import(ImportInput, PathBuf)`; `parse_import_url(&str) -> Result<String, String>`.

- [ ] **Step 1: Write failing URL validation and UI tests**

Validation accepts whitespace-wrapped HTTP(S), rejects FTP/javascript/empty/multiple URLs. Playwright covers manual paste+submit, `text/plain` drop, invalid localized error and mock remote Processing→Project. Manual paste must pass even if no drop event is available.

- [ ] **Step 2: Run focused tests and confirm RED**

Run: `LIBRARY_PATH=/tmp/opencode/xdo cargo test -p ustt-app parse_import_url` and `npm test -- tests/ui/url_import.spec.ts`
Expected: input/API missing.

- [ ] **Step 3: Implement unified input**

Welcome drop first tries native file path, then `get_as_text()`, then `text/uri-list`. Add URL input and import button as a permanent fallback. Map app `ImportInput` to pipeline `ImportSource` in native service; map Download in `ProgressView`. Mock URL import emits Download progress then the appropriate demo project.

- [ ] **Step 4: Verify app and URL flows**

Run: `LIBRARY_PATH=/tmp/opencode/xdo cargo test -p ustt-app` and `npm test -- tests/ui/url_import.spec.ts tests/ui/welcome.spec.ts tests/ui/processing.spec.ts`
Expected: PASS.

---

### Task 9: Bundle yt-dlp and Deno in portable release

**Files:**
- Modify: `scripts/fetch-win-deps.py`
- Modify: `scripts/package_release.py`
- Modify: `scripts/tests/test_fetch_win_deps.py`
- Modify: `scripts/tests/test_package_release.py`
- Modify: `.github/workflows/ci.yml`
- Modify: `NOTICE`
- Modify: `README.md`
- Modify: `docs/BUILD-WINDOWS.md`

**Interfaces:**
- Consumes: Task 6 explicit tool locator and pinned versions.
- Produces: required `yt-dlp.exe`, `deno.exe`, licenses and manifest entries in full ZIP.

- [ ] **Step 1: Write failing packaging tests**

Fixture/real-fetch validation requires both executables, rejects missing Deno, uses yt-dlp `2026.10.26` and Deno `v2.9.8` official URLs, and includes Unlicense/MIT notices. Assert no unexpected downloaded files enter the allowlist.

- [ ] **Step 2: Run packaging tests and confirm RED**

Run: `python3 -m pytest scripts/tests -q`
Expected: missing downloader/runtime requirement failures.

- [ ] **Step 3: Implement pinned fetch and packaging**

Extend the existing verified download/retry/checksum pipeline. Extract only `deno.exe` from `deno-x86_64-pc-windows-msvc.zip`; download standalone `yt-dlp.exe`. Update REQUIRED/allowlist/notices/docs. Do not make CI download online test media.

- [ ] **Step 4: Verify packaging and self-test**

Run: `python3 -m pytest scripts/tests -q` and `python3 scripts/package_release.py --self-test --root /tmp/opencode/packaging-ui-links`
Expected: PASS.

---

### Task 10: End-to-end acceptance and visual finish

**Files:**
- Create: `tests/e2e/direct_url.md` (manual evidence template only)
- Modify: `README.md`
- Update: existing Playwright selectors only if required by final UI.

**Interfaces:**
- Consumes: all previous tasks.

- [ ] **Step 1: Run complete automated suites**

Run:
```bash
cargo fmt --all -- --check
LIBRARY_PATH=/tmp/opencode/xdo cargo clippy --workspace --all-targets -- -D warnings
LIBRARY_PATH=/tmp/opencode/xdo cargo test --workspace
python3 -m pytest scripts/tests -q
npm test
cargo xwin build -p ustt-app -p ustt-cli --target x86_64-pc-windows-msvc --release
```
Expected: all exit 0; record exact test counts.

- [ ] **Step 2: Run direct URL E2E**

Serve a real audio/video fixture over local HTTP, import its URL through the same native/CLI path, and assert project creation, `origin_url`, proxy, transcript and no temporary download left behind.

- [ ] **Step 3: Inspect rendered UI and run Impeccable finish gate**

Capture 1280×800 and 1024×700 audio/video, dark/light, processing and URL-error states. Run the required `impeccable detect --json` on changed UI targets; fix critical/contrast/overflow findings. Use actual browser playback/seek and >50-segment navigation, not only static screenshots.

- [ ] **Step 4: Record native/online acceptance boundaries**

Test the original MP4 through native Windows/Proton where available. Attempt one accessible direct/YouTube/Yandex/VK URL each; record exact login/anti-bot/network limitation instead of claiming success. Hand the build to the user for physical Windows playback acceptance. No release without fresh confirmation.

## Self-review notes

- Spec coverage: layout Tasks 1/4, streaming Tasks 2/3, ETA Task 5, URL input Task 8, downloader Tasks 6/7, packaging Task 9, acceptance Task 10.
- Shared names are intentionally fixed: `MediaServer`, `best_segment_for_turn`, `phase_eta_seconds`, `ImportSource`, `ImportInput`, `origin_url`, `DOWNLOAD_WEIGHT`.
- No task requires a commit; standing git-confirmation rule takes precedence over checkpoint commits.
