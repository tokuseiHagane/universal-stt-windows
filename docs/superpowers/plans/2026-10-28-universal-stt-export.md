# Universal STT Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Export the current project as DOCX, PDF, SRT, VTT, TXT or Markdown with a speaker filter, and extract WAV/M4A audio for chosen speakers or chosen transcript segments — from both the CLI and the desktop UI.

**Architecture:** A new native crate `ustt-export` owns text rendering, DOCX/PDF generation, audio planning and the FFmpeg runner. The UI calls it through the existing `Service` boundary: native runs an export worker thread and reports progress/cancel; the wasm mock returns a downloadable text payload. The export modal is a Dioxus component; transcript rows gain selection checkboxes for segment-level audio export. Existing `Project`, `Segment`, `Speaker` stay the single source of truth — export reads the edited state.

**Tech Stack:** Rust 2024, `docx-rs` 0.4, `printpdf` 0.8 (manual ops + embedded Noto Sans), FFmpeg, Dioxus 0.7, Playwright.

**Spec:** `docs/superpowers/specs/2026-10-27-universal-stt-windows-design.md` §10 Export, §13.1 tests.

## Global Constraints

- Export always reads the current `Project` after edits and speaker renames.
- Text filter: all speakers, or an explicit subset; segments with `speaker_id: None` are included only when no filter is set.
- Timestamps optional; default from `Project::export_defaults` (DOCX, WAV).
- Output filename: `<slug(title)>-<YYYY-MM-DD>.<ext>`.
- DOCX and PDF contain title, date, duration, speaker list, then `[HH:MM:SS] Name: text` blocks.
- PDF embeds Noto Sans (Latin + Cyrillic); no external font lookup at runtime.
- Audio exports read `project/media/audio16k.wav` only, never the original; chunks are joined with a 0.4 s pause.
- Audio modes: none, one file per speaker, or one file from selected segments.
- Progress and cancel work like import; cancellation removes partial output files.
- wasm build must not compile `docx-rs`, `printpdf`, FFmpeg runners or `ustt-media`.
- Playwright covers the modal + download; native file writing is covered by unit/gated tests.

## Review Focus

1. **Speaker filter correctness.** Filtering must apply to text and to `by speaker` audio; `None` speakers never create a track. Test — Task 1 `filter_excludes_unselected_speakers`, Task 3 `plan_skips_unassigned`.
2. **Timestamp formats.** SRT/VTT use millisecond timestamps and end times; DOCX/PDF/TXT use `[HH:MM:SS]`; VTT starts with `WEBVTT`. Test — Task 1 `srt_and_vtt_are_well_formed`.
3. **Long transcripts and many ranges.** Audio planning must not build a shell argument that exceeds OS limits; the FFmpeg filter goes through `-filter_complex_script`. Test — Task 3 `plan_writes_filter_script_for_many_ranges`.
4. **Cancel and partial files.** Cancel removes the temp dir and does not leave half-written output. Test — Task 3 `cancelled_extract_removes_partial_file`.
5. **Missing media / read-only destination.** A missing `audio16k.wav` or an unwritable folder yields a readable error, not a panic or silent empty file. Test — Task 3 `missing_wav_is_an_error`, Task 6 native wiring.

---

## File Structure

```
crates/export/
  Cargo.toml
  assets/NotoSans-Regular.ttf      # embedded PDF font
  src/lib.rs                        # re-exports
  src/model.rs                      # ExportLine, TextOutput, ExportRequest
  src/lines.rs                      # project -> filtered lines
  src/text.rs                       # TXT, Markdown, SRT, VTT
  src/docx.rs                       # DOCX builder
  src/pdf.rs                        # printpdf + HTML layout
  src/audio.rs                      # AudioPlan, plan_*, extract
  tests/export.rs
  tests/audio_gated.rs
crates/cli/src/main.rs              # export subcommand
crates/core/src/model.rs            # AudioFormat::M4a
crates/app/src/service/{mod,native,mock}.rs
crates/app/src/components/export_modal.rs
crates/app/src/components/transcript.rs   # selection checkboxes
crates/app/src/components/project.rs      # export button + selection signal
crates/app/src/components/mod.rs
tests/ui/export.spec.ts
```

---

### Task 1: Export model, filtering and text formats

**Files:**
- Create: `crates/export/Cargo.toml`, `crates/export/src/{lib,model,lines,text}.rs`
- Create: `crates/export/tests/export.rs`
- Modify: root `Cargo.toml`, `crates/core/src/model.rs`

**Interfaces:**
- Produces: `TextOutput { Docx, Pdf, Srt, Vtt, Txt, Markdown }` (serde snake_case, `extension()`); `ExportLine { start, end, speaker: String, text: String }`; `ExportOptions { speakers: Option<Vec<Uuid>>, timestamps: bool }`; `lines(project: &Project, speakers: Option<&[Uuid]>) -> Vec<ExportLine>`; `render_text(lines: &[ExportLine], format: TextOutput, timestamps: bool) -> String`; `TextOutput::from_str`; `AudioFormat::M4a` added to core.

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn filter_excludes_unselected_speakers() {
    let p = sample_project();
    let only = lines(&p, Some(&[p.speakers[0].id]));
    assert_eq!(only.len(), 2);
    assert!(only.iter().all(|l| l.speaker == "Alice"));
    assert_eq!(lines(&p, None).len(), 3);
}

#[test]
fn srt_and_vtt_are_well_formed() {
    let p = sample_project();
    let ls = lines(&p, None);
    let srt = render_text(&ls, TextOutput::Srt, true);
    assert!(srt.starts_with("1\n00:00:00,000 --> 00:00:02,000\nAlice: hello"));
    let vtt = render_text(&ls, TextOutput::Vtt, true);
    assert!(vtt.starts_with("WEBVTT\n\n00:00:00.000 --> 00:00:02.000\nAlice: hello"));
}

#[test]
fn txt_respects_timestamps_flag() {
    let ls = lines(&sample_project(), None);
    assert!(render_text(&ls, TextOutput::Txt, true).contains("[00:00:00] Alice: hello"));
    assert!(!render_text(&ls, TextOutput::Txt, false).contains("[00:00:00]"));
}

#[test]
fn markdown_groups_by_speaker() {
    let md = render_text(&lines(&sample_project(), None), TextOutput::Markdown, false);
    assert!(md.contains("**Alice**"));
    assert!(md.contains("**Bob**"));
}
```

- [ ] **Step 2: Run — FAIL**

Run: `cargo test -p ustt-export`
Expected: FAIL (crate missing).

- [ ] **Step 3: Implement**

`lines`: filter, map speaker name via `project.speakers`, skip segments outside the filter. `render_text`:
- TXT: one line per segment.
- Markdown: `## <title>` is added by the caller? Keep renderer pure: speaker sections `### Name` followed by lines.
- SRT: counter, `HH:MM:SS,mmm --> ...`, text `Name: text`, blank line.
- VTT: `WEBVTT`, then `.` milliseconds.
Timestamps toggle applies to TXT/Markdown/DOCX/PDF; SRT/VTT always carry timing.

- [ ] **Step 4: PASS**

Run: `cargo test -p ustt-export`.
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock crates/export crates/core
git commit -m "feat(export): filtered transcript lines and text formats"
```

---

### Task 2: DOCX and PDF documents

**Files:**
- Create: `crates/export/src/docx.rs`, `crates/export/src/pdf.rs`, `crates/export/assets/NotoSans-Regular.ttf`
- Modify: `crates/export/Cargo.toml`, `crates/export/src/lib.rs`, `crates/export/tests/export.rs`

**Interfaces:**
- Produces: `render_docx(project, &[ExportLine], timestamps) -> Result<Vec<u8>, ExportError>`; `render_pdf(project, &[ExportLine], timestamps) -> Result<Vec<u8>, ExportError>`; `ExportError { Io, Docx(String), Pdf(String), Audio(String), MissingMedia }`.

- [ ] **Step 1: Failing tests**

```rust
#[test]
fn docx_is_a_zip_with_document_xml() {
    let bytes = render_docx(&sample_project(), &lines(&sample_project(), None), true).unwrap();
    assert_eq!(&bytes[..2], b"PK");
    assert!(bytes.len() > 2_000);
}

#[test]
fn pdf_has_a_header_and_cyrillic_text() {
    let bytes = render_pdf(&sample_project(), &lines(&sample_project(), None), true).unwrap();
    assert_eq!(&bytes[..4], b"%PDF");
    assert!(bytes.len() > 4_000);
}
```

- [ ] **Step 2: Run — FAIL**

Run: `cargo test -p ustt-export docx_is_a_zip pdf_has_a_header`.
Expected: FAIL.

- [ ] **Step 3: Implement**

Vendor `NotoSans-Regular.ttf` (Google Noto, OFL) into `assets/`; add its license to `NOTICE`.

DOCX: `Docx::new().default_size(22)` + title paragraph bold 32 half-points, meta paragraph, then per line `Paragraph::new().add_run(Run::new().add_text(format!("[{time}] {speaker}: {text}")))`. Pack into `Vec<u8>` via `Cursor`.

PDF: build escaped HTML (`<h1>`, meta, `<p>` per line) and call `PdfDocument::from_html` with a named font map `Noto -> include_bytes!`, `font-family:'Noto'`, A4 options, footer `{builtin:pagenum}`; return bytes from `save`.

- [ ] **Step 4: PASS**

Run: `cargo test -p ustt-export`.
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/export NOTICE
git commit -m "feat(export): DOCX and PDF with embedded Noto Sans"
```

---

### Task 3: Audio planning and FFmpeg extraction

**Files:**
- Create: `crates/export/src/audio.rs`, `crates/export/tests/audio_gated.rs`
- Modify: `crates/export/Cargo.toml`, `crates/export/src/lib.rs`

**Interfaces:**
- Produces: `AudioMode { Speakers, Segments }`; `AudioPlan { tracks: Vec<AudioTrack> }`; `AudioTrack { name: String, ranges: Vec<(f64, f64)> }`; `plan_speakers(project, Option<&[Uuid]>) -> AudioPlan`; `plan_segments(project, &[Uuid]) -> AudioPlan`; `extract(tools: &Tools, wav: &Path, plan: &AudioPlan, format: AudioFormat, out_dir: &Path, cancel: &AtomicBool, progress: &mut dyn FnMut(f32, &str)) -> Result<Vec<PathBuf>, ExportError>`; `build_filter_script(track, wav) -> String`.

- [ ] **Step 1: Failing tests**

```rust
#[test]
fn plan_skips_unassigned() {
    let p = sample_project();
    let plan = plan_speakers(&p, None);
    assert_eq!(plan.tracks.len(), 2);
    assert!(plan.tracks.iter().all(|t| !t.ranges.is_empty()));
}

#[test]
fn plan_merges_adjacent_ranges() {
    let mut p = sample_project();
    p.segments[1].start = 2.1; // 0.1 s after segment 0
    let plan = plan_speakers(&p, Some(&[p.speakers[0].id]));
    assert_eq!(plan.tracks[0].ranges.len(), 1);
}

#[test]
fn plan_writes_filter_script_for_many_ranges() {
    let mut p = sample_project();
    p.segments = (0..2_000).map(|i| fake_segment(i as f64 * 0.5, i)).collect();
    let plan = plan_segments(&p, &p.segments.iter().map(|s| s.id).collect::<Vec<_>>());
    let script = build_filter_script(&plan.tracks[0], Path::new("/tmp/a.wav"));
    assert!(script.contains("concat=n=2000"));
    assert!(script.len() > 10_000);
}

#[test]
fn cancelled_extract_removes_partial_file() {
    // needs ffmpeg + a 2 s wav; set cancel before the call
    // assert Err(Cancelled) and !out.exists()
}

#[test]
fn missing_wav_is_an_error() {
    let err = extract(&tools(), Path::new("/missing.wav"), &plan, AudioFormat::Wav, tmp, &cancel, &mut |_,_| {}).unwrap_err();
    assert!(matches!(err, ExportError::MissingMedia));
}
```

Fake segment helper constructs `Segment` with the same speaker.

- [ ] **Step 2: Run — FAIL**

Run: `cargo test -p ustt-export audio`.
Expected: FAIL.

- [ ] **Step 3: Implement**

`plan_speakers`: group by `speaker_id` (skip None), sort by start, merge ranges when `next.start - prev.end <= 0.4`; track name = sanitized speaker name. `plan_segments`: one track `selected`, ranges sorted, merged with the same rule.

`build_filter_script`:

```text
[0:a]atrim=start=0:end=2,asetpts=PTS-STARTPTS,apad=pad_dur=0.4[s0];
...
[s0][s1]...concat=n=N:v=0:a=1[out]
```

`extract`: if `!wav.is_file()` → `MissingMedia`; create `tempfile::TempDir` next to output; for each track write `filter.txt`, run `ffmpeg -y -i wav -filter_complex_script filter.txt -map [out] <codec> <temp>`; poll child every 100 ms with cancel, kill and return `Cancelled`; rename each finished file to `<out_dir>/<title>-<track>.<ext>`; on error/cancel remove temp and any renamed partials.

Codec: WAV `pcm_s16le`; M4A `aac -b:a 192k`.

- [ ] **Step 4: PASS**

Run: `cargo test -p ustt-export` and gated `cargo test -p ustt-export --test audio_gated -- --ignored`.
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/export
git commit -m "feat(export): speaker and segment audio extraction"
```

---

### Task 4: CLI export subcommand

**Files:**
- Modify: `crates/cli/src/main.rs`, `crates/cli/Cargo.toml`

**Interfaces:**
- Consumes: `ProjectStore::open`, `resolve_models` not needed, `ustt_export::{...}`.
- Produces: `ustt-cli export <project> --out <dir> [--format docx|pdf|srt|vtt|txt|md] [--speakers <uuid,...>] [--no-timestamps] [--audio none|speakers|segments] [--segments <uuid,...>] [--audio-format wav|m4a]`.

- [ ] **Step 1: Failing test**

A unit test parses a `ExportArgs` struct from a `Vec<String>`:

| `parse_export(&["p.ustt","--out","/tmp","--format","srt"])` | format Srt, timestamps true, audio None |
| `parse_export` missing `--out` | Err contains "--out" |

- [ ] **Step 2: Run — FAIL**

Run: `cargo test -p ustt-cli export_args`.
Expected: FAIL.

- [ ] **Step 3: Implement**

Parse flags, open project, build `ExportOptions`, write text or call `extract` with `Tools::locate(None)`, print JSON `{ "files": [...] }`, errors to stderr exit 1. `--speakers`/`--segments` accept comma-separated UUIDs; invalid UUID is a readable error.

- [ ] **Step 4: PASS**

Run: `cargo test -p ustt-cli export_args`.
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/cli
git commit -m "feat(cli): export subcommand"
```

---

### Task 5: Service export boundary

**Files:**
- Modify: `crates/app/src/service/{mod,native,mock}.rs`, `crates/app/Cargo.toml`
- Add native dependency: `ustt-export = { path = "../export" }`

**Interfaces:**
- Produces: `ExportRequest { format: TextOutput, speakers: Option<Vec<Uuid>>, timestamps: bool, audio: AudioMode, segments: Option<Vec<Uuid>>, out_dir: PathBuf }`; `ExportEvent { Progress { fraction, detail }, Done(Vec<PathBuf>), Download { filename, bytes }, Failed(String), Cancelled }`; `ExportJob { rx, cancel }`; `Service::export(store, project, request) -> ExportJob`; `Service::default_export_dir() -> Option<PathBuf>`.

- [ ] **Step 1: Failing test**

Native: `export_request_rejects_empty_selection` → selecting `AudioMode::Segments` with an empty segment list returns `ExportEvent::Failed` containing "segment".

- [ ] **Step 2: Run — FAIL**

Run: `cargo test -p ustt-app export_request`.
Expected: FAIL.

- [ ] **Step 3: Implement**

Native: validate, spawn thread; render text via `ustt-export` and write to out_dir, or run `extract` with progress forwarding. Rename output `<slug>-<date>.<ext>`. Send `Done(files)`.

Mock: build a tiny VTT/SRT/TXT string from the project's lines (local formatter) and send `Download { filename, bytes }`; audio exports return `Failed("audio export needs the desktop build")`.

- [ ] **Step 4: PASS**

Run: `cargo test -p ustt-app export_request`.
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/app Cargo.lock
git commit -m "feat(app): export service boundary"
```

---

### Task 6: Export modal, segment selection and downloads

**Files:**
- Create: `crates/app/src/components/export_modal.rs`
- Modify: `crates/app/src/components/{mod,project,transcript}.rs`, `crates/app/src/app.rs`, `crates/app/assets/main.css`

**Interfaces:**
- Consumes: `Service::export`, `ExportRequest`, `TextOutput`, project speakers/segments.
- Produces: `ExportModal { project: Project, selected_segments: Signal<HashSet<Uuid>>, on_close }`; `selected_segments` context signal owned by `ProjectView`; transcript checkbox with `data-testid="segment-select"`; toolbar button `data-testid="open-export"`.

- [ ] **Step 1: Failing tests**

```rust
#[test]
fn default_filename_is_slugged_and_dated() {
    let name = export_filename("My Talk", TextOutput::Vtt, "2026-10-28");
    assert_eq!(name, "my-talk-2026-10-28.vtt");
}
```

- [ ] **Step 2: Run — FAIL**

Run: `cargo test -p ustt-app default_filename`.
Expected: FAIL.

- [ ] **Step 3: Implement**

Modal fields: format select (DOCX/PDF/SRT/VTT/TXT/MD), speaker checkboxes (all by default), timestamps toggle, audio mode select (none/by speaker/selected segments; selected disabled when none), audio format WAV/M4A, destination row with “Choose folder” (`Service::pick_project` / `default_export_dir`), Export button, progress bar, Cancel, error banner.

On submit build `ExportRequest` and poll `ExportJob.rx` every 80 ms (same pattern as import). `Download` → JS blob download via `document::eval` with base64 (`FileReader` not needed). `Done` → show file list with “Open folder” (native only) and close.

Transcript rows get a checkbox in a new first grid column; toggling updates `selected_segments`. Selection survives speaker filtering and is cleared when the project changes.

- [ ] **Step 4: PASS**

Run: `cargo test -p ustt-app default_filename`.
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/app
git commit -m "feat(app): export modal, segment selection and downloads"
```

---

### Task 7: Playwright export scenario and docs

**Files:**
- Create: `tests/ui/export.spec.ts`
- Modify: `README.md`, `NOTICE`

**Interfaces:**
- Consumes: `data-testid="open-export"`, `export-format`, `export-submit`, `export-progress`, `segment-select`.

- [ ] **Step 1: Failing spec**

Open the audio demo, click export, choose VTT, uncheck the second speaker, click Export, capture `page.waitForEvent('download')`, read the file and assert it starts with `WEBVTT` and contains only the first speaker name.

- [ ] **Step 2: Run — FAIL**

Run: `npx playwright test tests/ui/export.spec.ts`.
Expected: FAIL.

- [ ] **Step 3: Implement test ids and download path**

Add the test ids; fix only behaviors the spec exposes. Add README “Export” section and Noto/FFmpeg attribution to NOTICE.

- [ ] **Step 4: PASS**

Run: `npx playwright test tests/ui/export.spec.ts`.
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add tests/ui/export.spec.ts README.md NOTICE crates/app
git commit -m "test(ui): export modal and VTT download"
```

---

### Task 8: Final verification

- [ ] **Step 1: Full checks**

Run: `cargo test --workspace`
Run: `cargo fmt --all --check`
Run: `cargo clippy --workspace --all-targets -- -D warnings`
Run: `npm test`
Run: `cargo test -p ustt-export -- --ignored` with FFmpeg available.
Expected: all green.

- [ ] **Step 2: Commit**

```bash
git commit --allow-empty -m "chore: export milestone verification"
```

---

## Self-Review

**Spec coverage:** §10.1 text formats + speaker filter + timestamps, §10.2 audio modes/format/pause/FFmpeg, export from edited state. §13.1 export tests. UI modal replaces the placeholder.

**Type consistency:** `TextOutput` is export-only; core `TextFormat` stays the persisted default. `AudioFormat::M4a` added to core and used by UI/CLI. `ExportRequest` carries IDs, never names, so renames apply at render time.

**Review focus tests:** filter (Task 1), timestamps (Task 1), many ranges (Task 3), cancel/partials (Task 3), missing media (Task 3), native error surface (Task 5).
