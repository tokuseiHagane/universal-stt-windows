import importlib.util
import subprocess
import sys
import zipfile
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "package_release.py"
spec = importlib.util.spec_from_file_location("package_release", SCRIPT)
package = importlib.util.module_from_spec(spec)
spec.loader.exec_module(package)


def test_self_test_rejects_missing_dependency(tmp_path: Path) -> None:
    deps, manifest, _ = package.make_fixture(tmp_path)
    (deps / "ffmpeg.exe").unlink()
    assert "ffmpeg.exe" in package.validate_layout(deps, manifest)


def test_self_test_rejects_bad_hash(tmp_path: Path) -> None:
    deps, manifest, _ = package.make_fixture(tmp_path)
    (deps / "models" / "redux-packed.gguf").write_bytes(b"tampered")
    try:
        package.verify_models(deps / "models", manifest)
    except SystemExit:
        pass
    else:
        raise AssertionError("bad hash accepted")


def test_build_zip_layout_and_compression(tmp_path: Path) -> None:
    deps, manifest, exe = package.make_fixture(tmp_path)
    archive = package.build_zip(deps, exe, tmp_path / "dist", "1.2.3", manifest)
    assert archive.name == "UniversalSTT-1.2.3-win64-portable.zip"
    with zipfile.ZipFile(archive) as zf:
        assert "UniversalSTT.exe" in zf.namelist()
        assert "models/who/who_meta.json" in zf.namelist()
        assert "models/manifest.json" in zf.namelist()
        assert "swscale-9.dll" in zf.namelist()
        assert zf.getinfo("models/redux-packed.gguf").compress_type == zipfile.ZIP_STORED
        assert zf.getinfo("UniversalSTT.exe").compress_type == zipfile.ZIP_DEFLATED


def test_build_zip_skips_stray_files(tmp_path: Path) -> None:
    deps, manifest, exe = package.make_fixture(tmp_path)
    (deps / "leftover.part").write_bytes(b"junk")
    archive = package.build_zip(deps, exe, tmp_path / "dist", "1.0.0", manifest)
    with zipfile.ZipFile(archive) as zf:
        assert "leftover.part" not in zf.namelist()
        assert zf.getinfo("models/redux-packed.gguf").file_size == len(b"fixture-redux-packed.gguf")


def test_empty_version_is_rejected(tmp_path: Path) -> None:
    deps, manifest, exe = package.make_fixture(tmp_path)
    try:
        package.build_zip(deps, exe, tmp_path / "dist", " ", manifest)
    except SystemExit:
        pass
    else:
        raise AssertionError("empty version accepted")


def test_self_test_cli(tmp_path: Path) -> None:
    result = subprocess.run(
        [sys.executable, str(SCRIPT), "--self-test", "--root", str(tmp_path)],
        check=False,
    )
    assert result.returncode == 0
