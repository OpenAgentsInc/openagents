#!/usr/bin/env bash
# Build the desktop chat UI for the browser (apps/openagents/web).
#
# Writes into OUTDIR:
#
#   openagents_web_demo.js        wasm-bindgen's `--target web` glue
#   openagents_web_demo_bg.wasm   the module
#   start.js                      the page's one script: imports the glue
#   *.gz, *.br                    gzip and brotli copies, sent when asked
#
# openagents.com's server shows them at /demo/gpui when started with
# `openagents-web --gpui-demo OUTDIR`. With `--with-page`, also writes
# `index.html`, so any static server rooted at OUTDIR shows the demo.
#
# Prerequisites: the pinned toolchain's wasm32-unknown-unknown target and the
# `wasm-bindgen` CLI at the version web/Cargo.lock pins (`cargo install
# wasm-bindgen-cli --version VERSION --locked`). `wasm-opt` (Binaryen) is
# used when present, and `brotli` when present.
#
# Usage: apps/openagents/scripts/build-web-demo.sh [--with-page] OUTDIR
set -euo pipefail

app="$(cd "$(dirname "$0")/.." && pwd)"
web="$app/web"
target="wasm32-unknown-unknown"
target_dir="${CARGO_TARGET_DIR:-$web/target}"

with_page=false
out=""
for arg in "$@"; do
  case "$arg" in
    --with-page) with_page=true ;;
    -*) echo "unknown option $arg" >&2; exit 64 ;;
    *) out="$arg" ;;
  esac
done
if [ -z "$out" ]; then
  echo "usage: $0 [--with-page] OUTDIR" >&2
  exit 64
fi

locked="$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' "$web/Cargo.lock")"
installed="$(wasm-bindgen --version 2>/dev/null | awk '{ print $2 }')"
if [ "$installed" != "$locked" ]; then
  echo "wasm-bindgen ${installed:-is not installed}; this build needs $locked: cargo install wasm-bindgen-cli --version $locked --locked" >&2
  exit 69
fi

cargo build --locked --manifest-path "$web/Cargo.toml" -p openagents-web-demo \
  --release --target "$target"

mkdir -p "$out"
wasm-bindgen --target web --no-typescript --out-dir "$out" \
  "$target_dir/$target/release/openagents_web_demo.wasm"
if command -v wasm-opt >/dev/null 2>&1; then
  wasm-opt -Os --enable-bulk-memory --enable-nontrapping-float-to-int \
    --enable-sign-ext --enable-mutable-globals --enable-reference-types \
    "$out/openagents_web_demo_bg.wasm" -o "$out/openagents_web_demo_bg.wasm"
fi
cp "$web/demo/start.js" "$out/start.js"
for file in openagents_web_demo.js openagents_web_demo_bg.wasm start.js; do
  gzip -9 -n -c "$out/$file" > "$out/$file.gz"
  if command -v brotli >/dev/null 2>&1; then
    brotli -q 11 -f -o "$out/$file.br" "$out/$file"
  fi
done
if [ "$with_page" = true ]; then
  cp "$web/demo/index.html" "$out/index.html"
fi
ls -l "$out"
