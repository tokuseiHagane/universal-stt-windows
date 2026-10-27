#!/usr/bin/env python3
"""Validates the Windows dependency tree and builds the portable ZIP."""
from __future__ import annotations

import argparse
import hashlib
import os
import json
import shutil
import sys
import tempfile
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REQUIRED = [
    "parakeet_cpu.dll",
    "parakeet_vulkan.dll",
    "onnxruntime.dll",
    "ffmpeg.exe",
    "ffprobe.exe",
    "MicrosoftEdgeWebView2RuntimeInstallerX64.exe",
    "VC_redist.x64.exe",
]
STORED_SUFFIXES = {".gguf", ".onnx", ".f16", ".f32"}
ZIP64_LIMIT = 0xFFFFFFFF


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def verify_models(models: Path, manifest: Path) -> None:
    for entry in json.loads(manifest.read_text())["files"]:
        path = models / entry["name"]
        if not path.is_file():
            raise SystemExit(f"missing model: {path}")
        if path.stat().st_size != entry["bytes"]:
            raise SystemExit(f"size mismatch: {path}")
        if sha256(path) != entry["sha256"]:
            raise SystemExit(f"sha256 mismatch: {path}")


def validate_layout(root: Path, manifest: Path | None) -> list[str]:
    problems = [name for name in REQUIRED if not (root / name).is_file()]
    if (root / "parakeet.dll").exists():
        problems.append("unexpected parakeet.dll (expected parakeet_cpu.dll/parakeet_vulkan.dll)")
    if not any(root.glob("avcodec-*.dll")):
        problems.append("avcodec-*.dll")
    if manifest is not None and manifest.is_file():
        for entry in json.loads(manifest.read_text())["files"]:
            path = root / "models" / entry["name"]
            if not path.is_file():
                problems.append(f"models/{entry['name']}")
    return problems


def build_zip(
    deps: Path,
    exe: Path,
    out_dir: Path,
    version: str,
    manifest: Path,
) -> Path:
    verify_models(deps / "models", manifest)
    if not exe.is_file():
        raise SystemExit(f"app executable missing: {exe}")
    if not version.strip():
        raise SystemExit("version must not be empty")
    out_dir.mkdir(parents=True, exist_ok=True)
    entries = json.loads(manifest.read_text())["files"]
    with tempfile.TemporaryDirectory() as tmp:
        stage = Path(tmp) / "UniversalSTT"
        stage.mkdir()
        allowed = list(REQUIRED) + ["parakeet.dll"]
        allowed += [p.name for p in deps.glob("av*.dll")]
        for name in allowed:
            source = deps / name
            if source.is_file():
                shutil.copy2(source, stage / name)
        (stage / "models").mkdir()
        for entry in entries:
            source = deps / "models" / entry["name"]
            target = stage / "models" / entry["name"]
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)
        shutil.copy2(manifest, stage / "models" / "manifest.json")
        shutil.copy2(exe, stage / "UniversalSTT.exe")
        for name in ("LICENSE", "NOTICE", "README.md"):
            source = ROOT / name
            if source.is_file():
                shutil.copy2(source, stage / name)
        commit = os.environ.get("GITHUB_SHA", "local")
        (stage / "version.txt").write_text(f"version={version}\ncommit={commit}\n")
        name = f"UniversalSTT-{version}-win64-portable.zip"
        target = out_dir / name
        stage_files = sorted(p for p in stage.rglob("*") if p.is_file())
        with zipfile.ZipFile(target, "w", allowZip64=True) as zf:
            for path in stage_files:
                relative = path.relative_to(stage).as_posix()
                method = (
                    zipfile.ZIP_STORED
                    if path.suffix.lower() in STORED_SUFFIXES
                    else zipfile.ZIP_DEFLATED
                )
                info = zipfile.ZipInfo(relative)
                info.compress_type = method
                info.external_attr = 0o644 << 16
                info.file_size = path.stat().st_size
                with path.open("rb") as source, zf.open(info, "w") as dest:
                    shutil.copyfileobj(source, dest, 1 << 20)
        digest = sha256(target)
        (out_dir / "SHA256SUMS").write_text(f"{digest}  {name}\n")
        print(f"wrote {target} ({target.stat().st_size} bytes)")
        return target


def make_fixture(root: Path) -> tuple[Path, Path, Path]:
    deps = root / "vendor" / "win64"
    (deps / "models" / "who").mkdir(parents=True, exist_ok=True)
    for name in REQUIRED:
        (deps / name).write_bytes(b"fixture")
    (deps / "avcodec-62.dll").write_bytes(b"fixture")
    files = []
    for name in ("redux-packed.gguf", "silero-vad-f16.gguf", "who/who_meta.json"):
        path = deps / "models" / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(("fixture-" + name).encode())
        files.append({"name": name, "sha256": sha256(path), "bytes": path.stat().st_size})
    manifest = root / "manifest.json"
    manifest.write_text(json.dumps({"files": files}))
    exe = root / "UniversalSTT.exe"
    exe.write_bytes(b"MZ fixture")
    return deps, manifest, exe


def self_test(root: Path) -> int:
    root.mkdir(parents=True, exist_ok=True)
    deps, manifest, exe = make_fixture(root)
    problems = validate_layout(deps, manifest)
    if problems:
        print("layout problems:", problems)
        return 1
    archive = build_zip(deps, exe, root / "dist", "0.0.0-test", manifest)
    with zipfile.ZipFile(archive) as zf:
        names = set(zf.namelist())
        for required in ("UniversalSTT.exe", "models/redux-packed.gguf", "version.txt", "LICENSE"):
            if required not in names:
                print("missing zip entry:", required)
                return 1
        gguf = zf.getinfo("models/redux-packed.gguf")
        if gguf.compress_type != zipfile.ZIP_STORED:
            print("model is not stored")
            return 1
        exe_info = zf.getinfo("UniversalSTT.exe")
        if exe_info.compress_type != zipfile.ZIP_DEFLATED:
            print("exe is not deflated")
            return 1
    missing_tree = root / "missing"
    shutil.copytree(deps, missing_tree)
    (missing_tree / "ffmpeg.exe").unlink()
    if not any("ffmpeg.exe" in p for p in validate_layout(missing_tree, manifest)):
        print("missing dependency was not detected")
        return 1
    bad_tree = root / "bad"
    shutil.copytree(deps, bad_tree)
    (bad_tree / "models" / "redux-packed.gguf").write_bytes(b"tampered")
    try:
        verify_models(bad_tree / "models", manifest)
    except SystemExit:
        pass
    else:
        print("bad hash was not detected")
        return 1
    print(f"package self-test ok: {archive}")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--deps", type=Path, default=Path("vendor/win64"))
    parser.add_argument("--exe", type=Path)
    parser.add_argument("--manifest", type=Path, default=Path("models/manifest.json"))
    parser.add_argument("--out", type=Path, default=Path("dist"))
    parser.add_argument("--version", default="0.1.0")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--root", type=Path)
    args = parser.parse_args()
    if args.self_test:
        if args.root is None:
            parser.error("--self-test requires --root")
        return self_test(args.root)
    if args.exe is None:
        parser.error("--exe is required")
    problems = validate_layout(args.deps, args.manifest)
    if problems:
        print("layout problems:", ", ".join(problems), file=sys.stderr)
        return 1
    build_zip(args.deps, args.exe, args.out, args.version, args.manifest)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
