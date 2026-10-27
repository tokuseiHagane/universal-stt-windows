#!/usr/bin/env python3
"""Downloads pinned Windows binaries and models into a portable layout."""
from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import sys
import tempfile
import urllib.request
import zipfile
from pathlib import Path

PARAKEET = "https://github.com/mudler/parakeet.cpp/releases/download/v0.6.1"
PARAKEET_ASSETS = {
    "parakeet_cpu.dll": f"{PARAKEET}/parakeet-v0.6.1-lib-win-cpu-x64.zip",
    "parakeet_vulkan.dll": f"{PARAKEET}/parakeet-v0.6.1-lib-win-vulkan-x64.zip",
}
ORT = "https://github.com/microsoft/onnxruntime/releases/download/v1.31.1/onnxruntime-win-x64-1.31.1.zip"
FFMPEG = (
    "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/"
    "ffmpeg-master-latest-win64-lgpl-shared.zip"
)
WEBVIEW2 = "https://go.microsoft.com/fwlink/p/?LinkId=2124703"
WEBVIEW2_NAME = "MicrosoftEdgeWebView2RuntimeInstallerX64.exe"
WHO_REV = "097d29a140dd19e220cff132e455e515d0e47e59"
ASR_REV = "c77726e46ceaab96b5a5a3328915edb44c73209d"


def download(url: str, target: Path) -> None:
    target.parent.mkdir(parents=True, exist_ok=True)
    print(f"fetching {url}")
    with urllib.request.urlopen(url, timeout=120) as response, target.open("wb") as out:
        shutil.copyfileobj(response, out, 1 << 20)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def extract_zip(archive: Path, dest: Path) -> None:
    with zipfile.ZipFile(archive) as zf:
        zf.extractall(dest)


def install_parakeet(dest: Path, work: Path) -> None:
    for canonical, url in PARAKEET_ASSETS.items():
        archive = work / Path(url).name
        download(url, archive)
        extract = work / f"extract-{canonical}"
        extract_zip(archive, extract)
        matches = list(extract.rglob("parakeet.dll"))
        if len(matches) != 1:
            raise SystemExit(f"{url}: expected one parakeet.dll, found {len(matches)}")
        shutil.copy2(matches[0], dest / canonical)


def install_ort(dest: Path, work: Path) -> None:
    archive = work / "onnxruntime.zip"
    download(ORT, archive)
    extract = work / "ort"
    extract_zip(archive, extract)
    matches = list(extract.rglob("onnxruntime.dll"))
    if not matches:
        raise SystemExit("onnxruntime.dll not found in the ONNX Runtime archive")
    shutil.copy2(matches[0], dest / "onnxruntime.dll")


def install_ffmpeg(dest: Path, work: Path) -> None:
    archive = work / "ffmpeg.zip"
    download(FFMPEG, archive)
    extract = work / "ffmpeg"
    extract_zip(archive, extract)
    bins = list(extract.rglob("ffmpeg.exe"))
    if not bins:
        raise SystemExit("ffmpeg.exe not found in the FFmpeg archive")
    bin_dir = bins[0].parent
    for name in ("ffmpeg.exe", "ffprobe.exe"):
        shutil.copy2(bin_dir / name, dest / name)
    for dll in bin_dir.glob("*.dll"):
        shutil.copy2(dll, dest / dll.name)


def install_webview2(dest: Path, work: Path) -> None:
    download(WEBVIEW2, work / "webview2.exe")
    shutil.copy2(work / "webview2.exe", dest / WEBVIEW2_NAME)


def model_url(name: str) -> str:
    if name.startswith("who/"):
        base = "https://huggingface.co/desert-ant-labs/who/resolve"
        return f"{base}/{WHO_REV}/onnx-int8/{Path(name).name}"
    base = "https://huggingface.co/mudler/parakeet-cpp-gguf/resolve"
    return f"{base}/{ASR_REV}/{name}"


def install_models(dest: Path, manifest: Path) -> None:
    data = json.loads(manifest.read_text())
    for entry in data["files"]:
        target = dest / "models" / entry["name"]
        if not target.is_file() or sha256(target) != entry["sha256"]:
            download(model_url(entry["name"]), target)
        if target.stat().st_size != entry["bytes"] or sha256(target) != entry["sha256"]:
            raise SystemExit(f"model hash mismatch: {entry['name']}")


REQUIRED = [
    "parakeet_cpu.dll",
    "parakeet_vulkan.dll",
    "onnxruntime.dll",
    "ffmpeg.exe",
    "ffprobe.exe",
    WEBVIEW2_NAME,
]


def validate_layout(root: Path, manifest: Path | None = None) -> list[str]:
    missing = [name for name in REQUIRED if not (root / name).is_file()]
    if (root / "parakeet.dll").exists():
        missing.append("unexpected parakeet.dll (expected canonical names)")
    if not any((root).glob("avcodec-*.dll")):
        missing.append("avcodec-*.dll")
    if manifest and manifest.is_file():
        for entry in json.loads(manifest.read_text())["files"]:
            path = root / "models" / entry["name"]
            if not path.is_file():
                missing.append(f"models/{entry['name']}")
            elif path.stat().st_size != entry["bytes"] or sha256(path) != entry["sha256"]:
                missing.append(f"models/{entry['name']} (hash mismatch)")
    return missing


def self_test(root: Path) -> int:
    deps = root / "vendor" / "win64"
    models = deps / "models" / "who"
    models.mkdir(parents=True, exist_ok=True)
    (deps / "models").mkdir(parents=True, exist_ok=True)
    for name in REQUIRED:
        (deps / name).write_bytes(b"x")
    (deps / "avcodec-62.dll").write_bytes(b"x")
    files = []
    for name in ("redux-packed.gguf", "silero-vad-f16.gguf"):
        path = deps / "models" / name
        path.write_bytes(name.encode())
        files.append({"name": name, "sha256": sha256(path), "bytes": path.stat().st_size})
    manifest = deps / "models" / "manifest.json"
    manifest.write_text(json.dumps({"files": files}))
    problems = validate_layout(deps, manifest)
    if problems:
        print("self-test layout problems:", problems)
        return 1
    print(f"self-test ok: {deps}")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dest", type=Path, default=Path("vendor/win64"))
    parser.add_argument("--manifest", type=Path, default=Path("models/manifest.json"))
    parser.add_argument("--skip-webview2", action="store_true")
    parser.add_argument("--skip-models", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--root", type=Path, default=Path("."))
    args = parser.parse_args()
    if args.self_test:
        return self_test(args.root)
    dest = args.dest
    dest.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        install_parakeet(dest, work)
        install_ort(dest, work)
        install_ffmpeg(dest, work)
        if not args.skip_webview2:
            install_webview2(dest, work)
    if not args.skip_models:
        install_models(dest, args.manifest)
    problems = validate_layout(dest, None if args.skip_models else args.manifest)
    if args.skip_webview2:
        problems = [p for p in problems if p != WEBVIEW2_NAME]
    if problems:
        print("layout problems:", ", ".join(problems), file=sys.stderr)
        return 1
    print(f"windows dependencies ready: {dest}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
