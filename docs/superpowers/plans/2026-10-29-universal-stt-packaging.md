# Universal STT Windows Packaging and CI Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Produce a portable Windows ZIP with the desktop app, FFmpeg, ONNX Runtime, parakeet.cpp CPU/Vulkan DLLs and all models, built locally by a reproducible script and by GitHub Actions on a `v*` tag.

**Architecture:** Two Python scripts own everything testable on any OS: `fetch-win-deps.py` downloads and verifies the pinned Windows binaries/models into `vendor/win64/`; `package_release.py` validates that tree, the model manifest and the app executable, then writes a ZIP64 archive plus SHA-256 sums. The Windows-only part is just `dx build --platform desktop --release`; CI wires the three steps together. No slim archive in v1 (there is no model downloader yet) and no CUDA archive in v1 (separate optional asset later).

**Tech Stack:** Python 3.12 (stdlib only), Dioxus CLI 0.7.10, GitHub Actions (`windows-latest`, `ubuntu-latest`), parakeet.cpp v0.6.1 prebuilt libs, ONNX Runtime 1.31.1, BtbN FFmpeg LGPL shared, WebView2 Evergreen standalone installer.

**Spec:** `docs/superpowers/specs/2026-10-27-universal-stt-windows-design.md` §11 models/licenses, §12 build and delivery, §13.2 Windows acceptance.

## Global Constraints

- Portable ZIP name: `UniversalSTT-<version>-win64-portable.zip`; contents run without installation or admin rights.
- Archive contents: `UniversalSTT.exe`, `parakeet_cpu.dll`, `parakeet_vulkan.dll`, `onnxruntime.dll`, `ffmpeg.exe`, `ffprobe.exe`, FFmpeg DLLs, `WebView2Loader.dll` if present, `models/` (12 manifest files), `MicrosoftEdgeWebView2RuntimeInstallerX64.exe`, `LICENSE`, `NOTICE`, `README.md`, `version.txt`.
- DLL filenames must match the loader: `parakeet_lib(Backend::Cpu) = parakeet_cpu.dll`, Vulkan = `parakeet_vulkan.dll`; parakeet.cpp ships both as `parakeet.dll`, so the fetch step renames them.
- Every model file is verified against committed `models/manifest.json` (`sha256`, `bytes`) before packaging; a mismatch aborts.
- Models are stored in the ZIP (`ZIP_STORED`); executables/DLLs are deflated. ZIP64 is mandatory.
- CUDA and slim archives are out of scope for v1; README documents this.
- CI must never run `#[ignore]` model/FFmpeg tests; release workflow runs on tags only.
- No code signing; README carries the SmartScreen warning.
- Windows acceptance under Wine/Proton remains a manual release gate, not a CI job.

## Review Focus

1. **Missing or wrong dependency.** A missing DLL/model or an archive that still has `parakeet.dll` fails with a named file, not a partial ZIP. Test — Task 2 `self_test_rejects_missing_dependency`.
2. **Manifest drift.** A model whose bytes/SHA differ aborts packaging and CI. Test — Task 2 `self_test_rejects_bad_hash`.
3. **Large archive.** ZIP uses ZIP64 and stored GGUF/ONNX entries; a >4 GiB synthetic tree still packs. Test — Task 2 `self_test_zip64_flags`.
4. **CI scope.** `ci.yml` runs unit/web tests only; `release.yml` is tag-gated and fails if any download/hash fails.
5. **Portable layout.** All loader lookups (`ffmpeg.exe` next to exe, `models/` next to exe, DLLs next to exe) work from the ZIP root. Test — Task 1 `self_test_layout`.

---

## File Structure

```
scripts/fetch-win-deps.py         # downloads + renames + verifies Windows deps
scripts/package_release.py        # validates tree, builds ZIP64 + SHA256SUMS
scripts/fetch-parakeet.sh         # fix Windows DLL naming for local dev
.github/workflows/ci.yml          # fmt, clippy, tests, web build, Playwright
.github/workflows/release.yml     # tag -> Windows build -> portable ZIP
docs/BUILD-WINDOWS.md             # manual and CI build, verification, limitations
README.md                         # install/run, SmartScreen, scope
NOTICE                            # Windows redistributables
```

---

### Task 1: Windows dependency fetcher with layout self-test

**Files:**
- Create: `scripts/fetch-win-deps.py`
- Modify: `scripts/fetch-parakeet.sh`

**Interfaces:**
- Produces: `fetch(dest: Path, only: Option<&str>) -> None`; CLI `python scripts/fetch-win-deps.py --dest vendor/win64 [--skip-webview2] [--models-dir models]`; layout `vendor/win64/{UniversalSTT deps…}`; exit non-zero on any failed download/hash.
- Constants: parakeet v0.6.1 asset URLs, ORT 1.31.1, FFmpeg BtbN `latest` LGPL shared, WebView2 `https://go.microsoft.com/fwlink/p/?LinkId=2124703`, models from HF revision `c77726e46ceaab96b5a5a3328915edb44c73209d` and Who `097d29a140dd19e220cff132e455e515d0e47e59`.

- [ ] **Step 1: Failing self-test**

```python
def test_self_test_layout(tmp_path):
    rc = run([sys.executable, SCRIPT, "--self-test", "--root", str(tmp_path)])
    assert rc == 0
    assert (tmp_path / "vendor" / "win64" / "parakeet_cpu.dll").exists()
    assert (tmp_path / "vendor" / "win64" / "parakeet_vulkan.dll").exists()
```

Run: `python3 -m pytest scripts/tests/test_fetch_win_deps.py -q`
Expected: FAIL (script absent).

- [ ] **Step 2: Implement**

Download to a temp dir, extract with `zipfile`, find each `parakeet.dll` and copy it under the canonical name. FFmpeg: copy `bin/ffmpeg.exe`, `bin/ffprobe.exe` and every `bin/*.dll`. ORT: copy `lib/onnxruntime.dll`. WebView2 installer saves as the documented filename. Models: download each manifest entry, verify `bytes` and `sha256`, write into `models/`.

`--self-test` creates dummy 1-byte files with the exact expected names in a temp root and calls `validate_layout`, without network.

- [ ] **Step 3: Fix the local Windows branch of `fetch-parakeet.sh`**

The current `fetch_lib "*.dll"` copies both CPU and Vulkan as `parakeet.dll`, so the second overwrites the first. After each Windows fetch, rename to `parakeet_cpu.dll` / `parakeet_vulkan.dll`; skip only when the canonical name already exists.

- [ ] **Step 4: PASS**

Run: `python3 -m pytest scripts/tests/test_fetch_win_deps.py -q`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add scripts/fetch-win-deps.py scripts/fetch-parakeet.sh scripts/tests
git commit -m "build: windows dependency fetcher"
```

---

### Task 2: Portable ZIP builder with self-test

**Files:**
- Create: `scripts/package_release.py`, `scripts/tests/test_package_release.py`

**Interfaces:**
- Produces: `validate_layout(root: Path) -> list[str]`; `verify_models(models: Path, manifest: Path) -> None`; `build_zip(src: Path, out: Path, version: str) -> Path`; CLI `package_release.py --deps vendor/win64 --exe target/.../UniversalSTT.exe --out dist --version 0.1.0 [--self-test]`.

- [ ] **Step 1: Failing tests**

```python
def test_self_test_rejects_missing_dependency(tmp_path):
    (tmp_path / "vendor").mkdir()
    rc = run([sys.executable, SCRIPT, "--self-test", "--root", str(tmp_path), "--case", "missing"])
    assert rc != 0

def test_self_test_rejects_bad_hash(tmp_path):
    rc = run([... "--case", "bad-hash"])
    assert rc != 0

def test_self_test_zip64_flags(tmp_path):
    out = run_self_test(tmp_path, case="large")
    with zipfile.ZipFile(out) as z:
        info = z.getinfo("models/redux-packed.gguf")
        assert info.compress_type == zipfile.ZIP_STORED
        assert any(i.file_size > 0xFFFFFFFF for i in z.infolist()) or z.start_dir > 0xFFFFFFFF
```

The "large" case uses sparse metadata (declared size) rather than allocating 4 GB.

- [ ] **Step 2: Run — FAIL**

Run: `python3 -m pytest scripts/tests/test_package_release.py -q`
Expected: FAIL.

- [ ] **Step 3: Implement**

`validate_layout` checks every required name, plus absence of `parakeet.dll`, plus that `models/` has exactly the manifest files. `verify_models` streams SHA-256. `build_zip` writes `ZIP_STORED` for `.gguf`/`.onnx`/`.f16`/`.f32`, `ZIP_DEFLATED` for the rest, `allowZip64=True`; includes `version.txt` with `version=<arg>` and commit if `GITHUB_SHA` is set. `SHA256SUMS` is written next to the ZIP. `--self-test` generates the fixture tree, runs the three cases and exits 0 only if all behave.

- [ ] **Step 4: PASS**

Run: `python3 -m pytest scripts/tests/test_package_release.py -q`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add scripts/package_release.py scripts/tests
git commit -m "build: portable zip builder with manifest verification"
```

---

### Task 3: Local Linux package dry-run

**Files:**
- No production change; the existing `--self-test` covers the Linux dry run

- [ ] **Step 1: Dry-run**

Build the Linux desktop binary (`dx build --platform desktop --release`), copy it as `UniversalSTT.exe` into a temp tree with neutralized deps, and run `package_release.py` against that tree. Assert the ZIP has the full layout. This proves the script, not the Windows binary.

- [ ] **Step 2: Commit**

```bash
git commit -am "build: linux dry-run path"
```

---

### Task 4: CI workflow

**Files:**
- Create: `.github/workflows/ci.yml`

**Interfaces:**
- Jobs: `test` (ubuntu), `web` (ubuntu), `windows-check` (windows).
- `test`: fmt, clippy `-D warnings`, `cargo test --workspace` (install webkit2gtk/xdo deps on ubuntu).
- `web`: `dx build --platform web --release`, `npm ci`, `npx playwright install --with-deps chromium`, `npm test`.
- `windows-check`: `cargo test --workspace` (no gated tests).

- [ ] **Step 1: Write workflow**

Use `actions/checkout@v4`, `dtolnay/rust-toolchain@stable` with `components: clippy, rustfmt`, `Swatinem/rust-cache@v2`, `actions/setup-node@v4`. Ubuntu installs `libwebkit2gtk-4.1-dev libgtk-3-dev libxdo-dev libssl-dev`. Pin action major versions; no secrets.

- [ ] **Step 2: Validate YAML**

Run: `python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/ci.yml'))"`
Expected: no error.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: unit, web and windows checks"
```

---

### Task 5: Release workflow

**Files:**
- Create: `.github/workflows/release.yml`

**Interfaces:**
- Trigger `push: tags: ['v*']`, `permissions: contents: write`.
- Job `windows-release` (windows-latest): checkout, Rust, dx 0.7.10, `dx build --platform desktop --release` in `crates/app`, `python scripts/fetch-win-deps.py --dest vendor/win64`, locate `UniversalSTT.exe`, `python scripts/package_release.py`, `gh release create/upload`.
- Artifacts: portable ZIP, `SHA256SUMS`, `NOTICE`, `LICENSE`.

- [ ] **Step 1: Write workflow**

Cache `~/.cargo`, `target`, `vendor/win64` keyed by lock/manifest. Use `actions/cache@v4`. Locate the exe with PowerShell `Get-ChildItem -Recurse -Filter UniversalSTT.exe target/dx`. `gh release create "$env:GITHUB_REF_NAME" ... --generate-notes`; upload with `gh release upload --clobber`.

- [ ] **Step 2: YAML check**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))"`

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: tag-driven portable windows release"
```

---

### Task 6: Windows build documentation and README

**Files:**
- Create: `docs/BUILD-WINDOWS.md`
- Modify: `README.md`, `NOTICE`

**Interfaces:**
- Produces: manual steps (Rust MSVC, dx, `fetch-win-deps.py`, `package_release.py`), verification checklist, known limits (unsigned, WebView2, no CUDA/slim in v1, Wine lane).

- [ ] **Step 1: Write docs**

Include exact commands, expected ZIP size (~1.1–1.5 GB), SmartScreen text, offline behavior, how to verify SHA-256, where projects/settings live (`Documents\Universal STT`, `%APPDATA%\UniversalSTT`), and the fact that Windows acceptance is still pending on this branch.

- [ ] **Step 2: README**

Replace “next plan” rows for packaging with “ready (CI)”; add download/run section; keep export/packaging limitations explicit.

- [ ] **Step 3: Commit**

```bash
git add docs/BUILD-WINDOWS.md README.md NOTICE
git commit -m "docs: windows packaging and manual build"
```

---

### Task 7: Final verification

- [ ] Run `python3 -m pytest scripts/tests -q`
- [ ] Run `python3 scripts/package_release.py --self-test --root /tmp/ustt-pkg-self-test`
- [ ] Run `cargo test --workspace` with `LIBRARY_PATH=/tmp/opencode/xdo`
- [ ] Run `npm test`
- [ ] Validate both workflow YAMLs
- [ ] Record in the ledger: Windows binary execution under Wine/Proton not performed — release gate.

```bash
git commit --allow-empty -m "chore: packaging milestone verification"
```

---

## Self-Review

**Spec coverage:** §12 artifact list, portable ZIP, models, licenses/NOTICE, tag CI, no signing; §11 pinned revisions and hashes. Slim archive and CUDA asset are explicitly deferred with README documentation; §13.2 Wine acceptance remains manual.

**Type consistency:** Canonical DLL names are the ones `backend.rs` loads; model names come from `manifest.json`; version comes from the CLI arg and `Cargo.toml`; ZIP root layout matches `Tools::locate`, `models_dir`, `parakeet_lib_path`.

**Review focus tests:** missing dep/hash (Task 2), ZIP64/stored (Task 2), layout (Task 1), CI scope (Task 4/5).
