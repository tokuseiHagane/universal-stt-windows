import importlib.util
import subprocess
import sys
import zipfile
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "fetch-win-deps.py"


def load_script():
    spec = importlib.util.spec_from_file_location("fetch_win_deps", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def run(*args: str) -> int:
    return subprocess.run([sys.executable, str(SCRIPT), *args], check=False).returncode


def test_self_test_layout(tmp_path: Path) -> None:
    rc = run("--self-test", "--root", str(tmp_path))
    assert rc == 0
    deps = tmp_path / "vendor" / "win64"
    assert (deps / "parakeet_cpu.dll").exists()
    assert (deps / "parakeet_vulkan.dll").exists()
    assert (deps / "avcodec-62.dll").exists()
    assert (deps / "yt-dlp.exe").exists()
    assert (deps / "deno.exe").exists()
    assert (deps / "models" / "manifest.json").exists()


def test_downloader_is_pinned_and_extracts_only_deno(tmp_path: Path, monkeypatch) -> None:
    module = load_script()
    assert module.YT_DLP_VERSION == "2026.10.26"
    assert module.DENO_VERSION == "2.9.8"
    assert module.YT_DLP.endswith("/2026.10.26/yt-dlp.exe")
    assert module.DENO.endswith("/v2.9.8/deno-x86_64-pc-windows-msvc.zip")

    def fake_download(url: str, target: Path) -> None:
        target.parent.mkdir(parents=True, exist_ok=True)
        if url == module.YT_DLP:
            target.write_bytes(b"yt-dlp")
        else:
            with zipfile.ZipFile(target, "w") as archive:
                archive.writestr("deno.exe", "deno")
                archive.writestr("README.txt", "do not ship")

    monkeypatch.setattr(module, "download", fake_download)
    monkeypatch.setattr(module, "sha256", lambda _path: "valid")
    monkeypatch.setattr(module, "YT_DLP_SHA256", "valid")
    monkeypatch.setattr(module, "DENO_SHA256", "valid")
    module.install_downloaders(tmp_path / "deps", tmp_path / "work")

    assert (tmp_path / "deps" / "yt-dlp.exe").read_bytes() == b"yt-dlp"
    assert (tmp_path / "deps" / "deno.exe").read_bytes() == b"deno"
    assert not (tmp_path / "deps" / "README.txt").exists()


def test_notices_cover_downloader_runtime() -> None:
    notice = (SCRIPT.parents[1] / "NOTICE").read_text()
    assert "yt-dlp" in notice
    assert "Unlicense" in notice
    assert "Deno" in notice
    assert "MIT" in notice


def test_webview2_core_package_only_adds_msedge(tmp_path: Path, monkeypatch) -> None:
    module = load_script()

    def fake_download(_url: str, target: Path) -> None:
        target.parent.mkdir(parents=True, exist_ok=True)
        with zipfile.ZipFile(target, "w") as package:
            if target.name == "webview2-runtime.nupkg":
                package.writestr(
                    "contentFiles/any/any/WebView2/msedgewebview2.exe", "runtime"
                )
            elif target.name == "webview2-core.nupkg":
                package.writestr("contentFiles/any/any/WebView2/msedge.dll", "core")
            else:
                package.writestr("build/native/x64/WebView2Loader.dll", "loader")

    monkeypatch.setattr(module, "download", fake_download)
    module.install_webview2(tmp_path / "deps", tmp_path / "work")

    runtime = tmp_path / "deps" / "WebView2Runtime"
    assert (runtime / "msedgewebview2.exe").read_text() == "runtime"
    assert (runtime / "msedge.dll").read_text() == "core"
    assert (tmp_path / "deps" / "WebView2Loader.dll").read_text() == "loader"
