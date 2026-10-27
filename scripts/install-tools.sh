#!/usr/bin/env bash
# Installs the desktop/web UI toolchain. Safe to run repeatedly.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"

if command -v rustup >/dev/null 2>&1; then
  rustup target add wasm32-unknown-unknown
else
  cat <<'MSG'
rustup is not installed. Install it from https://rustup.rs, then re-run this script.
On Arch Linux: sudo pacman -S rust-wasm
MSG
fi

if ! command -v dx >/dev/null 2>&1; then
  cargo install dioxus-cli --version 0.7.10 --locked
fi

if [ -f "$root/package.json" ]; then
  (cd "$root" && npm install)
  (cd "$root" && npx playwright install chromium)
fi

cargo --version
dx --version
