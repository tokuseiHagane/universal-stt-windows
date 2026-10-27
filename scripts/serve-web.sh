#!/usr/bin/env bash
# Builds the wasm demo and serves it for Playwright.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
export PATH="${HOME}/.cargo/bin:${PATH}"
(cd crates/app && dx build --platform web --release)
public="$(ls -td target/dx/*/*/web/public 2>/dev/null | head -1)"
[ -n "$public" ] || { echo "web build output not found" >&2; exit 1; }
echo "serving $public"
exec python3 "$root/scripts/serve-web.py" "$public" 4173
