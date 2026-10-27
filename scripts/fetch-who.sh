#!/usr/bin/env bash
# Downloads the public Who ONNX-int8 model into models/who.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
dest="${1:-$root/models/who}"
rev="097d29a140dd19e220cff132e455e515d0e47e59"
base="https://huggingface.co/desert-ant-labs/who/resolve/$rev/onnx-int8"

files=(
  encoder_6000.onnx
  encoder_30000.onnx
  queries_6000.onnx
  queries_30000.onnx
  voice_3256.onnx
  who_embed.f16
  who_mel_banks.f32
  who_voice_banks.f32
  who_meta.json
)

mkdir -p "$dest"
for file in "${files[@]}"; do
  if [ -f "$dest/$file" ]; then
    echo "present: $file"
    continue
  fi
  echo "fetching: $file"
  curl -L --fail --retry 3 --progress-bar -o "$dest/$file.part" "$base/$file"
  mv "$dest/$file.part" "$dest/$file"
done
echo "Who model ready in $dest"
