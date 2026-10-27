#!/usr/bin/env bash
# Writes models/manifest.json in the shape ustt_pipeline::models::verify_models reads.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
dir="${1:-$root/models}"

python3 - "$dir" <<'PY'
import hashlib, json, pathlib, sys

root = pathlib.Path(sys.argv[1])
files = [
    "redux-packed.gguf",
    "redux-q8_0.gguf",
    "silero-vad-f16.gguf",
    "who/encoder_6000.onnx",
    "who/encoder_30000.onnx",
    "who/queries_6000.onnx",
    "who/queries_30000.onnx",
    "who/voice_3256.onnx",
    "who/who_embed.f16",
    "who/who_mel_banks.f32",
    "who/who_voice_banks.f32",
    "who/who_meta.json",
]
entries = []
for name in files:
    path = root / name
    if not path.is_file():
        sys.exit(f"missing model file: {path}")
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    entries.append({"name": name, "sha256": digest.hexdigest(), "bytes": path.stat().st_size})

out = root / "manifest.json"
out.write_text(json.dumps({"files": entries}, indent=1) + "\n")
print(f"wrote {out} ({len(entries)} files)")
PY
