# Universal STT: ядро и движки — план реализации

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Собрать headless-ядро приложения: импорт любого медиафайла → Who-диаризация → Parakeet Redux-транскрипция → сохранённый проект `.ustt/`, с тестами на Linux CPU.

**Architecture:** Rust-workspace из шести крейтов. Домен (`ustt-core`) не знает про FFmpeg и модели. `ustt-media` оборачивает ffmpeg/ffprobe. `ustt-diarize` — порт пайплайна Who с ONNX Runtime. `ustt-asr` — FFI к `parakeet.cpp` через `libloading`. `ustt-pipeline` связывает шаги и считает прогресс. `ustt-cli` — отладочный вход и приёмочный harness.

**Tech Stack:** Rust 2024, `serde`/`serde_json`, `uuid`, `chrono`, `thiserror`, `hound`, `rustfft`, `ort` 2.0.0-rc.15 (`load-dynamic`), `libloading`, `ash`, `sha2`, `tempfile`.

**Spec:** `docs/superpowers/specs/2026-10-27-universal-stt-windows-design.md`

**Scope:** этот план — только ядро, движки и headless-пайплайн. Dioxus-UI, экспорт DOCX/PDF/аудио, инсталлятор и GitHub Actions планируются отдельными планами после приёмки этого.

## Global Constraints

- Целевая платформа — Windows 10/11 x64, MSVC; разработка и тесты — Linux x86_64.
- В рантайме нет Python, Node, ffmpeg в PATH, CUDA Toolkit. Все бинарники лежат рядом с приложением.
- Частота для моделей — 16 000 Гц, моно, f32 PCM. Исходники декодируются FFmpeg.
- Who: `desert-ant-labs/who` revision `097d29a140dd19e220cff132e455e515d0e47e59`, каталог `onnx-int8/`.
- Parakeet: `parakeet.cpp` v0.6.1, C API ABI 10; модели `mudler/parakeet-cpp-gguf` revision `c77726e46ceaab96b5a5a3328915edb44c73209d`.
- Redux CPU — `redux-packed.gguf` (213 МБ); GPU — `redux-q8_0.gguf` (942 МБ); VAD — `silero-vad-f16.gguf`.
- Схема проекта — `schema: 1`. Каталог проекта `<slug>.ustt/` с `project.json`, `media/audio16k.wav`, `media/waveform.bin`, `media/proxy.mp4|m4a`.
- Волна — бакеты 20 мс, `min`/`max` f32.
- Сегменты: пауза > 0.6 с или пунктуация или 15 с → граница; спикер сегмента — максимальное перекрытие с turns Who; нет перекрытия → `None`.
- Who: пороги и правило читаются из `who_meta.json` (сейчас presence 0.8, mask 0.5, rule `mask`, pool 1); куски по 300 с; join cosine 0.65, min solo 1.0 с.
- Undo — снимки, глубина 100. Автосохранение — 5 с (UI-план).
- Код — MIT. Комментарии только про инварианты и неочевидные решения.
- `cargo fmt --check` и `cargo clippy --workspace --all-targets -- -D warnings` — обязательны перед каждым коммитом.

## Review Focus

1. **Трёхчасовой файл.** Память не должна расти с длиной: WAV и волна читаются потоково, log-mel считается кусками, в памяти живёт один кусок. Тест — Task 6 (`waveform_from_wav_1h_is_streaming`) и Task 10 (`diarize_700s_uses_two_pieces`).
2. **Тишина, музыка, речь без пауз.** VAD может вернуть пусто; приложение обязано дать пустой проект без падения и без «фантомных» сегментов. Тест — Task 12 (`empty_vad_falls_back_to_fixed_windows`, `silence_yields_no_segments`).
3. **Нестандартный звук.** 8/44.1/48/96 кГц, стерео, битый или обрезанный файл. Декодер приводит к 16 кГц моно; ошибка FFmpeg — понятная ошибка, временные файлы удалены. Тест — Task 5 (`probe_rejects_truncated_file`), Task 6 (`decode_resamples_48k_stereo`).
4. **Перекрытия и больше четырёх спикеров.** Turns могут пересекаться; сегмент получает спикера с максимальным перекрытием, при равенстве — первого по времени; без перекрытия — `None`. Тест — Task 13 (`overlapping_turns_pick_max_overlap`, `no_overlap_leaves_speaker_none`).
5. **Отмена и закрытие посреди обработки.** Отмена между кусками, удаление temp, проект не создаётся наполовину. Тест — Task 15 (`cancel_leaves_no_partial_project`).

---

## File Structure

```
Cargo.toml                       workspace
rust-toolchain.toml
.gitignore
LICENSE
README.md
crates/core/                     ustt-core: домен, история, store, настройки
  src/{lib,model,color,history,paths,settings,recent,store}.rs
crates/media/                    ustt-media: ffmpeg/ffprobe, wav, волна, прокси
  src/{lib,tools,probe,decode,waveform,proxy,wav}.rs
crates/diarize/                  ustt-diarize: Who
  src/{lib,meta,frontend,dft,banks,session,logits,decision,pieces,linker,embedding,voices,cluster}.rs
crates/asr/                      ustt-asr: parakeet.cpp FFI, VAD, склейка
  src/{lib,ffi,json,vad,stitch}.rs
crates/pipeline/                 ustt-pipeline: бэкенды, модели, оркестрация
  src/{lib,backend,models,merge,progress,import}.rs
crates/cli/                      ustt-cli
  src/main.rs
scripts/fetch-who.sh
scripts/fetch-parakeet.sh
scripts/fetch-fixtures.sh
scripts/reference/who_frontend.py
tests/fixtures/                  генерируемые фикстуры, в git только JSON
```

---

### Task 1: Workspace, домен, репозиторий

**Files:**
- Create: `Cargo.toml`, `rust-toolchain.toml`, `.gitignore`, `LICENSE`, `README.md`
- Create: `crates/core/Cargo.toml`, `crates/core/src/lib.rs`, `crates/core/src/model.rs`, `crates/core/src/color.rs`
- Test: `crates/core/src/model.rs` (модульные тесты)

**Interfaces:**
- Produces: `ustt_core::{Project, SourceRef, MediaKind, Speaker, Segment, Word, Turn, Diarization, ExportDefaults, TextFormat, AudioFormat}`; `ustt_core::color::{SPEAKER_COLORS: [&str; 8], color_for_index(usize) -> &'static str}`; `ustt_core::model::speakers_from_diarization(&Diarization) -> Vec<Speaker>`.

- [ ] **Step 1: Инициализировать репозиторий и workspace**

```bash
git init
cargo new --lib crates/core --name ustt-core
```
В корневом `Cargo.toml` — `[workspace] members = ["crates/*"]`, `resolver = "2"`, workspace-зависимости: `serde = { version = "1", features = ["derive"] }`, `serde_json = "1"`, `uuid = { version = "1", features = ["v4", "serde"] }`, `chrono = { version = "0.4", features = ["serde"] }`, `sha2 = "0.10"`, `thiserror = "2"`. В `rust-toolchain.toml` — `channel = "stable"`, компоненты `rustfmt`, `clippy`.

- [ ] **Step 2: Написать падающие тесты модели**

```rust
#[test]
fn project_json_roundtrip() {
    let mut p = Project::new("Talk", sample_source(), MediaKind::Video, 12.5);
    p.speakers = speakers_from_diarization(&Diarization {
        turns: vec![Turn { start: 0.0, end: 2.0, speaker_index: 1 }],
        speaker_count: 2,
        duration: 12.5,
    });
    p.segments.push(Segment {
        id: Uuid::new_v4(), start: 0.0, end: 2.0,
        text: "hello".into(), speaker_id: Some(p.speakers[1].id),
        words: vec![Word { start: 0.0, end: 0.4, text: "hello".into(), confidence: Some(0.9) }],
    });
    let json = serde_json::to_string(&p).unwrap();
    let back: Project = serde_json::from_str(&json).unwrap();
    assert_eq!(back, p);
    assert_eq!(back.schema, 1);
}

#[test]
fn speakers_from_diarization_orders_by_first_appearance() {
    let d = Diarization { turns: vec![
        Turn { start: 5.0, end: 6.0, speaker_index: 3 },
        Turn { start: 1.0, end: 2.0, speaker_index: 3 },
        Turn { start: 2.5, end: 3.0, speaker_index: 0 },
    ], speaker_count: 4, duration: 10.0 };
    let s = speakers_from_diarization(&d);
    assert_eq!(s.len(), 4);
    assert_eq!(s[0].name, "Speaker 1");
    assert_eq!(s[0].color, 0);
    assert_eq!(s[3].color, 3);
}

#[test]
fn speaker_colors_cycle_after_eight() {
    assert_eq!(SPEAKER_COLORS.len(), 8);
    assert_eq!(color_for_index(8), SPEAKER_COLORS[0]);
}
```

- [ ] **Step 3: Запустить тесты — убедиться, что не компилируется**

Run: `cargo test -p ustt-core`
Expected: FAIL — типы не определены.

- [ ] **Step 4: Реализовать модель**

`Project::new` ставит `schema: 1`, `id: Uuid::new_v4()`, `created_at = modified_at = Utc::now()`, пустые `speakers`/`segments`/`turns`, `language: None`, `export_defaults: ExportDefaults::default()`. `speakers_from_diarization` создаёт `speaker_count` спикеров с именами `Speaker {i+1}` и цветом `i % 8`, порядок — по `speaker_index` (Who уже нумерует по первому появлению). `SourceRef::new(path)` считает `size_bytes`, `modified_unix`, `fingerprint` = SHA-256 первых и последних 1 МиБ (для файлов ≤2 МиБ — всего файла).

- [ ] **Step 5: Тесты, fmt, clippy, коммит**

Run: `cargo test -p ustt-core && cargo fmt --all --check && cargo clippy -p ustt-core -- -D warnings`
Expected: PASS.
```bash
git add Cargo.toml rust-toolchain.toml .gitignore LICENSE README.md crates/core
git commit -m "feat(core): domain model and workspace scaffold"
```

---

### Task 2: История правок

**Files:**
- Create: `crates/core/src/history.rs`
- Modify: `crates/core/src/lib.rs`

**Interfaces:**
- Produces: `History<T: Clone + PartialEq>` с `new(initial, capacity)`, `current() -> &T`, `commit(next)`, `undo() -> bool`, `redo() -> bool`, `can_undo() -> bool`, `can_redo() -> bool`, `is_dirty() -> bool`, `mark_saved()`.

- [ ] **Step 1: Тесты**

```rust
#[test]
fn undo_redo_roundtrip() {
    let mut h = History::new(0u32, 100);
    h.commit(1); h.commit(2);
    assert!(h.is_dirty());
    assert!(h.undo()); assert_eq!(*h.current(), 1);
    assert!(h.undo()); assert_eq!(*h.current(), 0);
    assert!(!h.undo());
    assert!(h.redo()); assert_eq!(*h.current(), 1);
}

#[test]
fn capacity_drops_oldest() {
    let mut h = History::new(0u32, 2);
    h.commit(1); h.commit(2); h.commit(3);
    assert_eq!(*h.current(), 3);
    assert!(h.undo()); assert!(h.undo());
    assert!(!h.undo());
    assert_eq!(*h.current(), 1);
}

#[test]
fn mark_saved_clears_dirty_and_is_idempotent() {
    let mut h = History::new(0u32, 10);
    h.commit(1); h.mark_saved();
    assert!(!h.is_dirty());
    h.mark_saved();
    assert!(!h.is_dirty());
}
```

- [ ] **Step 2: Run — FAIL**

Run: `cargo test -p ustt-core history`
Expected: FAIL.

- [ ] **Step 3: Реализовать**

Вектор снимков + индекс; `commit` обрезает redo-хвост, при переполнении отбрасывает первый; `is_dirty` — `current != saved`.

- [ ] **Step 4: PASS + коммит**

Run: `cargo test -p ustt-core history`
```bash
git add crates/core
git commit -m "feat(core): snapshot undo history"
```

---

### Task 3: Пути, настройки, недавние проекты

**Files:**
- Create: `crates/core/src/paths.rs`, `crates/core/src/settings.rs`, `crates/core/src/recent.rs`
- Modify: `crates/core/src/lib.rs`, `crates/core/Cargo.toml` (dev-dep `tempfile`)

**Interfaces:**
- Produces: `config_dir_from(explicit: Option<&Path>) -> PathBuf`; `Settings { language, theme, backend, threads, autosave_secs, models_dir, ffmpeg_dir, export }` и `BackendPreference { Auto, Cpu, Vulkan, Cuda }`; `Settings::load(path)`, `Settings::save(path)`; `RecentEntry { path, title, opened_at, duration, kind }`; `RecentStore::new(file)`, `load()`, `push(entry)`, `list()`, `prune_missing()`.

- [ ] **Step 1: Тесты**

```rust
#[test]
fn settings_roundtrip_and_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("settings.json");
    let mut s = Settings::default();
    assert_eq!(s.backend, BackendPreference::Auto);
    assert_eq!(s.autosave_secs, 5);
    s.language = Some("ru".into());
    s.save(&file).unwrap();
    assert_eq!(Settings::load(&file).unwrap().language.as_deref(), Some("ru"));
}

#[test]
fn recent_caps_at_twelve_and_prunes_missing() {
    let dir = tempfile::tempdir().unwrap();
    let store_file = dir.path().join("recent.json");
    std::fs::create_dir_all(dir.path().join("p1.ustt")).unwrap();
    let mut r = RecentStore::new(store_file.clone());
    for i in 0..14 { r.push(RecentEntry::new(dir.path().join(format!("x{i}.ustt")), "x", 1.0, MediaKind::Audio)); }
    r.push(RecentEntry::new(dir.path().join("p1.ustt"), "p1", 10.0, MediaKind::Audio));
    assert_eq!(r.list().len(), 12);
    r.prune_missing();
    assert_eq!(r.list().len(), 1);
    assert_eq!(RecentStore::new(store_file).load().unwrap().list().len(), 1);
}
```

- [ ] **Step 2: Run — FAIL**

Run: `cargo test -p ustt-core recent`
Expected: FAIL.

- [ ] **Step 3: Реализовать**

`config_dir_from` без аргумента: `%APPDATA%\UniversalSTT` на Windows, `$XDG_CONFIG_HOME/universal-stt` или `~/.config/universal-stt` на Linux. Запись атомарная: `tempfile::NamedTempFile::new_in(parent)` → `write_all` → `persist`. `prune_missing` удаляет записи, где нет `project.json`.

- [ ] **Step 4: PASS + коммит**

Run: `cargo test -p ustt-core recent && cargo test -p ustt-core settings`
```bash
git add crates/core
git commit -m "feat(core): settings, config paths, recent projects"
```

---

### Task 4: Хранилище проекта

**Files:**
- Create: `crates/core/src/store.rs`
- Modify: `crates/core/src/lib.rs`

**Interfaces:**
- Produces: `slugify(&str) -> String`; `ProjectStore::create(dir, title, source, kind, duration) -> Result<Self>`; `ProjectStore::open(dir) -> Result<(Self, Project)>`; `save(&self, &mut Project) -> Result<()>`; `media_dir(&self) -> PathBuf`; `project_file(&self) -> PathBuf`.

- [ ] **Step 1: Тесты**

```rust
#[test]
fn create_save_open_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("talk.ustt");
    let (store, mut p) = ProjectStore::create(&dir, "Talk", sample_source(), MediaKind::Audio, 3.0).unwrap();
    assert!(store.media_dir().is_dir());
    p.title = "Renamed".into();
    store.save(&mut p).unwrap();
    let (_, back) = ProjectStore::open(&dir).unwrap();
    assert_eq!(back.title, "Renamed");
    assert!(back.modified_at >= back.created_at);
}

#[test]
fn save_is_atomic_and_leaves_no_tmp() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("a.ustt");
    let (store, mut p) = ProjectStore::create(&dir, "A", sample_source(), MediaKind::Audio, 1.0).unwrap();
    store.save(&mut p).unwrap();
    let leftovers: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp")).collect();
    assert!(leftovers.is_empty());
}

#[test]
fn open_rejects_wrong_schema() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("bad.ustt");
    let (store, mut p) = ProjectStore::create(&dir, "Bad", sample_source(), MediaKind::Audio, 1.0).unwrap();
    store.save(&mut p).unwrap();
    let mut v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("project.json")).unwrap()).unwrap();
    v["schema"] = 99.into();
    std::fs::write(dir.join("project.json"), v.to_string()).unwrap();
    assert!(matches!(ProjectStore::open(&dir), Err(StoreError::Schema(99))));
}

#[test]
fn slugify_keeps_cyrillic_and_strips_separators() {
    assert_eq!(slugify("  Интервью: Иван / Петя  "), "интервью-иван-петя");
}
```

- [ ] **Step 2: Run — FAIL**

Run: `cargo test -p ustt-core store`
Expected: FAIL.

- [ ] **Step 3: Реализовать**

`create` создаёт `<dir>/media`, `project.json` с пустым проектом. `save` обновляет `modified_at`, сериализует pretty JSON во временный файл рядом и делает rename поверх. `open` читает, проверяет `schema == 1`, иначе `StoreError::Schema(u32)`.

- [ ] **Step 4: PASS + коммит**

Run: `cargo test -p ustt-core store`
```bash
git add crates/core
git commit -m "feat(core): atomic project store"
```

---

### Task 5: Поиск FFmpeg и probe

**Files:**
- Create: `crates/media/Cargo.toml`, `crates/media/src/lib.rs`, `src/tools.rs`, `src/probe.rs`
- Create: `crates/media/tests/probe.rs`
- Modify: root `Cargo.toml`

**Interfaces:**
- Produces: `Tools { ffmpeg: PathBuf, ffprobe: PathBuf }`; `Tools::locate(explicit_dir: Option<&Path>) -> Result<Tools, MediaError>`; `probe(&Tools, &Path) -> Result<MediaInfo, MediaError>`; `MediaInfo { duration, video: Option<VideoStream>, audio: Option<AudioStream> }`; `MediaError`.

- [ ] **Step 1: Тесты (fixture через ffmpeg)**

```rust
fn make_mp4(dir: &Path) -> PathBuf {
    let out = dir.join("a.mp4");
    let ok = Command::new("ffmpeg").args(["-y", "-f", "lavfi", "-i", "testsrc=size=320x240:rate=25",
        "-f", "lavfi", "-i", "sine=frequency=440:sample_rate=48000", "-t", "2",
        "-c:v", "mpeg4", "-c:a", "aac", out.to_str().unwrap()]).status().unwrap().success();
    assert!(ok);
    out
}

#[test]
fn probe_reports_video_and_audio() {
    let tmp = tempfile::tempdir().unwrap();
    let tools = Tools::locate(None).unwrap();
    let info = probe(&tools, &make_mp4(tmp.path())).unwrap();
    assert!((info.duration - 2.0).abs() < 0.2);
    assert!(info.video.is_some());
    assert_eq!(info.audio.unwrap().channels, 1);
}

#[test]
fn probe_rejects_truncated_file() {
    let tmp = tempfile::tempdir().unwrap();
    let f = tmp.path().join("bad.mp4");
    std::fs::write(&f, b"not a video at all").unwrap();
    assert!(matches!(probe(&Tools::locate(None).unwrap(), &f), Err(MediaError::Probe(_))));
}

#[test]
fn locate_honours_explicit_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let exe = if cfg!(windows) { ".exe" } else { "" };
    for name in ["ffmpeg", "ffprobe"] {
        let p = tmp.path().join(format!("{name}{exe}"));
        std::fs::write(&p, b"#!/bin/sh\n").unwrap();
        #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap(); }
    }
    let tools = Tools::locate(Some(tmp.path())).unwrap();
    assert_eq!(tools.ffmpeg, tmp.path().join(format!("ffmpeg{exe}")));
}
```

- [ ] **Step 2: Run — FAIL**

Run: `cargo test -p ustt-media --test probe`
Expected: FAIL.

- [ ] **Step 3: Реализовать**

`locate`: explicit dir → имена `ffmpeg.exe/ffprobe.exe` на Windows, иначе `ffmpeg`; затем `USTT_FFMPEG_DIR`; затем каталог текущего exe; затем PATH (`which`-логика без crate). `probe`: `ffprobe -v error -print_format json -show_format -show_streams <src>`, парсинг `serde_json::Value`; `duration` из `format.duration` или максимального `stream.duration`; видео — первый `codec_type=video` с `width/height > 0` (attached pic игнорируем), аудио — первый `codec_type=audio`. Ошибка ffprobe → `MediaError::Probe(stderr)`.

- [ ] **Step 4: PASS + коммит**

Run: `cargo test -p ustt-media --test probe`
```bash
git add Cargo.toml crates/media
git commit -m "feat(media): ffmpeg locator and ffprobe"
```

---

### Task 6: Декодирование в 16 кГц, WAV, волна

**Files:**
- Create: `crates/media/src/{decode,wav,waveform}.rs`
- Create: `crates/media/tests/decode.rs`, `crates/media/tests/waveform.rs`
- Modify: `crates/media/src/lib.rs`, `crates/media/Cargo.toml` (`hound`, dev `tempfile`)

**Interfaces:**
- Produces: `decode_wav16k(&Tools, &Path, &Path) -> Result<(), MediaError>`; `read_wav_mono16k(&Path) -> Result<Vec<f32>, MediaError>`; `write_wav_mono16k(&Path, &[f32]) -> Result<(), MediaError>`; `Waveform { bucket_ms: u32, peaks: Vec<Peak> }`; `Peak { min, max }`; `waveform_from_samples(&[f32], sample_rate, bucket_ms) -> Waveform`; `WaveformBuilder::{new, push, finish}`; `waveform_from_wav(&Path, bucket_ms) -> Result<Waveform, MediaError>`; `waveform_to_bytes/from_bytes`.

- [ ] **Step 1: Тесты**

```rust
#[test]
fn decode_resamples_48k_stereo() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("s.wav");
    write_stereo_48k(&src, 1.0);           // helper: hound, 48000, 2 канала, синус
    let dst = tmp.path().join("out.wav");
    decode_wav16k(&Tools::locate(None).unwrap(), &src, &dst).unwrap();
    let s = read_wav_mono16k(&dst).unwrap();
    assert!((s.len() as i64 - 16_000).abs() <= 1);
    assert!(s.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
}

#[test]
fn waveform_buckets_are_20ms() {
    let samples: Vec<f32> = (0..16_000).map(|i| if i < 8_000 { -0.5 } else { 0.75 }).collect();
    let w = waveform_from_samples(&samples, 16_000, 20);
    assert_eq!(w.peaks.len(), 50);
    assert!((w.peaks[0].min + 0.5).abs() < 1e-6);
    assert!((w.peaks[49].max - 0.75).abs() < 1e-6);
}

#[test]
fn waveform_from_wav_1h_is_streaming() {
    let tmp = tempfile::tempdir().unwrap();
    let f = tmp.path().join("long.wav");
    let spec = hound::WavSpec { channels: 1, sample_rate: 16_000, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut w = hound::WavWriter::create(&f, spec).unwrap();
    for i in 0..16_000u32 * 3600 { w.write_sample(((i % 200) as i16) - 100).unwrap(); }
    w.finalize().unwrap();
    let wf = waveform_from_wav(&f, 20).unwrap();
    assert_eq!(wf.peaks.len(), 180_000);
}

#[test]
fn waveform_bytes_roundtrip() {
    let w = waveform_from_samples(&[0.0, 1.0, -1.0, 0.5], 16_000, 20);
    assert_eq!(Waveform::from_bytes(&w.to_bytes()).unwrap(), w);
}
```

- [ ] **Step 2: Run — FAIL**

Run: `cargo test -p ustt-media --test waveform`
Expected: FAIL.

- [ ] **Step 3: Реализовать**

`decode_wav16k`: `ffmpeg -y -i <src> -vn -ac 1 -ar 16000 -c:a pcm_s16le <dst>`; stderr в ошибку. `read_wav_mono16k`/`write_wav_mono16k` через `hound` (Int16 ↔ f32 делением на 32768). `WaveformBuilder` копит min/max по бакету и не хранит сэмплы. `waveform_from_wav` читает через `hound::WavReader::samples` потоково. Бинарный формат волны: `magic "USTTW1"`, `u32 bucket_ms`, `u64 count`, затем пары f32 little-endian.

- [ ] **Step 4: PASS + коммит**

Run: `cargo test -p ustt-media --test decode --test waveform`
```bash
git add crates/media
git commit -m "feat(media): 16k decode, WAV IO, streaming waveform"
```

---

### Task 7: Прокси для воспроизведения

**Files:**
- Create: `crates/media/src/proxy.rs`
- Create: `crates/media/tests/proxy.rs`
- Modify: `crates/media/src/lib.rs`

**Interfaces:**
- Produces: `ProxyInfo { path: PathBuf, codec: String }`; `make_proxy(&Tools, src, out_dir: &Path, stem: &str, kind: MediaKind) -> Result<ProxyInfo, MediaError>`; `select_video_encoder(&Tools) -> Result<&'static str, MediaError>`.

- [ ] **Step 1: Тесты**

```rust
#[test]
fn audio_proxy_is_m4a_aac() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("a.wav");
    write_sine_wav(&src, 1.0);
    let info = make_proxy(&Tools::locate(None).unwrap(), &src, tmp.path(), "proxy", MediaKind::Audio).unwrap();
    assert_eq!(info.codec, "aac");
    assert_eq!(info.path.extension().unwrap(), "m4a");
    assert!(info.path.is_file());
}

#[test]
fn video_proxy_uses_an_available_encoder() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("v.mp4");
    make_testsrc_mp4(&src);
    let info = make_proxy(&Tools::locate(None).unwrap(), &src, tmp.path(), "proxy", MediaKind::Video).unwrap();
    assert!(["h264", "vp9"].contains(&info.codec.as_str()));
    assert!(info.path.is_file());
}

#[test]
fn select_video_encoder_reports_vp9_on_linux() {
    let enc = select_video_encoder(&Tools::locate(None).unwrap()).unwrap();
    assert!(matches!(enc, "h264_mf" | "libx264" | "libvpx-vp9"));
}
```

- [ ] **Step 2: Run — FAIL**

Run: `cargo test -p ustt-media --test proxy`
Expected: FAIL.

- [ ] **Step 3: Реализовать**

`select_video_encoder`: читает `ffmpeg -hide_banner -encoders`, порядок предпочтения `h264_mf`, `libx264`, `libvpx-vp9`. `make_proxy`: аудио → `<out_dir>/<stem>.m4a` с `-vn -c:a aac -b:a 128k`; видео → `<out_dir>/<stem>.mp4` для H.264 и `<out_dir>/<stem>.webm` для VP9 (`-b:v 2M`, `-preset veryfast` для x264, `-deadline realtime -cpu-used 8` для vp9), всегда `-c:a aac`. Кодек определяется по фактическому `ffprobe` результата.

- [ ] **Step 4: PASS + коммит**

Run: `cargo test -p ustt-media --test proxy`
```bash
git add crates/media
git commit -m "feat(media): playback proxy with encoder fallback"
```

---

### Task 8: Who — mel-фронтенд и эталон

**Files:**
- Create: `crates/diarize/Cargo.toml`, `src/lib.rs`, `src/banks.rs`, `src/dft.rs`, `src/frontend.rs`
- Create: `scripts/fetch-who.sh`, `scripts/fetch-ort.sh`, `scripts/reference/who_frontend.py`
- Create: `crates/diarize/tests/frontend_golden.rs`, `tests/fixtures/who_frontend.json`
- Modify: root `Cargo.toml`

**Interfaces:**
- Produces: `FrontendGeometry { sample_rate, frame, shift, n_fft, mels, preemphasis, scale, log_floor }`; `Frontend::new(geometry, banks: &[u8]) -> Result<Frontend, DiarizeError>`; `Frontend::frames(&self, samples: usize) -> usize`; `Frontend::means(&self, &[f32]) -> (Vec<f32>, usize)`; `Frontend::features(&self, &[f32], Range<usize>, &[f32]) -> Vec<f32>`; `parse_banks(&[u8]) -> Result<(Vec<f32>, usize, usize), DiarizeError>`.

- [ ] **Step 1: Написать независимый эталон на Python**

`scripts/fetch-who.sh` скачивает `onnx-int8/*` (включая `who_mel_banks.f32`) в `models/who/`; `scripts/fetch-ort.sh` скачивает ONNX Runtime 1.22+ для текущей ОС в `lib/`. Оба — `curl -L --fail`, версии закреплены.

`scripts/reference/who_frontend.py` (numpy) читает `who_mel_banks.f32` и строит сигнал
`0.5 sin(2π·440·t) + 0.25 sin(2π·1234.5·t) + 0.05 sin(2π·97·t)`, 30 с, 16 кГц;
считает Kaldi log-mel точно по шагам из спеки (`scale=32768`, mean removal, preemphasis 0.97, hamming symmetric, n_fft 512, 23 mel, log floor 2^-23), вычитает средние по записи и пишет `tests/fixtures/who_frontend.json`:
`{"frames":2998,"means":[...23],"spots":[{"mel":M,"frame":F,"value":V}, ...12]}`.
Споты — детерминированно выбранные (mel, frame) с ненулевым значением.

- [ ] **Step 2: Тест, читающий фикстуру**

```rust
#[test]
fn frontend_matches_reference() {
    let g: Golden = serde_json::from_str(include_str!("../../../tests/fixtures/who_frontend.json")).unwrap();
    let path = std::env::var("WHO_MEL_BANKS").expect("run with WHO_MEL_BANKS=models/who/who_mel_banks.f32");
    let banks = std::fs::read(path).unwrap();
    let f = Frontend::new(geometry(), &banks).unwrap();
    let signal = reference_signal(30.0, 16_000);
    let (means, frames) = f.means(&signal);
    assert_eq!(frames, g.frames);
    for (a, b) in means.iter().zip(&g.means) { assert!((a - b).abs() < 1e-4, "{a} vs {b}"); }
    let feats = f.features(&signal, 0..frames, &means);
    for s in &g.spots {
        let got = feats[s.frame * 23 + s.mel];
        assert!((got - s.value).abs() < 1e-3, "spot {s:?}: {got}");
    }
}
```

- [ ] **Step 3: Run — FAIL**

Run: `WHO_MEL_BANKS=models/who/who_mel_banks.f32 cargo test -p ustt-diarize --test frontend_golden`
Expected: FAIL.

- [ ] **Step 4: Реализовать**

`dft.rs` — radix-2 FFT в f64 (как в Swift-референсе), наружу f32; для n_fft=512 всегда степень двойки. `banks.rs` — формат: два little-endian u32 (`mels`, `bins`), затем `f32 * mels * bins`; проверить длину. `frontend.rs` — точный порядок: scale → mean removal по кадру → preemphasis с конца → окно → zero pad → FFT → мощность → mel → `ln(max(x, floor))`; `means` считает суммы в f64 по кускам 6000 кадров, затем делит; `features` вычитает средние. `frames = 1 + (n - frame)/shift`, при `n < frame` — 0.

- [ ] **Step 5: PASS + коммит**

Run: `WHO_MEL_BANKS=models/who/who_mel_banks.f32 cargo test -p ustt-diarize --test frontend_golden`
```bash
git add Cargo.toml crates/diarize scripts/reference tests/fixtures
git commit -m "feat(diarize): Who log-mel frontend with golden test"
```

---

### Task 9: Who — meta, сессии, логиты

**Files:**
- Create: `crates/diarize/src/{meta,session,logits}.rs`
- Create: `crates/diarize/tests/logits.rs`, `scripts/reference/who_onnx.py`
- Modify: `crates/diarize/src/lib.rs`, `Cargo.toml` (`ort`, `ndarray`)

**Interfaces:**
- Produces: `Meta` (serde, поля как в `who_meta.json`), `Meta::load(dir) -> Result<Meta>`, `Meta::validate() -> Result<(), DiarizeError>`; `Sessions::load(dir, &Meta) -> Result<Sessions>`; `Sessions::run(&mut self, graph: &str, inputs: Vec<(&str, Tensor)>, outputs: &[&str]) -> Result<Vec<Tensor>>`; `WhoLogits { presence: Vec<f32>, masks: Vec<f32>, queries: Vec<f32> }`; `compute_logits(&mut Sessions, &Meta, &Frontend, &[f32], piece: Range<usize>, means: &[f32]) -> Result<WhoLogits>`; `pub fn concat_mask_chunks(outputs: &[Vec<f32>], chunks: &[usize], frames: usize, queries: usize) -> Vec<f32>`.

- [ ] **Step 1: Python-эталон логитов**

`scripts/reference/who_onnx.py` повторяет `compute_logits` на `onnxruntime` для того же синтетического сигнала 100 с: один кусок 30000 кадров, паддинг последним кадром, encoder → queries по чанкам, sigmoid не применяется. Пишет `tests/fixtures/who_logits.json` с `presence` (50 значений) и `mask_row_means` (50 значений) и `mask_spots` (12 значений). Файл коммитится.

- [ ] **Step 2: Тесты**

```rust
#[test]
fn mask_chunks_concatenate_without_padding() {
    let masks = concat_mask_chunks(&[vec![1.0, 2.0, 3.0, 4.0], vec![5.0, 6.0]], &[4, 2], 5, 1);
    assert_eq!(masks, vec![1.0, 2.0, 3.0, 4.0, 5.0]);
}

#[test]
fn meta_rejects_window_without_chunks() {
    let mut meta: Meta = serde_json::from_str(MINIMAL_META).unwrap();
    meta.windows[0].chunks = vec![];
    assert!(meta.validate().is_err());
}

#[test]
#[ignore = "needs models and onnxruntime"]
fn logits_match_onnx_reference() {
    let g: LogitsGolden = serde_json::from_str(include_str!("../../../tests/fixtures/who_logits.json")).unwrap();
    // загрузка models/who, сигнал 100 с, compute_logits
    // presence: |a-b| < 1e-3; mask_row_means: < 1e-2; spots: < 5e-2
}
```

- [ ] **Step 3: Run — FAIL (unit) / ignore (gated)**

Run: `cargo test -p ustt-diarize --test logits`
Expected: FAIL.

- [ ] **Step 4: Реализовать**

`Meta` — serde-структуры строго по `who_meta.json`; `validate()` проверяет schema=2, architecture `eend_m2f`, `frames == low * downsample`, сумма chunks == frames, `chunks.len()+1 == queries.inputs.len()`, наличие сессий. `Sessions` грузит `encoder_6000`, `encoder_30000`, `queries_6000`, `queries_30000`, `voice_3256` через `ort::session::Session::builder().commit_from_file`; `run` берёт `&mut Session` (ort 2.0 требует mut) и возвращает `Tensor<f32>` как `Vec<f32>`. `compute_logits`: выбрать кратчайшее окно ≥ frames, заполнить паддинг последним mel-кадром, encoder вход `[1, window_frames, mels]`, выходы `memory [1, low, 256]` и `frames [1, window_frames, 256]`; queries входы `memory`, `frames_i` по чанкам; выходы `presence [50]`, `queries [50*256]`, `masks_i [50*chunk]`; `concat_mask_chunks` отбрасывает паддинг.

- [ ] **Step 5: PASS + коммит**

Run: `cargo test -p ustt-diarize --test logits`
```bash
git add crates/diarize scripts/reference tests/fixtures
git commit -m "feat(diarize): Who ONNX sessions and logits"
```

---

### Task 10: Who — решение, куски, voice join

**Files:**
- Create: `crates/diarize/src/{decision,pieces,linker,embedding,voices,cluster}.rs`
- Create: `crates/diarize/tests/who_pipeline.rs`
- Modify: `crates/diarize/src/lib.rs`

**Interfaces:**
- Produces: `WhoModel::load(dir: &Path) -> Result<WhoModel>`; `WhoModel::diarize(&mut self, samples: &[f32], sample_rate: u32, progress: &mut dyn FnMut(f32)) -> Result<Diarization>`; `Decision::{thresholds, stretches}`; `pieces::cut(frames, window) -> Vec<Range<usize>>`; `cluster::average_linkage(&[[f32]], cosine) -> Vec<usize>`; `Embedding::from_sidecar(&[u8]) -> Result<Embedding>`; `Voices::windows(...)`, `column_mask(...)`, `nearest(...)`.

- [ ] **Step 1: Тесты**

```rust
#[test]
fn decision_applies_mask_rule_and_pooling() {
    let d = Decision { presence_threshold: 0.8, mask_threshold: 0.5, rule: Rule::Mask, pool: 1 };
    let s = d.stretches(&[0.9, 0.1], &[0.0, 0.6, 0.7, 0.0, 0.9, 0.0], 3);
    assert_eq!(s, vec![Stretch { query: 0, first: 1, end: 3 }]);
}

#[test]
fn pieces_are_300s_end_to_end() {
    assert_eq!(pieces::cut(30_000, 30_000), vec![0..30_000]);
    assert_eq!(pieces::cut(30_001, 30_000), vec![0..30_000, 30_000..30_001]);
}

#[test]
fn average_linkage_merges_close_vectors() {
    let labels = cluster::average_linkage(&[vec![1.0, 0.0], vec![0.99, 0.01], vec![0.0, 1.0]], 0.65);
    assert_eq!(labels[0], labels[1]);
    assert_ne!(labels[0], labels[2]);
}

#[test]
fn embedding_sidecar_parses_header() {
    // 2x4 weight + 2 bias, f16: out=2, in=4 => columns=2
    let bytes = embedding_sidecar(2, 4, &[0.0; 10]);
    let e = Embedding::from_sidecar(&bytes).unwrap();
    assert_eq!((e.out, e.columns), (2, 2));
}

#[test]
#[ignore = "needs models"]
fn diarize_700s_uses_two_pieces() {
    // сигнал 700 с, две гармоники; проверяем, что progress доходит до 1.0 и turns не выходят за 700 с
}

#[test]
#[ignore = "needs models"]
fn diarize_two_speakers_fixture() {
    // models + tests/fixtures/two_speakers.wav => 2 спикера, turns отсортированы
}
```

- [ ] **Step 2: Run — FAIL**

Run: `cargo test -p ustt-diarize --test who_pipeline`
Expected: FAIL.

- [ ] **Step 3: Реализовать**

Порт один-в-один `Decision`, `FrameClock`, `Pieces`, `Linker`, `Embedding`, `Voices`, `AverageLinkage` из Swift-референса. `WhoModel::diarize`: ресемплинг линейный до 16 кГц; `frontend.means` по всей записи; один кусок — быстрый путь; иначе куски по 30 000 кадров, на каждый `compute_logits`, sigmoid, stretches, solo-маска, lanes; если есть voice-граф — voice frontend (80 mel), окна `Voices::windows`, пулы, embedding, кластеризация, fold безголосых; финальные turns через `Linker::joined`. Прогресс — после каждого куска и каждого voice-окна. `Diarization.duration` — длина аудио в секундах.

- [ ] **Step 4: PASS + gated-тесты**

Run: `cargo test -p ustt-diarize --test who_pipeline`
Run: `USTT_MODELS_DIR=$PWD/models cargo test -p ustt-diarize --test who_pipeline -- --ignored`
Expected: PASS.

- [ ] **Step 5: Коммит**

```bash
git add crates/diarize
git commit -m "feat(diarize): Who decision, pieces, voice join"
```

---

### Task 11: Parakeet FFI и JSON

**Files:**
- Create: `crates/asr/Cargo.toml`, `src/lib.rs`, `src/ffi.rs`, `src/json.rs`
- Create: `crates/asr/tests/json.rs`, `crates/asr/tests/ffi.rs`
- Create: `scripts/fetch-parakeet.sh`
- Modify: root `Cargo.toml`

**Interfaces:**
- Produces: `ParakeetLib::open(path: &Path) -> Result<ParakeetLib, AsrError>`; `ParakeetLib::abi_version() -> u32` (после open); `AsrEngine::load(lib: Arc<ParakeetLib>, model: &Path) -> Result<AsrEngine>`; `AsrEngine::transcribe_wav(&mut self, wav: &Path) -> Result<AsrJson>`; `AsrJson { text, language: Option<String>, words: Vec<Word> }`; `AsrError`.

- [ ] **Step 0: Скрипт загрузки движка и моделей**

`scripts/fetch-parakeet.sh`: для Linux — `parakeet-v0.6.1-lib-linux-cpu-x64.tar.gz`, для Windows — `parakeet-v0.6.1-lib-win-cpu-x64.zip` и `parakeet-v0.6.1-lib-win-vulkan-x64.zip` из GitHub-релиза в `lib/`; плюс `redux-packed.gguf`, `redux-q8_0.gguf`, `silero-vad-f16.gguf` с revision `c77726e46ceaab96b5a5a3328915edb44c73209d` в `models/`. `curl -L --fail`, распаковка во временный каталог, проверка наличия `libparakeet`.

- [ ] **Step 1: Тесты парсинга**

```rust
#[test]
fn parses_words_with_offsets() {
    let j: AsrJson = serde_json::from_str(r#"{"text":"hi there","frame_sec":0.08,
        "words":[{"w":"hi","start":0.48,"end":0.64,"conf":0.9},
                 {"w":"there","start":0.64,"end":0.96,"conf":0.8}],
        "tokens":[]}"#).unwrap();
    assert_eq!(j.words.len(), 2);
    assert_eq!(j.words[0].text, "hi");
    assert_eq!(j.words[1].start, 0.64);
    assert_eq!(j.language, None);
}

#[test]
fn empty_document_is_valid() {
    let j: AsrJson = serde_json::from_str(r#"{"text":"","words":[],"tokens":[]}"#).unwrap();
    assert!(j.words.is_empty());
}
```

- [ ] **Step 2: Тест ABI (gated)**

```rust
#[test]
#[ignore = "needs libparakeet"]
fn abi_is_ten() {
    let lib = ParakeetLib::open(&lib_path()).unwrap();
    assert!(lib.abi_version() >= 10);
}
```

- [ ] **Step 3: Run — FAIL**

Run: `cargo test -p ustt-asr --test json`
Expected: FAIL.

- [ ] **Step 4: Реализовать**

`ffi.rs`: `libloading::Library`, символы `parakeet_capi_abi_version`, `_load`, `_load_error`, `_free`, `_transcribe_path_json`, `_free_string`, `_vad_path_json`, `_model_kind`. Все вызовы — `unsafe`, результат `*mut c_char` → `CStr` → `String`, затем `free_string`. Проверка ABI при `open`; `parakeet_capi_load` NULL → текст из `load_error`. `AsrEngine` хранит `Arc<ParakeetLib>` и `SendPtr` ctx; `Drop` освобождает ctx. `unsafe impl Send for AsrEngine` с комментарием: контекст используется одним потоком.

- [ ] **Step 5: PASS + коммит**

Run: `cargo test -p ustt-asr --test json`
```bash
git add Cargo.toml crates/asr
git commit -m "feat(asr): parakeet.cpp FFI and JSON types"
```

---

### Task 12: VAD-нарезка и транскрипция кусков

**Files:**
- Create: `crates/asr/src/vad.rs`, `crates/asr/src/stitch.rs`
- Create: `crates/asr/tests/vad.rs`, `crates/asr/tests/stitch.rs`
- Modify: `crates/asr/src/lib.rs`

**Interfaces:**
- Produces: `VadEngine::load(lib: Arc<ParakeetLib>, silero: &Path) -> Result<VadEngine>`; `VadEngine::speech_ranges(&mut self, wav: &Path) -> Result<Vec<TimeRange>>`; `TimeRange { start: f64, end: f64 }`; `chunk_ranges(&[TimeRange], max_len_s: f64, duration: f64) -> Vec<TimeRange>`; `transcribe_chunks(&mut AsrEngine, &mut VadEngine, wav: &Path, max_len_s: f64, progress: &mut dyn FnMut(f32)) -> Result<AsrJson>`; `stitch(chunks: &[(TimeRange, AsrJson)]) -> AsrJson`.

- [ ] **Step 1: Тесты**

```rust
#[test]
fn empty_vad_falls_back_to_fixed_windows() {
    let r = chunk_ranges(&[], 30.0, 65.0);
    assert_eq!(r.len(), 3);
    assert!((r[2].end - 65.0).abs() < 1e-9);
    assert!(r.windows(2).all(|w| w[0].end <= w[1].start + 1e-9));
}

#[test]
fn long_speech_is_split_at_30s() {
    let r = chunk_ranges(&[TimeRange { start: 0.0, end: 75.0 }], 30.0, 75.0);
    assert_eq!(r, vec![
        TimeRange { start: 0.0, end: 30.0 },
        TimeRange { start: 30.0, end: 60.0 },
        TimeRange { start: 60.0, end: 75.0 },
    ]);
}

#[test]
fn stitch_offsets_words_and_joins_text() {
    let a = AsrJson { text: "hello".into(), language: None, words: vec![Word { start: 0.2, end: 0.4, text: "hello".into(), confidence: None }] };
    let b = AsrJson { text: "world".into(), language: Some("en".into()), words: vec![Word { start: 0.1, end: 0.3, text: "world".into(), confidence: None }] };
    let out = stitch(&[(TimeRange { start: 10.0, end: 12.0 }, a), (TimeRange { start: 20.0, end: 22.0 }, b)]);
    assert_eq!(out.text, "hello world");
    assert_eq!(out.words[0].start, 10.2);
    assert_eq!(out.words[1].start, 20.1);
    assert_eq!(out.language.as_deref(), Some("en"));
}

#[test]
#[ignore = "needs models"]
fn silence_yields_no_segments() { /* WAV 5 с нулей: vad возвращает пусто, transcribe_chunks — пустой AsrJson */ }
```

- [ ] **Step 2: Run — FAIL**

Run: `cargo test -p ustt-asr --test vad --test stitch`
Expected: FAIL.

- [ ] **Step 3: Реализовать**

`vad.rs`: `parakeet_capi_vad_path_json(ctx, wav, null)` → JSON `{"segments":[{"start","end"}]}`; парсинг терпим к лишним полям. `chunk_ranges`: режет длинные диапазоны по `max_len_s`, пустой вход → равномерные окна от 0 до duration (последнее короче). `transcribe_chunks`: для каждого диапазона пишет временный WAV через `ffmpeg -ss X -to Y` (или `ustt_media::write_wav_mono16k` после нарезки в памяти), зовёт `transcribe_wav`, прогресс — по сумме длительностей; удаляет temp в `Drop`-guard. `stitch`: конкатенация текстов через пробел, сдвиг слов на `range.start`, первый непустой `language`.

- [ ] **Step 4: PASS + gated**

Run: `cargo test -p ustt-asr --test vad --test stitch`
Run: `USTT_MODELS_DIR=$PWD/models cargo test -p ustt-asr --test vad -- --ignored`
Expected: PASS.

- [ ] **Step 5: Коммит**

```bash
git add crates/asr
git commit -m "feat(asr): VAD chunking and timed transcription"
```

---

### Task 13: Сегменты и спикеры

**Files:**
- Create: `crates/pipeline/Cargo.toml`, `src/lib.rs`, `src/merge.rs`
- Create: `crates/pipeline/tests/merge.rs`
- Modify: root `Cargo.toml`

**Interfaces:**
- Produces: `merge::build_segments(words: &[Word], turns: &[Turn], speaker_ids: &[Uuid], duration: f64) -> Vec<Segment>`; `merge::assign_speaker(start, end, turns, speaker_ids) -> Option<Uuid>`; `merge::{MAX_GAP_S: f64, MAX_SEGMENT_S: f64}`.

- [ ] **Step 1: Тесты**

```rust
#[test]
fn splits_on_gap_punctuation_and_length() {
    let words = words_from(&[(0.0, 0.2, "hi"), (0.3, 0.5, "there."), (2.0, 2.2, "next"), (16.0, 16.2, "late")]);
    let segs = build_segments(&words, &[], &[], 30.0);
    assert_eq!(segs.len(), 3);
    assert_eq!(segs[0].text, "hi there.");
    assert_eq!(segs[0].start, 0.0);
    assert!((segs[2].start - 16.0).abs() < 1e-9);
}

#[test]
fn overlapping_turns_pick_max_overlap() {
    let turns = vec![Turn { start: 0.0, end: 2.0, speaker_index: 0 }, Turn { start: 1.0, end: 4.0, speaker_index: 1 }];
    let ids = vec![Uuid::from_u128(1), Uuid::from_u128(2)];
    assert_eq!(assign_speaker(1.2, 1.6, &turns, &ids), Some(ids[1]));
    assert_eq!(assign_speaker(0.1, 0.5, &turns, &ids), Some(ids[0]));
}

#[test]
fn no_overlap_leaves_speaker_none() {
    let turns = vec![Turn { start: 0.0, end: 1.0, speaker_index: 0 }];
    assert_eq!(assign_speaker(5.0, 6.0, &turns, &[Uuid::from_u128(1)]), None);
}

#[test]
fn equal_overlap_picks_earliest_turn() {
    let turns = vec![Turn { start: 0.0, end: 1.0, speaker_index: 5 }, Turn { start: 1.0, end: 2.0, speaker_index: 7 }];
    let ids = vec![Uuid::from_u128(5), Uuid::from_u128(7)];
    assert_eq!(assign_speaker(0.9, 1.1, &turns, &ids), Some(ids[0]));
}
```

- [ ] **Step 2: Run — FAIL**

Run: `cargo test -p ustt-pipeline --test merge`
Expected: FAIL.

- [ ] **Step 3: Реализовать**

Границы: новый сегмент, если пауза между словами > 0.6 с, предыдущее слово кончается на `. ! ? …`, или длина превысила 15 с. Текст — слова через пробел с пунктуацией как есть. Спикер — максимум `min(end, t.end) - max(start, t.start)`; при равенстве (`>` не `>=`) выигрывает более ранний turn; `speaker_ids[turn.speaker_index]`, несуществующий индекс → `None`. `duration` ограничивает конец последнего сегмента.

- [ ] **Step 4: PASS + коммит**

Run: `cargo test -p ustt-pipeline --test merge`
```bash
git add Cargo.toml crates/pipeline
git commit -m "feat(pipeline): segment building and speaker assignment"
```

---

### Task 14: Бэкенд и модели

**Files:**
- Create: `crates/pipeline/src/backend.rs`, `crates/pipeline/src/models.rs`
- Create: `crates/pipeline/tests/backend.rs`, `crates/pipeline/tests/models.rs`
- Modify: `crates/pipeline/Cargo.toml` (`ash`, `sha2`, `libloading`)

**Interfaces:**
- Produces: `Backend { Cpu, Vulkan, Cuda }`; `Detection { selected, cuda_available, vulkan_available, hip_present, gpu_name: Option<String> }`; `detect(pref: BackendPreference) -> Detection`; `detect_with(pref: BackendPreference, env_override: Option<&str>) -> Detection`; `parakeet_lib(backend) -> &'static str` (`parakeet_cpu.dll`, `parakeet_vulkan.dll`, `parakeet_cuda.dll`); `ModelPaths { who_dir, asr_model, silero_vad }`; `resolve_models(dir: &Path, backend: Backend) -> Result<ModelPaths, ModelError>`; `ModelFile { name, sha256, bytes }`; `verify_models(dir) -> Vec<ModelCheck>`.

- [ ] **Step 1: Тесты**

```rust
#[test]
fn backend_maps_to_library_and_model() {
    assert_eq!(parakeet_lib(Backend::Cpu), "parakeet_cpu.dll");
    assert_eq!(parakeet_lib(Backend::Vulkan), "parakeet_vulkan.dll");
    assert_eq!(asr_model_file(Backend::Cpu), "redux-packed.gguf");
    assert_eq!(asr_model_file(Backend::Vulkan), "redux-q8_0.gguf");
}

#[test]
fn env_override_forces_cpu() {
    assert_eq!(detect_with(BackendPreference::Auto, Some("cpu")).selected, Backend::Cpu);
}

#[test]
fn resolve_models_reports_missing_file() {
    let tmp = tempfile::tempdir().unwrap();
    let err = resolve_models(tmp.path(), Backend::Cpu).unwrap_err();
    assert!(matches!(err, ModelError::Missing { .. }));
}

#[test]
fn verify_models_detects_bad_hash() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("redux-packed.gguf"), b"x").unwrap();
    let checks = verify_models(tmp.path());
    assert!(checks.iter().any(|c| !c.ok && c.name == "redux-packed.gguf"));
}
```

- [ ] **Step 2: Run — FAIL**

Run: `cargo test -p ustt-pipeline --test backend --test models`
Expected: FAIL.

- [ ] **Step 3: Реализовать**

`detect`: env `USTT_BACKEND`; иначе `cuda` при загружаемой `nvcuda.dll`/`libcuda.so.1` и наличии `parakeet_cuda.*` рядом, затем Vulkan через `ash::Entry::load()` + перечисление физических устройств (нужен graphics+compute), иначе CPU. `hip_present` — попытка загрузить `amdhip64.dll`/`libamdhip64.so` (в UI показывается как неподдерживаемый). `verify_models` читает `models/manifest.json` (генерируется скриптом Task 16) и сверяет SHA-256 потоково. `resolve_models`: who-каталог с 8 файлами, ASR по бэкенду, Silero.

- [ ] **Step 4: PASS + коммит**

Run: `cargo test -p ustt-pipeline --test backend --test models`
```bash
git add crates/pipeline
git commit -m "feat(pipeline): backend detection and model resolution"
```

---

### Task 15: Оркестрация импорта и CLI

**Files:**
- Create: `crates/pipeline/src/progress.rs`, `crates/pipeline/src/import.rs`
- Create: `crates/cli/Cargo.toml`, `crates/cli/src/main.rs`, `crates/pipeline/tests/import.rs`
- Modify: `crates/pipeline/src/lib.rs`, root `Cargo.toml`

**Interfaces:**
- Produces: `Phase { Probe, Decode, Proxy, Diarize, Transcribe, Merge, Done }`; `Progress { phase, fraction, eta_seconds, detail }`; `ImportRequest { input, project_dir, models, backend, make_proxy }`; `run_import(&ImportRequest, &AtomicBool, &mut dyn FnMut(Progress)) -> Result<Project, PipelineError>`; `PHASE_WEIGHTS`; `estimate_eta`.

- [ ] **Step 1: Тесты**

```rust
#[test]
fn phase_fractions_are_monotonic() {
    assert_eq!(phase_start(Phase::Probe), 0.0);
    assert!((phase_start(Phase::Transcribe) - 0.35).abs() < 1e-9);
    assert_eq!(phase_start(Phase::Done), 1.0);
    assert!(PHASE_WEIGHTS.iter().sum::<f32>() > 0.999);
}

#[test]
fn eta_extrapolates_from_processed_audio() {
    let eta = estimate_eta(60.0, 600.0, 6.0);
    assert!((eta - 54.0).abs() < 1e-6);
}

#[test]
fn cancel_leaves_no_partial_project() {
    let tmp = tempfile::tempdir().unwrap();
    let cancel = AtomicBool::new(true);
    let req = ImportRequest {
        input: tmp.path().join("missing.mp4"),
        project_dir: tmp.path().join("t.ustt"),
        models: ModelPaths { who_dir: tmp.path().join("who"), asr_model: tmp.path().join("m.gguf"), silero_vad: tmp.path().join("v.gguf") },
        backend: Backend::Cpu,
        make_proxy: false,
    };
    let err = run_import(&req, &cancel, &mut |_| {}).unwrap_err();
    assert!(matches!(err, PipelineError::Cancelled));
    assert!(!req.project_dir.exists() || !req.project_dir.join("project.json").exists());
}
```

- [ ] **Step 2: Run — FAIL**

Run: `cargo test -p ustt-pipeline --test import`
Expected: FAIL.

- [ ] **Step 3: Реализовать**

Веса фаз: Probe 0.02, Decode 0.05, Proxy 0.08, Diarize 0.20, Transcribe 0.60, Merge 0.05; `phase_start` — кумулятивная сумма. `run_import`: создаёт проект в `<project_dir>.partial`, пишет WAV и волну в него; после успеха переименовывает в `<project_dir>` атомарно; при отмене/ошибке удаляет `.partial`. Порядок: probe → decode → waveform → diarize (параллельно с proxy в отдельном потоке) → transcribe (VAD-куски) → merge → save. `cancel` проверяется перед каждой фазой и внутри progress-callback движков. `parakeet_lib` ищется рядом с exe CLI/`USTT_LIB_DIR`.

CLI:
```
ustt-cli probe <file>
ustt-cli doctor [--models <dir>]
ustt-cli process <file> --project <dir> --models <dir> [--backend auto|cpu|vulkan|cuda] [--no-proxy]
```
`process` печатает каждое `Progress` как JSON-строку в stdout, итог — `{"status":"done","project":"..."}`; ошибки — в stderr, код 1; `--help` — код 0.

- [ ] **Step 4: PASS + коммит**

Run: `cargo test -p ustt-pipeline --test import`
```bash
git add crates/pipeline crates/cli
git commit -m "feat(pipeline): import orchestration and CLI"
```

---

### Task 16: Скрипты моделей, e2e, README

**Files:**
- Create: `scripts/fetch-fixtures.sh`, `scripts/write-manifest.sh`
- Create: `crates/pipeline/tests/e2e.rs`
- Modify: `README.md`, `.gitignore`

**Interfaces:**
- Produces: `models/manifest.json` (имена, размеры, SHA-256), `models/who/`, `models/redux-packed.gguf`, `models/redux-q8_0.gguf`, `models/silero-vad-f16.gguf`, `tests/fixtures/two_speakers.wav`, `lib/` с `libparakeet` под текущую ОС.

- [ ] **Step 1: Скрипты**

`fetch-fixtures.sh`: `two_speakers.wav` из `mudler/parakeet.cpp` v0.6.1 в `tests/fixtures/`.
`write-manifest.sh`: `sha256sum` по всем моделям → `models/manifest.json` (формат читает `verify_models`).

- [ ] **Step 2: E2E-тест (gated)**

```rust
#[test]
#[ignore = "needs models, libparakeet and fixture"]
fn imports_two_speaker_clip_end_to_end() {
    let tmp = tempfile::tempdir().unwrap();
    let req = ImportRequest { input: fixture("two_speakers.wav"), project_dir: tmp.path().join("t.ustt"),
        models: resolve_models(&models_dir(), Backend::Cpu).unwrap(), backend: Backend::Cpu, make_proxy: true };
    let mut last = 0.0;
    let project = run_import(&req, &AtomicBool::new(false), &mut |p| { assert!(p.fraction >= last); last = p.fraction; }).unwrap();
    assert_eq!(last, 1.0);
    assert_eq!(project.speakers.len(), 2);
    assert!(!project.segments.is_empty());
    assert!(project.segments.iter().all(|s| s.end <= project.duration + 1e-6));
    let store = ProjectStore::open(&req.project_dir).unwrap().0;
    assert!(store.media_dir().join("audio16k.wav").is_file());
    assert!(store.media_dir().join("waveform.bin").is_file());
}
```

- [ ] **Step 3: Запустить e2e**

Run: `USTT_MODELS_DIR=$PWD/models cargo test -p ustt-pipeline --test e2e -- --ignored --nocapture`
Expected: PASS; в `target/` появляется проект с 2 спикерами.

- [ ] **Step 4: README**

Разделы: что это (пет-проект, некоммерческий), требования (Windows 10/11, WebView2), как поставить модели (`scripts/*`), как запустить CLI (`cargo run -p ustt-cli -- process ...`), лицензии и атрибуция, статус (ядро готово, UI в следующем плане).

- [ ] **Step 5: Финальная проверка и коммит**

Run: `cargo test --workspace && cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings`
```bash
git add scripts README.md .gitignore crates/pipeline/tests/e2e.rs
git commit -m "feat: model scripts, e2e import test, dev docs"
```

---

## Self-Review

- **Spec coverage:** домен/проект (1–4), media probe/decode/waveform/proxy (5–7), Who полностью (8–10), Redux FFI/VAD/склейка (11–12), сегменты/спикеры (13), бэкенды/модели (14), оркестрация/CLI/прогресс/отмена (15), скрипты/e2e/доки (16). UI, экспорт, упаковка, CI — вне этого плана по Scope.
- **Step scan:** каждый шаг — один тест, одна реализация или одна проверка; тела алгоритмов даны только там, где сигнатура и тест их не определяют (Who, merge).
- **Type consistency:** `Turn`/`Diarization`/`Word` из `ustt-core` используются в `ustt-diarize`, `ustt-asr`, `ustt-pipeline`; `MediaKind` один на всех; `ProjectStore::media_dir` — единственный источник путей внутри проекта.
- **Review Focus:** длинный файл → Task 6/10; тишина → Task 12; нестандартный звук → Task 5/6; перекрытия → Task 13; отмена → Task 15.
- **Proportion:** план длиннее спеки, потому что покрывает шесть крейтов и два FFI-контура; тела алгоритмов не переписаны, кроме Who-пайплайна, который нельзя вывести из сигнатур.
