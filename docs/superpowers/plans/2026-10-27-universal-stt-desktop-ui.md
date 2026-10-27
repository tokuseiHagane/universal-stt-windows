# Universal STT Desktop UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the Dioxus 0.7 desktop UI for Universal STT: welcome, import progress, audio/video project views with a synchronized transcript, AIMP-style timeline, editing with undo/autosave, recent projects and settings.

**Architecture:** One Dioxus app crate (`ustt-app`) renders both the desktop shell and the web target used by Playwright. All non-UI math (timeline geometry, virtualization, playback state, i18n, text formatting, edit helpers) lives in a Dioxus-free crate (`ustt-ui-model`) with unit tests. A `Service` trait isolates I/O: the native implementation runs the existing `ustt-pipeline` on a worker thread and serves project media over a custom `ustt://` protocol with HTTP Range; the wasm implementation returns deterministic demo projects. Components are split by screen and feature, with design tokens in one CSS file.

**Tech Stack:** Rust 2024, Dioxus 0.7.10 (`desktop` feature, `web` for tests), dioxus-cli 0.7.10, `ustt-core`/`ustt-media`/`ustt-pipeline`, `rfd` 0.15, `dirs` 6, `chrono`, `futures-timer`, Playwright (Chromium), HTML5 `<audio>`/`<video>` + canvas.

**Spec:** `docs/superpowers/specs/2026-10-27-universal-stt-windows-design.md` (§9 UI, §10 export is a later plan, §12 build, §13 verification)

## Global Constraints

- UI languages: RU and EN; default from system locale; all strings in `ustt-ui-model::i18n`.
- Dark theme default, light optional; palette graphite + one accent + exactly the 8 `SPEAKER_COLORS`; radii 10–12 px; transitions 120–160 ms.
- No external fonts or icon fonts at runtime: Inter woff2 and an inline SVG set are embedded assets.
- One import at a time; no queue. Progress/cancel must remain responsive.
- `Ctrl+S` saves; `Ctrl+Z` undo; `Ctrl+Shift+Z` and `Ctrl+Y` redo; `Space` play/pause; `←/→` ±5 s; `Shift+←/→` ±30 s; `Home/End`.
- Autosave every `Settings::autosave_secs` (default 5 s) when dirty; undo depth 100 (already in `History`).
- Original media is never copied; only project `media/audio16k.wav`, `waveform.bin`, `proxy.*` are read.
- Media URLs go through a token registry; the protocol handler rejects any path outside the open project directory.
- Web build uses only mock data; it must not be able to touch the filesystem.
- Export (DOCX/PDF/audio) is out of scope for this plan; no placeholder that pretends to work.
- Windows 10/11 is the target; Linux is the development and Playwright host.

## Review Focus

1. **Three-hour project.** Transcript renders only visible rows; waveform JSON is decimated to canvas width; playback updates never serialize all peaks. Test — Task 6 `virtualized_rows_stay_bounded`, Task 7 `timeline_payload_is_decimated`.
2. **Undo vs autosave.** Every edit pushes one history state, autosave clears dirty, undo after save marks dirty again; switching projects saves first. Test — Task 6 `edit_then_save_then_undo_marks_dirty`.
3. **Seeking before metadata / on pause.** Clicking a row or the timeline seeks even when paused and before `durationchange`; clamping handles duration 0. Test — Task 5 `seek_before_metadata_is_clamped`, Task 7 `timeline_click_maps_to_time`.
4. **Native failures are visible.** Missing models, FFmpeg or project files surface a readable error on the welcome/processing screen, never a silent no-op. Test — Task 3 `native_errors_survive_the_trait`, Task 9 `missing_project_shows_error`.
5. **Drop paths and protocol safety.** Paths with spaces/non-ASCII work; `ustt://` tokens cannot escape the project directory. Test — Task 7 `media_registry_rejects_traversal`.

---

## File Structure

```
Cargo.toml                       # workspace members += crates/ui-model, crates/app
crates/ui-model/                 # pure logic, no Dioxus/IO
  src/lib.rs                     # re-exports
  src/timeline.rs                # TimelineLayout, hit testing, zoom/scroll
  src/transcript.rs              # VirtualRows, active segment lookup
  src/playback.rs                # Playback state, seek/step/clamp
  src/i18n.rs                    # Lang, Strings, strings()
  src/format.rs                  # timecode/duration/date labels
  src/edit.rs                    # rename speaker, segment text, speaker time
  tests/model.rs
crates/app/                      # Dioxus binary
  Cargo.toml
  Dioxus.toml
  assets/main.css                # tokens, layout, components
  assets/timeline.js             # canvas draw + playhead + scroll helpers
  assets/fonts/Inter-Regular.woff2
  assets/fonts/Inter-SemiBold.woff2
  assets/demo/audio.wav          # 3 s tone for web demo
  assets/demo/video.mp4          # 3 s testsrc for web demo
  src/main.rs                    # launch + window config
  src/app.rs                     # root shell, screen switch, global keys
  src/service/mod.rs             # Service trait, ImportEvent, ImportHandle
  src/service/native.rs          # pipeline thread, settings/recent, rfd
  src/service/mock.rs            # wasm demo projects
  src/service/media.rs           # token registry + Range protocol
  src/components/welcome.rs
  src/components/processing.rs
  src/components/project.rs      # layout, speakers, transcript, transport
  src/components/speakers.rs
  src/components/transcript.rs
  src/components/timeline.rs
  src/components/settings.rs
  src/components/icons.rs
playwright.config.ts
tests/ui/welcome.spec.ts
tests/ui/project.spec.ts
tests/ui/playback.spec.ts
scripts/serve-web.sh
```

---

### Task 1: Pure UI model crate

**Files:**
- Create: `crates/ui-model/Cargo.toml`, `crates/ui-model/src/{lib,timeline,transcript,playback,i18n,format,edit}.rs`
- Create: `crates/ui-model/tests/model.rs`
- Modify: root `Cargo.toml`

**Interfaces:**
- Produces: `TimelineLayout { px_per_sec, scroll_sec, width_px, duration }` with `new(px_per_sec, width_px, duration)`, `x(t)`, `time(x)`, `visible() -> (f64, f64)`, `zoom_at(x, factor)`, `scroll_by(dx)`; `hit_segment(&[Segment], t) -> Option<usize>`; `VirtualRows { first, last, offset_px, total_px }` and `virtual_rows(scroll_top, viewport_h, row_h, count)`; `Playback { position, duration, playing, rate, volume, muted }` with `seek(t)`, `step(delta)`, `set_duration(d)`; `Lang { Ru, En }` with `from_system(Option<&str>)`; `Strings` struct and `strings(Lang) -> &'static Strings`; `timecode(f64) -> String`, `duration_label(f64)`, `date_label(&DateTime<Utc>, Lang)`; `rename_speaker(&mut Project, Uuid, &str) -> bool`, `set_segment_text(&mut Project, Uuid, &str) -> bool`, `speaker_time(&Project, Uuid) -> f64`.

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn timeline_maps_both_ways_and_clamps() {
    let mut t = TimelineLayout::new(10.0, 100.0, 120.0);
    assert!((t.x(5.0) - 50.0).abs() < 1e-9);
    assert!((t.time(50.0) - 5.0).abs() < 1e-9);
    assert_eq!(t.time(-10.0), 0.0);
    assert_eq!(t.time(10_000.0), 120.0);
}

#[test]
fn zoom_keeps_the_cursor_time_under_the_cursor() {
    let mut t = TimelineLayout::new(10.0, 30.0, 120.0);
    let before = t.time(400.0);
    t.zoom_at(400.0, 2.0);
    assert!((t.time(400.0) - before).abs() < 1e-6);
    assert!(t.px_per_sec <= 200.0);
}

#[test]
fn virtual_rows_cover_the_viewport_only() {
    let v = virtual_rows(640.0, 480.0, 64.0, 10_000);
    assert_eq!(v.first, 8);
    assert_eq!(v.last, 18);
    assert_eq!(v.total_px, 640_000);
}

#[test]
fn seek_before_metadata_is_clamped() {
    let mut p = Playback::default();
    p.seek(12.0);
    assert_eq!(p.position, 0.0);
    p.set_duration(60.0);
    p.seek(12.0);
    assert_eq!(p.position, 12.0);
    p.seek(999.0);
    assert_eq!(p.position, 60.0);
}

#[test]
fn both_languages_fill_every_string() {
    for lang in [Lang::Ru, Lang::En] {
        let s = strings(lang);
        assert!(!s.app_name.is_empty());
        assert!(!s.welcome_drop.is_empty());
        assert!(!s.processing_cancel.is_empty());
        assert_ne!(strings(Lang::Ru).welcome_drop, strings(Lang::En).welcome_drop);
    }
}

#[test]
fn edit_helpers_report_change() {
    let mut p = sample_project();
    assert!(rename_speaker(&mut p, p.speakers[0].id, "Alice"));
    assert!(!rename_speaker(&mut p, p.speakers[0].id, "Alice"));
    assert!(set_segment_text(&mut p, p.segments[0].id, "new"));
    assert_eq!(speaker_time(&p, p.speakers[0].id), p.segments[0].end - p.segments[0].start);
}
```

- [ ] **Step 2: Run — FAIL**

Run: `cargo test -p ustt-ui-model`
Expected: FAIL (crate absent).

- [ ] **Step 3: Implement**

`TimelineLayout::new(px_per_sec, width_px, duration)` with `scroll_sec = 0`; `x = (t - scroll_sec) * px_per_sec`; `time` clamps to `0..=duration`; `zoom_at` clamps `px_per_sec` to `1.0..=2000.0` and adjusts `scroll_sec` so the pointer time is unchanged; `scroll_by(dx)` clamps `scroll_sec` to `0..=(duration - width_px/px_per_sec).max(0)`.

`virtual_rows`: `first = (scroll_top / row_h).floor()`, `last = ((scroll_top + viewport_h) / row_h).ceil().min(count)`, `offset_px = first * row_h`, `total_px = count * row_h`; row height exactly 64.0.

`Playback::seek`: clamp against `duration` if `duration > 0`, else 0. `step` uses the same seek. `set_duration` re-clamps position.

`i18n::Strings` covers every visible label: app name, welcome title/subtitle/drop/browse/recent/empty, processing phases/cancel/eta, project save/undo/redo/export-later/speakers/transcript/current, player play/pause/rate/volume, settings fields, errors. `Lang::from_system` maps `ru*` to Ru, everything else En.

`edit` helpers mutate and return whether the value changed; empty names and texts are rejected.

- [ ] **Step 4: PASS**

Run: `cargo test -p ustt-ui-model`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock crates/ui-model
git commit -m "feat(ui-model): timeline, virtualization, playback and i18n logic"
```

---

### Task 2: Dioxus app shell and design tokens

**Files:**
- Create: `crates/app/Cargo.toml`, `crates/app/Dioxus.toml`, `crates/app/src/main.rs`, `crates/app/src/app.rs`, `crates/app/assets/main.css`, `crates/app/assets/timeline.js`, `crates/app/src/components/icons.rs`, `scripts/install-tools.sh`
- Create: `crates/app/assets/fonts/Inter-Regular.woff2`, `Inter-SemiBold.woff2`
- Modify: root `Cargo.toml`, `.gitignore`

**Interfaces:**
- Consumes: `ustt-ui-model::{Lang, strings}`.
- Produces: `App()` root component; `Screen { Welcome, Processing, Project }` in `app.rs`; CSS custom properties `--bg`, `--surface`, `--surface-2`, `--text`, `--muted`, `--accent`, `--speaker-0..7`, `--radius`, `--motion`; `data-theme="dark|light"` on `<html>`.

- [ ] **Step 0: Toolchain**

Run `scripts/install-tools.sh` (created in Step 4): wasm target, `dioxus-cli` 0.7.10, and later Playwright browsers.

- [ ] **Step 1: Vendored assets**

Download Inter 4.1 woff2 Regular and SemiBold from the official `rsms/inter` release into `crates/app/assets/fonts/` (commit both files, ~200 KB). Create a 3 s 440 Hz mono WAV (`assets/demo/audio.wav`) and a 3 s 320×180 H.264-free MP4 proxy from `ffmpeg testsrc` (`assets/demo/video.mp4`, VP9/WebM is also acceptable if WebView2 plays it; prefer MP4 for Edge).

- [ ] **Step 2: Write the smoke test**

`crates/app/src/app.rs`:

```rust
#[test]
fn screen_names_are_stable() {
    assert_eq!(Screen::Welcome.as_str(), "welcome");
    assert_eq!(Screen::Processing.as_str(), "processing");
    assert_eq!(Screen::Project.as_str(), "project");
}
```

- [ ] **Step 3: Run — FAIL**

Run: `cargo test -p ustt-app`
Expected: FAIL (crate absent).

- [ ] **Step 4: Implement scaffold**

`crates/app/Cargo.toml`:

```toml
[package]
name = "ustt-app"
version.workspace = true
edition.workspace = true
license.workspace = true

[features]
default = ["desktop"]
desktop = ["dioxus/desktop", "dep:rfd", "dep:dirs"]
web = ["dioxus/web"]

[dependencies]
dioxus = { version = "0.7.10" }
futures-timer = "3"
ustt-core = { path = "../core" }
ustt-media = { path = "../media" }
ustt-pipeline = { path = "../pipeline" }
ustt-ui-model = { path = "../ui-model" }
chrono.workspace = true
serde_json.workspace = true
rfd = { version = "0.15", optional = true }
dirs = { version = "6", optional = true }

[target.'cfg(target_arch = "wasm32")'.dependencies]
gloo-timers = { version = "0.3", features = ["futures"] }
wasm-bindgen = "0.2"
```

`Dioxus.toml`:

```toml
[application]
name = "Universal STT"
default_platform = "desktop"
public_dir = "assets"

[web.app]
title = "Universal STT"
```

`scripts/install-tools.sh` is idempotent: `rustup target add wasm32-unknown-unknown`, `cargo install dioxus-cli --version 0.7.10 --locked`, and prints `dx --version`.

`main.rs` launches with title `Universal STT`, 1280×800, min 1024×700, background `(18,20,24,255)`, default drag-drop handler enabled.

`main.css`: reset, dark/light token blocks, Inter `@font-face`, app grid, buttons, inputs, modal/scrim, chips, progress bar, transcript rows, timeline/transport, welcome dropzone, settings form, focus rings, `.sr-only`. Colors come from `SPEAKER_COLORS`. `timeline.js` exposes `window.usttTimeline = { draw(canvas, scene), setPlayhead(id, x, visible), scrollToRow(id), initKeys() }`.

`app.rs` renders header (logo, project title, save/undo/redo, language/theme, settings) and switches placeholder screens. `icons.rs` returns inline `svg` elements for play, pause, undo, redo, save, settings, folder, chevron.

- [ ] **Step 5: Build both targets**

Run: `cargo test -p ustt-app`
Run: `dx build --platform web` (installed per Task 11 environment note)
Expected: both succeed.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock crates/app .gitignore
git commit -m "feat(app): dioxus shell, design tokens and assets"
```

---

### Task 3: Service abstraction, native import worker, mock demo

**Files:**
- Create: `crates/app/src/service/mod.rs`, `native.rs`, `mock.rs`
- Modify: `crates/app/src/app.rs`, `crates/app/src/main.rs`

**Interfaces:**
- Produces: `Service` trait (`settings`, `save_settings`, `recent`, `pick_media`, `pick_project`, `import`, `open`, `save`, `media_url`), `ImportHandle { rx: std::sync::mpsc::Receiver<ImportEvent>, cancel: Arc<AtomicBool> }`, `ImportEvent { Progress(Progress), Finished(Box<Project>), Failed(String) }`, `StoreHandle` (native `ProjectStore`, wasm path), `MediaAsset { Audio, Proxy, Waveform }`, `project_dir_for(input, documents_dir) -> PathBuf`, `open_service() -> Arc<dyn Service>`, `NativeError`.

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn project_dir_is_slugged_and_dated() {
    let dir = project_dir_for(Path::new("/tmp/My Talk (final).mp4"), Path::new("/docs"));
    let name = dir.file_name().unwrap().to_string_lossy();
    assert!(name.starts_with("my-talk-final-"));
    assert!(name.ends_with(".ustt"));
    assert_eq!(dir.parent().unwrap(), Path::new("/docs/Universal STT"));
}

#[test]
fn import_error_is_kept_as_text() {
    let err = NativeError::Message("missing ffmpeg".into());
    assert_eq!(err.to_string(), "missing ffmpeg");
}
```

- [ ] **Step 2: Run — FAIL**

Run: `cargo test -p ustt-app service`
Expected: FAIL.

- [ ] **Step 3: Implement**

`Service` is object-safe (`Arc<dyn Service>`); all methods take `&self`. `NativeService` holds config paths and an `Arc<Mutex<MediaRegistry>>`. The import channel is `std::sync::mpsc` on both targets; the native worker thread is its only writer.

`import`: resolve models via `Settings::models_dir` (default: `models` next to the exe, then cwd), detect backend, spawn `std::thread` that calls `run_import` with a fresh `AtomicBool` and sends every `Progress`, then `Finished(project)` or `Failed(err.to_string())`. The project directory is `project_dir_for` under `dirs::document_dir()/Universal STT`, falling back to the config directory when Documents is unavailable.

`open`: `ProjectStore::open`, then register `audio16k.wav`, `waveform.bin`, and the first `proxy.*` in the media registry. Return `StoreHandle::Native(store)` plus project. Errors map to `NativeError::Message`.

`save`: `store.save(project)`.

`MockService`: wasm only; returns a deterministic 2-speaker project (audio demo) and a video demo project, with `media_url` pointing at `asset!("/assets/demo/audio.wav")` / `video.mp4`. `import` sends 5 progress events immediately followed by `Finished`. `pick_media`/`pick_project` return `None`.

`media_url` returns `ustt://media/<token>` on native (implemented in Task 7) and asset URLs on web.

- [ ] **Step 4: PASS**

Run: `cargo test -p ustt-app service` and `cargo test -p ustt-app`.
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/service crates/app/src/app.rs crates/app/src/main.rs
git commit -m "feat(app): service boundary, native import worker and web demo"
```

---

### Task 4: Welcome screen with drop, picker and recent projects

**Files:**
- Create: `crates/app/src/components/welcome.rs`
- Modify: `crates/app/src/app.rs`, `crates/app/assets/main.css`

**Interfaces:**
- Consumes: `Service`, `Screen`, `RecentEntry`, `MediaKind`.
- Produces: `Welcome { on_start_import: EventHandler<(PathBuf, PathBuf)>, on_open: EventHandler<PathBuf> }`.

- [ ] **Step 1: Write the failing component test**

`crates/app/src/components/welcome.rs` (pure helper):

```rust
#[test]
fn drop_accepts_the_first_supported_file() {
    let files = vec!["/tmp/a.txt".to_string(), "/tmp/b.MP4".to_string(), "/tmp/c.wav".to_string()];
    assert_eq!(pick_dropped(&files), Some(PathBuf::from("/tmp/b.MP4")));
    assert_eq!(pick_dropped(&["/tmp/a.txt".into()]), None);
}
```

Supported extensions: `mp3 wav flac ogg opus m4a aac wma mp4 mkv mov avi webm m4v mpeg mpg ts`.

- [ ] **Step 2: Run — FAIL**

Run: `cargo test -p ustt-app pick_dropped`
Expected: FAIL.

- [ ] **Step 3: Implement**

Welcome layout: brand + tagline; large dropzone (`ondragover` prevent default, `ondrop` reads `evt.data_transfer().files()`, takes `file.path()`), primary “Выбрать файл” button calling `service.pick_media()`, recent list (title, date `date_label`, duration `duration_label`, kind icon) with click → `on_open`, empty state. Drop path triggers `on_start_import(path, project_dir_for(path, documents_dir))`; errors render in a dismissible banner. A wasm-only “Открыть демо” button is shown when `service.demo_available()`.

Accessibility: dropzone is a `<button>` with `aria-label`; recent rows are `<button>`; focus ring from CSS.

- [ ] **Step 4: PASS**

Run: `cargo test -p ustt-app pick_dropped`.
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/components/welcome.rs crates/app/src/app.rs crates/app/assets/main.css
git commit -m "feat(app): welcome screen with drop, picker and recents"
```

---

### Task 5: Processing screen, polling and cancellation

**Files:**
- Create: `crates/app/src/components/processing.rs`
- Modify: `crates/app/src/app.rs`, `crates/app/src/service/mod.rs`, `crates/app/assets/main.css`

**Interfaces:**
- Consumes: `ImportEvent`, `ImportHandle`, `Progress`, `Phase`.
- Produces: `Processing { file_name, handle: ImportHandle, on_done: EventHandler<Project>, on_cancel: EventHandler<()> }`; `poll_import(rx, on_event)` async helper using 80 ms `futures_timer::Delay` ticks (works on native and wasm).

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn phase_label_covers_every_phase() {
    for phase in [Phase::Probe, Phase::Decode, Phase::Proxy, Phase::Diarize, Phase::Transcribe, Phase::Merge, Phase::Done] {
        assert!(!phase_label(phase, Lang::Ru).is_empty());
        assert!(!phase_label(phase, Lang::En).is_empty());
    }
}

#[test]
fn eta_text_is_optional() {
    assert_eq!(eta_text(None, Lang::Ru), "Осталось: —");
    assert!(eta_text(Some(61.0), Lang::Ru).contains("1"));
}
```

- [ ] **Step 2: Run — FAIL**

Run: `cargo test -p ustt-app phase_label`.
Expected: FAIL.

- [ ] **Step 3: Implement**

Processing view: file name, phase label, percent bar (`progress.fraction * 100`), ETA, detail, Cancel button. The screen owns the `ImportHandle`; a `use_future` polls with `try_recv` every 80 ms, updates signals, and on `Finished` marks the recent list, saves settings, and calls `on_done`; on `Failed` shows the message plus “Назад”.

Cancel sets `handle.cancel` and waits for `Cancelled`; the screen returns to Welcome after the event. Closing the screen drops the handle but the worker keeps running: keep the handle in `App` state so returning to Welcome/another import cannot start a second job; a banner “Идёт обработка” offers to return.

Native worker uses `Arc<AtomicBool>`; wasm mock ignores it.

- [ ] **Step 4: PASS**

Run: `cargo test -p ustt-app phase_label eta_text`.
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/components/processing.rs crates/app/src/app.rs crates/app/src/service/mod.rs crates/app/assets/main.css
git commit -m "feat(app): processing screen with live progress and cancel"
```

---

### Task 6: Project shell, speakers, virtualized transcript, editing and autosave

**Files:**
- Create: `crates/app/src/components/project.rs`, `speakers.rs`, `transcript.rs`
- Modify: `crates/app/src/app.rs`, `crates/app/src/service/mod.rs`, `crates/app/assets/main.css`
- Test: `crates/app/src/components/project.rs` (pure helpers)

**Interfaces:**
- Consumes: `History<Project>`, `ProjectStore`/`StoreHandle`, `rename_speaker`, `set_segment_text`, `speaker_time`, `virtual_rows`, `Strings`.
- Produces: `EditableProject { history: History<Project>, store: StoreHandle, dirty: Signal<bool>, saved_at: Signal<Option<DateTime<Utc>>> }`; `active_segment(&Project, f64) -> Option<usize>`; `row_height() -> f64 = 64.0`; `edit_then_save_then_undo_marks_dirty(history)`.

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn edit_then_save_then_undo_marks_dirty() {
    let mut h = History::new(sample_project(), 100);
    rename_speaker(h.current_mut(), h.current().speakers[0].id, "Alice");
    h.commit();
    assert!(h.is_dirty());
    h.mark_saved();
    assert!(!h.is_dirty());
    let restored = h.undo().unwrap();
    assert_ne!(restored.speakers[0].name, "Alice");
    assert!(h.can_redo());
    h.commit();
    assert!(h.is_dirty());
}

#[test]
fn active_segment_follows_playhead() {
    let p = sample_project();
    assert_eq!(active_segment(&p, 0.5), Some(0));
    assert_eq!(active_segment(&p, 100.0), None);
}
```

- [ ] **Step 2: Run — FAIL**

Run: `cargo test -p ustt-app edit_then_save`.
Expected: FAIL.

- [ ] **Step 3: Implement**

Project layout: header (title, save state “Сохранено/Не сохранено”, undo/redo, project switcher), speaker chips row (color dot, inline `<input>` rename on double-click/Enter, `speaker_time`), transcript panel, player area, timeline/transport placeholders filled in Task 7.

Transcript: fixed 64 px rows, scroll container with top/bottom spacer heights from `VirtualRows`. Each row: `data-testid="segment-row"`, timecode, speaker dot + name, text (2-line clamp). Click seeks; double-click turns text into `<textarea>`; commit on Enter/blur (Shift+Enter newline) via `set_segment_text`, push one history state, mark dirty. Speaker filter is visual only: non-selected speakers get 45% opacity, selected stays 100%.

Autosave: `use_future` loop every `settings.autosave_secs`; if dirty and store present, `service.save`, `history.mark_saved()`, set `saved_at`. `Ctrl+S` triggers the same path. Switching project saves first.

Undo/redo: header buttons and keys call `history.undo/redo`, commit the restored state, mark dirty; scroll active row into view.

- [ ] **Step 4: PASS**

Run: `cargo test -p ustt-app edit_then_save active_segment`.
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/components/project.rs crates/app/src/components/speakers.rs crates/app/src/components/transcript.rs crates/app/src/app.rs crates/app/src/service/mod.rs crates/app/assets/main.css
git commit -m "feat(app): project shell, speakers, transcript editing and autosave"
```

---

### Task 7: Playback bridge, timeline canvas and media protocol

**Files:**
- Create: `crates/app/src/components/timeline.rs`, `crates/app/src/service/media.rs`
- Modify: `crates/app/src/components/project.rs`, `crates/app/src/app.rs`, `crates/app/src/service/mod.rs`, `crates/app/assets/timeline.js`, `crates/app/assets/main.css`
- Test: `crates/app/src/service/media.rs`, `crates/app/src/components/timeline.rs`

**Interfaces:**
- Produces: `MediaRegistry::{insert(token, path), insert_with_root(token, path, root), url(token) -> String, resolve(uri) -> Option<(PathBuf, String)>}`; `media_response(registry, request) -> wry::http::Response<Cow<'static, [u8]>>` with `200`/`206`/`404`; `TimelineScene { duration, peaks, turns, segments, speaker_colors }` and `canvas_peaks(&[Peak], width_px) -> Vec<(f32, f32)>`; `timeline_click_to_time(x, &TimelineLayout) -> f64`.

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn timeline_click_maps_to_time() {
    let layout = TimelineLayout::new(10.0, 0.0, 60.0);
    assert!((timeline_click_to_time(150.0, &layout) - 15.0).abs() < 1e-9);
}

#[test]
fn timeline_payload_is_decimated() {
    let peaks: Vec<Peak> = (0..100_000).map(|i| Peak { min: -0.5, max: 0.5 + i as f32 * 0.0 }).collect();
    assert_eq!(canvas_peaks(&peaks, 800).len(), 800);
}

#[test]
fn media_registry_rejects_traversal() {
    let dir = tempfile::tempdir().unwrap();
    let mut r = MediaRegistry::new();
    let inside = dir.path().join("a.wav");
    std::fs::write(&inside, b"x").unwrap();
    r.insert_with_root("audio", inside.clone(), dir.path().to_path_buf());
    assert!(r.resolve("ustt://media/audio").is_some());
    r.insert_with_root("evil", dir.path().join("../secret"), dir.path().to_path_buf());
    assert!(r.resolve("ustt://media/evil").is_none());
}

#[test]
fn range_request_returns_206() {
    // build a 100-byte file, request bytes=10-19, assert status 206 and 10 bytes
}
```

- [ ] **Step 2: Run — FAIL**

Run: `cargo test -p ustt-app timeline_click_to_time media_registry`.
Expected: FAIL.

- [ ] **Step 3: Implement**

Media registry: global `OnceLock<Arc<Mutex<MediaRegistry>>>` shared by `main.rs` (protocol handler) and `NativeService`. Tokens are `audio`, `proxy`, `waveform` and map to canonicalized paths under the open project. `resolve` parses `ustt://media/<token>`, canonicalizes the stored path, and requires `starts_with(project_root)`. Handler returns `Content-Type` by extension (`.wav audio/wav`, `.m4a audio/mp4`, `.mp4 video/mp4`, `.webm video/webm`), `Accept-Ranges: bytes`, and `206 Partial Content` for `Range: bytes=a-b`; `HEAD` returns headers only; anything else `404`.

Timeline: canvas id `timeline`, `onmounted` records `get_client_rect`; scene is sent to `timeline.js` on changes (duration, zoom, scroll, turns, waveform decimated to canvas width). Wheel zooms (`zoom_at(cursor_x, 1.2/0.8)`), Shift+wheel scrolls, click seeks, drag scrubs, pointer selection sets `selection: Option<(f64, f64)>`. Playhead is a DOM overlay (`data-testid="playhead"`) positioned by CSS transform at 60 fps from playback events, so the canvas is not redrawn per frame.

Player: `<audio>`/`<video>` with `id="player"`, `src=media_url`, `preload="metadata"`. A long-lived `document::eval` attaches `timeupdate`, `durationchange`, `play`, `pause`, `ended`, `volumechange` and sends JSON `{position,duration,playing,rate,volume,muted}`; Rust receives it in a `use_future` and updates `Playback`. Commands (`play`, `pause`, `seek`, `rate`, `volume`, `mute`) go through `player.js` helpers called with `document::eval`. Keyboard listener sends keys; the app maps them to `Playback`.

- [ ] **Step 4: PASS**

Run: `cargo test -p ustt-app timeline_click_to_time timeline_payload_is_decimated media_registry_rejects_traversal range_request_returns_206`.
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/components/timeline.rs crates/app/src/service/media.rs crates/app/src/components/project.rs crates/app/src/app.rs crates/app/assets/timeline.js crates/app/assets/main.css
git commit -m "feat(app): playback bridge, timeline canvas and ranged media protocol"
```

---

### Task 8: Video split view and synchronized preview

**Files:**
- Modify: `crates/app/src/components/project.rs`, `crates/app/src/components/transcript.rs`, `crates/app/assets/main.css`

**Interfaces:**
- Consumes: `MediaKind::Video`, `MediaAsset::Proxy`, `Playback`.
- Produces: `ProjectBody { kind }` layout: audio = full-width transcript; video = 50/50 transcript + `<video>`; one player element only.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn video_layout_is_half_and_half() {
    let g = project_grid(MediaKind::Video);
    assert_eq!(g.transcript, "minmax(0,1fr)");
    assert_eq!(g.preview, "minmax(0,1fr)");
    assert_eq!(project_grid(MediaKind::Audio).preview, "0px");
}
```

- [ ] **Step 2: Run — FAIL**

Run: `cargo test -p ustt-app video_layout_is_half_and_half`
Expected: FAIL.

- [ ] **Step 3: Implement**

`project_grid(kind)` returns CSS grid-template-columns strings used inline (no duplicated layout rules). For video, render `<video id="player" src=proxy>` in the right pane, `object-fit: contain`, black background, native controls hidden; transport from Task 7 drives it. Clicking the transcript/timeline seeks video; playback events update both the active row and the timeline playhead. If the proxy is missing, show “Прокси недоступен” with the original path and keep the transcript usable.

- [ ] **Step 4: PASS**

Run: `cargo test -p ustt-app video_layout_is_half_and_half`.
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/components/project.rs crates/app/src/components/transcript.rs crates/app/assets/main.css
git commit -m "feat(app): video split view with synchronized preview"
```

---

### Task 9: Settings, project switching, Save As and error surfaces

**Files:**
- Create: `crates/app/src/components/settings.rs`
- Modify: `crates/app/src/app.rs`, `crates/app/src/service/native.rs`, `crates/app/src/service/mod.rs`, `crates/app/src/components/welcome.rs`, `crates/app/assets/main.css`

**Interfaces:**
- Consumes: `Settings`, `BackendPreference`, `detect`, `verify_models`, `RecentStore`, `ProjectStore`.
- Produces: `SettingsScreen { on_close }`; `save_as(project_dir) -> Result<StoreHandle>`; `switch_project(path)`; `error_banner(message)`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn missing_project_shows_error() {
    let err = load_project_error(Path::new("/definitely/missing.ustt"));
    assert!(err.contains("project.json") || err.contains("не найден") || err.contains("missing"));
}
```

- [ ] **Step 2: Run — FAIL**

Run: `cargo test -p ustt-app missing_project_shows_error`
Expected: FAIL.

- [ ] **Step 3: Implement**

Settings: language RU/EN/system, theme dark/light/system, backend auto/cpu/vulkan/cuda (with detected GPU + availability labels), threads, autosave seconds, models dir with per-file check from `verify_models`, FFmpeg dir. Save writes atomically and applies theme/lang immediately.

Project switcher: header dropdown of recent entries; choosing saves the current project, opens the target, replaces history/store, resets playback. Save As uses `rfd::FileDialog::pick_folder`, creates `<chosen>/<slug>.ustt`, copies `media/` files, saves, updates recent.

Error surfaces: `App` keeps `error: Signal<Option<String>>`; welcome/processing/project render a dismissible banner. `load_project_error` produces a UI-ready Russian/English message from `StoreError`.

- [ ] **Step 4: PASS**

Run: `cargo test -p ustt-app missing_project_shows_error`.
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/components/settings.rs crates/app/src/app.rs crates/app/src/service crates/app/src/components/welcome.rs crates/app/assets/main.css
git commit -m "feat(app): settings, project switching, save as and error surfaces"
```

---

### Task 10: Playwright harness and end-to-end UI scenarios

**Files:**
- Create: `package.json`, `playwright.config.ts`, `scripts/serve-web.sh`, `tests/ui/welcome.spec.ts`, `tests/ui/project.spec.ts`, `tests/ui/playback.spec.ts`
- Modify: `.gitignore`, `README.md`

**Interfaces:**
- Consumes: web build with `MockService`, `data-testid` attributes from Tasks 4–9.
- Produces: `npm test` runs Playwright against `http://127.0.0.1:4173`.

- [ ] **Step 1: Harness**

`serve-web.sh`: run `dx build --platform web`, locate the newest `target/dx/*/*/web/public`, serve it with `python3 -m http.server 4173 --directory "$public"`. `playwright.config.ts` uses `webServer: { command: "bash scripts/serve-web.sh", url: "http://127.0.0.1:4173", reuseExistingServer: true }`, Chromium only, baseURL.

- [ ] **Step 2: Write failing specs**

`welcome.spec.ts`: dropzone and “Открыть демо” visible; click demo → project screen; two speaker chips.
`project.spec.ts`: rename speaker, assert chip label; `Control+z` restores; edit a segment (double-click, type, Enter), assert text; save badge switches to “Сохранено”.
`playback.spec.ts`: click a transcript row → `#player` currentTime within ±0.3 s of segment start; click timeline at 50% → currentTime ≈ duration/2; press Space → paused/playing toggles; video demo shows `.preview` with `<video>`.

Run: `npm test`
Expected: FAIL (selectors/implementation missing).

- [ ] **Step 3: Implement test ids and behavior gaps**

Add `data-testid` where the plan names them: `dropzone`, `demo-audio`, `demo-video`, `speaker-chip`, `speaker-name-input`, `segment-row`, `segment-text`, `save-state`, `timeline`, `playhead`, `player`, `preview`. Fix only behaviors the specs expose.

- [ ] **Step 4: PASS**

Run: `npm test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add package.json package-lock.json playwright.config.ts scripts/serve-web.sh tests/ui .gitignore README.md
git commit -m "test(ui): playwright scenarios for welcome, editing and playback"
```

---

### Task 11: Developer docs, build config and final checks

**Files:**
- Modify: `README.md`, `crates/app/Dioxus.toml`, `.gitignore`, `scripts/install-tools.sh`

**Interfaces:**
- Produces: reproducible commands for `dx`, wasm target, Playwright; Windows build instructions; documented Linux web lane.

- [ ] **Step 1: README sections**

Add “Desktop UI (development)”, “Web target + Playwright”, “Windows build (`dx build --platform desktop --release`)”, “Keyboard shortcuts”, “Current scope: export/packaging next”. State that Windows artifacts and CI are the next plan.

- [ ] **Step 2: Final verification**

Run: `cargo test --workspace`
Run: `cargo fmt --all --check`
Run: `cargo clippy --workspace --all-targets -- -D warnings`
Run: `npm test`
Expected: all green.

- [ ] **Step 3: Commit**

```bash
git add README.md crates/app/Dioxus.toml .gitignore scripts/install-tools.sh
git commit -m "docs: desktop UI development and verification"
```

---

## Self-Review

**Spec coverage:** §9.1 screens 1–4 and 6 (export modal deferred by scope), §9.2 timeline, §9.3 transcript, §9.4 player, §9.5 design, §13.1 UI tests. Export (§10), packaging/CI (§12), Windows acceptance (§13.2) remain in later plans.

**Type consistency:** `Playback` lives only in `ustt-ui-model`; `Service` is the only I/O boundary; `StoreHandle` hides native/web differences; `MediaAsset` names match native tokens (`audio`, `proxy`, `waveform`); `project_grid` is the single layout source.

**Review focus tests:** memory (Task 6/7), undo/autosave (Task 6), early seek (Task 1/7), native errors (Task 3/9), drop/protocol safety (Task 7).

**Proportion:** tasks carry signatures and tests; no component body is transcribed beyond what the test fixes.
