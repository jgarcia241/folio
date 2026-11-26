#!/usr/bin/env bash
# Builds the offline web app (PWA) for iPhone, iPad and any browser into web/dist/.
# Usage:  ./web/build.sh
set -euo pipefail
cd "$(dirname "$0")/.."

WASM_BINDGEN_VERSION=0.2.130   # must match the wasm-bindgen version in Cargo.toml

echo "==> Checking tools"
rustup target add wasm32-unknown-unknown >/dev/null
if ! command -v wasm-bindgen >/dev/null || [ "$(wasm-bindgen --version | awk '{print $2}')" != "$WASM_BINDGEN_VERSION" ]; then
  echo "    installing wasm-bindgen-cli $WASM_BINDGEN_VERSION (one time)"
  cargo install wasm-bindgen-cli --version "$WASM_BINDGEN_VERSION" --locked
fi

echo "==> Compiling Rust to WebAssembly"
cargo build --lib --target wasm32-unknown-unknown --profile web

OUT=web/dist
rm -rf "$OUT"
mkdir -p "$OUT/icons"

echo "==> Generating JavaScript bindings"
wasm-bindgen --target web --no-typescript --out-dir "$OUT" --out-name folio \
  target/wasm32-unknown-unknown/web/folio.wasm

# Optional: shrink the download further if Binaryen's wasm-opt is installed.
if command -v wasm-opt >/dev/null; then
  echo "==> Optimizing with wasm-opt"
  wasm-opt -Os --enable-bulk-memory --enable-nontrapping-float-to-int -o "$OUT/folio_bg.wasm" "$OUT/folio_bg.wasm"
fi

echo "==> Copying page, manifest, icons and offline worker"
cp web/index.html web/manifest.webmanifest web/config.js web/sync.js "$OUT/"
cp web/icons/*.png "$OUT/icons/"
VERSION=$(date +%Y%m%d%H%M%S)
sed "s/__VERSION__/$VERSION/" web/sw.js > "$OUT/sw.js"

echo
echo "Done: $OUT ($(du -sh "$OUT" | cut -f1))"
echo "Try it on this computer:   python3 -m http.server 8080 --directory $OUT   then open http://localhost:8080"
echo "Put it online for iPhone/iPad: upload the contents of $OUT to any HTTPS static host (e.g. GitHub Pages)."
