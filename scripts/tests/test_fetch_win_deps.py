import subprocess
import sys
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "fetch-win-deps.py"


def run(*args: str) -> int:
    return subprocess.run([sys.executable, str(SCRIPT), *args], check=False).returncode


def test_self_test_layout(tmp_path: Path) -> None:
    rc = run("--self-test", "--root", str(tmp_path))
    assert rc == 0
    deps = tmp_path / "vendor" / "win64"
    assert (deps / "parakeet_cpu.dll").exists()
    assert (deps / "parakeet_vulkan.dll").exists()
    assert (deps / "avcodec-62.dll").exists()
    assert (deps / "models" / "manifest.json").exists()
