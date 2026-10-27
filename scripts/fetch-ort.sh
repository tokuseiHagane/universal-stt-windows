#!/usr/bin/env bash
# Downloads a pinned ONNX Runtime CPU build into lib/ for local runs.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
version="1.31.1"
dest="$root/lib"
mkdir -p "$dest"

case "$(uname -s)" in
  Linux)
    name="onnxruntime-linux-x64-$version"
    url="https://github.com/microsoft/onnxruntime/releases/download/v$version/$name.tgz"
    ;;
  MINGW*|MSYS*|CYGWIN*)
    name="onnxruntime-win-x64-$version"
    url="https://github.com/microsoft/onnxruntime/releases/download/v$version/$name.zip"
    ;;
  *)
    echo "unsupported OS for this script: $(uname -s)" >&2
    exit 1
    ;;
esac

archive="$dest/$name.download"
echo "fetching: $url"
curl -L --fail --retry 3 -o "$archive" "$url"

case "$archive" in
  *.tgz)
    tar -xzf "$archive" -C "$dest" --strip-components=1
    ;;
  *.zip)
    unzip -q -o "$archive" -d "$dest"
    mv "$dest/$name"/* "$dest"/
    rmdir "$dest/$name"
    ;;
esac
rm -f "$archive"
echo "ONNX Runtime $version ready in $dest"
