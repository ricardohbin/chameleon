#!/usr/bin/env bash
# Builds the web version into web/dist (serve that folder with web/serve.py, which behaves like itch.io) and a
# ready-to-upload web/ghore-web.zip.
#
# One-time setup:
#   rustup target add wasm32-unknown-unknown   (msys2 users: the target ships with the toolchain package)
#   cargo install wasm-bindgen-cli --version <the wasm-bindgen version in Cargo.lock>
#   (optional) wasm-opt from binaryen, for a smaller .wasm
#   (optional) brotli, to ship the game compressed (about 6x smaller): the release builds use it if found
#
# Usage: web/build.sh [--dev | --small | --names]
#   (default) the `web-release` profile: opt-level 3, the fastest in the browser
#   --small   `web-small`: opt-level s, the smallest download, a bit slower to run
#   --names   `web-profile`: like the default with function names, for DevTools profiling (no wasm-opt, no brotli)
#   FEATURES=diag web/build.sh ...  also logs the frame-time diagnostics (see Cargo.toml)
set -euo pipefail
cd "$(dirname "$0")/.."

if [[ "${1:-}" == "--dev" ]]; then
  profile=dev; dir=debug; flag=()
elif [[ "${1:-}" == "--small" ]]; then
  profile=web-small; dir=web-small; flag=(--profile web-small)
elif [[ "${1:-}" == "--names" ]]; then
  profile=web-profile; dir=web-profile; flag=(--profile web-profile)
else
  profile=web-release; dir=web-release; flag=(--profile web-release)
fi

# The release build embeds assets/world.baked (see build.rs); skip its freshness check only if you must:
#   GHORE_SKIP_BAKE_CHECK=1 web/build.sh
cargo build "${flag[@]}" ${FEATURES:+--features "$FEATURES"} --target wasm32-unknown-unknown

rm -rf web/dist && mkdir -p web/dist
wasm-bindgen --target web --no-typescript $([[ "$profile" == web-profile ]] && echo --keep-debug) --out-name ghore --out-dir web/dist \
  "target/wasm32-unknown-unknown/$dir/ghore.wasm"
if [[ "$profile" != web-profile ]] && command -v wasm-opt >/dev/null; then
  wasm-opt $([[ "$profile" == web-small ]] && echo -Os || echo -O3) -o web/dist/ghore_bg.wasm web/dist/ghore_bg.wasm
fi
cp assets/fonts/GhoreSerif.ttf web/dist/

# The page counts the download in bytes of the decompressed game (the browser only reports the compressed size).
size=$(stat -c %s web/dist/ghore_bg.wasm)
file=ghore_bg.wasm
if [[ "$profile" == web-release || "$profile" == web-small ]]; then
  if command -v brotli >/dev/null; then
    # `.br`: itch.io (and web/serve.py) serve it with `Content-Encoding: br` and the type of the name without it.
    echo "compressing the game with brotli (takes a minute)..."
    brotli -q "${BROTLI_Q:-11}" -f -o web/dist/ghore_bg.wasm.br web/dist/ghore_bg.wasm
    rm web/dist/ghore_bg.wasm
    file=ghore_bg.wasm.br
  else
    echo "brotli not found: shipping the plain .wasm (the host may still gzip it)"
  fi
fi
sed -e "s|__WASM_FILE__|$file|" -e "s|__WASM_SIZE__|$size|" web/index.html > web/dist/index.html

# What to upload to itch.io: index.html at the root of the zip.
rm -f web/ghore-web.zip
(cd web/dist && python -m zipfile -c ../ghore-web.zip *)
echo "built web/dist ($(du -h web/dist/$file | cut -f1) $file, $((size / 1048576)) MB decompressed)"
echo "  test it:   python web/serve.py   (then http://localhost:8000)"
echo "  upload:    web/ghore-web.zip     (itch.io: kind of project = HTML, 'played in the browser')"
