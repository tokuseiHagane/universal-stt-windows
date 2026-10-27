#!/usr/bin/env python3
"""Downloads pinned Windows binaries and models into a portable layout."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import sys
import tempfile
import traceback
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
    "https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-2026-10-27-18-38/"
    "ffmpeg-n8.1.3-24-g92611900bb-win64-lgpl-shared-8.1.zip"
)
FFMPEG_SHA256 = "6c6167da57decb941a0466b60d95f7f51e4edd6b2d79c24dc4860c1c15701292"
WEBVIEW2_VERSION = "156.0.4314.50"
WEBVIEW2_RUNTIME = (
    "https://api.nuget.org/v3-flatcontainer/webview2.runtime.x64/"
    f"{WEBVIEW2_VERSION}/webview2.runtime.x64.{WEBVIEW2_VERSION}.nupkg"
)
WEBVIEW2_CORE = (
    "https://api.nuget.org/v3-flatcontainer/webview2.runtime.x64.core/"
    f"{WEBVIEW2_VERSION}/webview2.runtime.x64.core.{WEBVIEW2_VERSION}.nupkg"
)
WEBVIEW2_FOLDER = "WebView2Runtime"
VCREDIST = "https://aka.ms/vs/17/release/vc_redist.x64.exe"
VC_DLLS = ["msvcp140.dll", "msvcp140_1.dll", "vcruntime140.dll", "vcruntime140_1.dll"]
WEBVIEW2_SDK = "1.0.4314.35"
WEBVIEW2_NUGET = (
    "https://api.nuget.org/v3-flatcontainer/microsoft.web.webview2/"
    f"{WEBVIEW2_SDK}/microsoft.web.webview2.{WEBVIEW2_SDK}.nupkg"
)
WEBVIEW2_LOADER = "WebView2Loader.dll"
WHO_REV = "097d29a140dd19e220cff132e455e515d0e47e59"
ASR_REV = "c77726e46ceaab96b5a5a3328915edb44c73209d"


def download(url: str, target: Path, attempts: int = 3) -> None:
    target.parent.mkdir(parents=True, exist_ok=True)
    last: Exception | None = None
    for attempt in range(1, attempts + 1):
        print(f"fetching {url} (attempt {attempt}/{attempts})")
        try:
            with urllib.request.urlopen(url, timeout=300) as response, target.open("wb") as out:
                shutil.copyfileobj(response, out, 1 << 20)
            return
        except Exception as error:  # noqa: BLE001 - retry every transport failure
            last = error
            target.unlink(missing_ok=True)
            if attempt < attempts:
                import time

                time.sleep(2 * attempt)
    raise SystemExit(f"download failed: {url}: {last}")


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
        if (dest / canonical).is_file():
            print(f"present: {canonical}")
            continue
        archive = work / Path(url).name
        download(url, archive)
        extract = work / f"extract-{canonical}"
        extract_zip(archive, extract)
        matches = list(extract.rglob("parakeet.dll"))
        if len(matches) != 1:
            raise SystemExit(f"{url}: expected one parakeet.dll, found {len(matches)}")
        shutil.copy2(matches[0], dest / canonical)


def install_ort(dest: Path, work: Path) -> None:
    if (dest / "onnxruntime.dll").is_file():
        print("present: onnxruntime.dll")
        return
    archive = work / "onnxruntime.zip"
    download(ORT, archive)
    extract = work / "ort"
    extract_zip(archive, extract)
    matches = list(extract.rglob("onnxruntime.dll"))
    if not matches:
        raise SystemExit("onnxruntime.dll not found in the ONNX Runtime archive")
    shutil.copy2(matches[0], dest / "onnxruntime.dll")


def install_ffmpeg(dest: Path, work: Path) -> None:
    if (dest / "ffmpeg.exe").is_file() and (dest / "ffprobe.exe").is_file():
        if any(dest.glob("swscale-*.dll")):
            print("present: ffmpeg")
            return
    archive = work / "ffmpeg.zip"
    download(FFMPEG, archive)
    if sha256(archive) != FFMPEG_SHA256:
        raise SystemExit("ffmpeg archive sha256 mismatch")
    extract = work / "ffmpeg"
    extract_zip(archive, extract)
    bins = list(extract.rglob("ffmpeg.exe"))
    if not bins:
        raise SystemExit("ffmpeg.exe not found in the FFmpeg archive")
    bin_dir = bins[0].parent
    for name in ("ffmpeg.exe", "ffprobe.exe"):
        source = bin_dir / name
        if not source.is_file():
            raise SystemExit(f"{name} not found in the FFmpeg archive")
        shutil.copy2(source, dest / name)
    for dll in bin_dir.glob("*.dll"):
        shutil.copy2(dll, dest / dll.name)


def install_webview2(dest: Path, work: Path) -> None:
    runtime = dest / WEBVIEW2_FOLDER
    if (runtime / "msedgewebview2.exe").is_file():
        print(f"present: {WEBVIEW2_FOLDER}")
    else:
        for name, url in (
            ("webview2-runtime.nupkg", WEBVIEW2_RUNTIME),
            ("webview2-core.nupkg", WEBVIEW2_CORE),
        ):
            package = work / name
            download(url, package)
            extract = work / f"{name}-x"
            extract_zip(package, extract)
            source = extract / "contentFiles" / "any" / "any" / "WebView2"
            if source.is_dir():
                runtime.mkdir(parents=True, exist_ok=True)
                shutil.copytree(source, runtime, dirs_exist_ok=True)
        if not (runtime / "msedgewebview2.exe").is_file():
            raise SystemExit("WebView2 runtime missing msedgewebview2.exe")
    if not (dest / WEBVIEW2_LOADER).is_file():
        package = work / "webview2.nupkg"
        download(WEBVIEW2_NUGET, package)
        extract = work / "webview2-sdk"
        extract_zip(package, extract)
        loader = extract / "build" / "native" / "x64" / WEBVIEW2_LOADER
        if not loader.is_file():
            raise SystemExit(f"{WEBVIEW2_LOADER} not found in the WebView2 SDK package")
        shutil.copy2(loader, dest / WEBVIEW2_LOADER)


def install_vc_runtime(dest: Path, work: Path) -> None:
    if all((dest / name).is_file() for name in VC_DLLS):
        print("present: VC runtime")
        return
    external = os.environ.get("USTT_VC_DLL_DIR")
    if external:
        source = Path(external)
    elif os.name == "nt":
        installer = work / "vc_redist.x64.exe"
        download(VCREDIST, installer)
        import subprocess

        subprocess.run(
            [str(installer), "/quiet", "/norestart"], check=False, timeout=600
        )
        source = Path(os.environ["SystemRoot"]) / "System32"
    else:
        raise SystemExit(
            "VC runtime DLLs need Windows or USTT_VC_DLL_DIR pointing at the four DLLs"
        )
    for name in VC_DLLS:
        path = source / name
        if not path.is_file():
            raise SystemExit(f"VC runtime file missing: {path}")
        shutil.copy2(path, dest / name)


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
    WEBVIEW2_LOADER,
    *VC_DLLS,
    f"{WEBVIEW2_FOLDER}/msedgewebview2.exe",
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
        path = deps / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(b"x")
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
    parser.add_argument("--root", type=Path)
    args = parser.parse_args()
    if args.self_test:
        if args.root is None:
            parser.error("--self-test requires --root")
        return self_test(args.root)
    dest = args.dest
    dest.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        install_parakeet(dest, work)
        install_ort(dest, work)
        install_ffmpeg(dest, work)
        install_vc_runtime(dest, work)
        if not args.skip_webview2:
            install_webview2(dest, work)
    if not args.skip_models:
        install_models(dest, args.manifest)
    problems = validate_layout(dest, None if args.skip_models else args.manifest)
    if args.skip_webview2:
        problems = [
            p
            for p in problems
            if p != WEBVIEW2_LOADER and not p.startswith(f"{WEBVIEW2_FOLDER}/")
        ]
    if problems:
        print("layout problems:", ", ".join(problems), file=sys.stderr)
        return 1
    print(f"windows dependencies ready: {dest}")
    return 0


def report_failure(message: str) -> None:
    print(message, file=sys.stderr)
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        with open(summary, "a", encoding="utf-8") as handle:
            handle.write(f"### {Path(__file__).name} failed\n```\n{message[:4000]}\n```\n")


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except SystemExit as error:
        if error.code not in (0, None):
            report_failure(str(error.code))
        raise
    except Exception:
        report_failure(traceback.format_exc())
        raise
