#!/usr/bin/env bash
# Downloads parakeet.cpp v0.6.1 libraries and the Redux GGUF models.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
libdir="${1:-$root/lib}"
modeldir="${2:-$root/models}"
version="0.6.1"
rev="c77726e46ceaab96b5a5a3328915edb44c73209d"
base="https://github.com/mudler/parakeet.cpp/releases/download/v$version"
hf="https://huggingface.co/mudler/parakeet-cpp-gguf/resolve/$rev"

mkdir -p "$libdir" "$modeldir"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

fetch_lib() {
  local asset="$1" pattern="$2"
  if compgen -G "$libdir/$pattern" > /dev/null; then
    echo "present: $pattern"
    return
  fi
  echo "fetching: $asset"
  curl -L --fail --retry 3 -o "$tmp/$asset" "$base/$asset"
  case "$asset" in
    *.zip) unzip -q -o "$tmp/$asset" -d "$tmp/extract" ;;
    *.tar.gz) mkdir -p "$tmp/extract" && tar -xzf "$tmp/$asset" -C "$tmp/extract" ;;
  esac
  find "$tmp/extract" -type f -name "$pattern" -exec cp {} "$libdir/" \;
  rm -rf "$tmp/extract"
}

case "$(uname -s)" in
  Linux)
    fetch_lib "parakeet-v$version-lib-linux-cpu-x64.tar.gz" "libparakeet.so*"
    ;;
  MINGW*|MSYS*|CYGWIN*)
    fetch_lib "parakeet-v$version-lib-win-cpu-x64.zip" "*.dll"
    fetch_lib "parakeet-v$version-lib-win-vulkan-x64.zip" "*.dll"
    ;;
  *)
    echo "unsupported OS for this script: $(uname -s)" >&2
    exit 1
    ;;
esac

for file in redux-packed.gguf redux-q8_0.gguf silero-vad-f16.gguf; do
  if [ -f "$modeldir/$file" ]; then
    echo "present: $file"
    continue
  fi
  echo "fetching: $file"
  curl -L --fail --retry 3 --progress-bar -o "$modeldir/$file.part" "$hf/$file"
  mv "$modeldir/$file.part" "$modeldir/$file"
done

echo "parakeet.cpp ready in $libdir and $modeldir"
