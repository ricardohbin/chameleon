#!/usr/bin/env bash
# Builds the web version into web/dist (serve that folder with any static server, e.g. `python -m http.server -d web/dist`).
#
# One-time setup:
#   rustup target add wasm32-unknown-unknown   (msys2 users: the target ships with the toolchain package)
#   cargo install wasm-bindgen-cli --version <the wasm-bindgen version in Cargo.lock>
#   (optional) wasm-opt from binaryen, for a smaller .wasm
#
# Usage: web/build.sh [--dev]    (default: the `web-release` profile)
set -euo pipefail
cd "$(dirname "$0")/.."

if [[ "${1:-}" == "--dev" ]]; then
  profile=dev; dir=debug; flag=()
else
  profile=web-release; dir=web-release; flag=(--profile web-release)
fi

# The release build embeds assets/world.baked (see build.rs); skip its freshness check only if you must:
#   GHORE_SKIP_BAKE_CHECK=1 web/build.sh
cargo build "${flag[@]}" --target wasm32-unknown-unknown

rm -rf web/dist && mkdir -p web/dist
wasm-bindgen --target web --no-typescript --out-name ghore --out-dir web/dist \
  "target/wasm32-unknown-unknown/$dir/ghore.wasm"
cp web/index.html web/dist/
if command -v wasm-opt >/dev/null; then
  wasm-opt -Os -o web/dist/ghore_bg.wasm web/dist/ghore_bg.wasm
fi
echo "built web/dist ($(du -h web/dist/ghore_bg.wasm | cut -f1) wasm); serve it with: python -m http.server -d web/dist"
