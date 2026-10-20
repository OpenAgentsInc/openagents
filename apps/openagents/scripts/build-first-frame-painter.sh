#!/usr/bin/env bash
# The first-frame painter (apps/openagents/web/first-frame-paint): a
# paint-only WebGL2 module that draws a captured or recorded GPUI frame
# before an app's module has loaded. Pages carry it inline.
#
# Writes into OUTDIR: painter.js (wasm-bindgen's `--target no-modules`
# glue), painter.wasm and painter.wasm.gz, and boot.js (the page's inline
# script after the glue). build-web-demo.sh (AgentGraph) and
# build-web-chat.sh run it.
#
# Usage: apps/openagents/scripts/build-first-frame-painter.sh OUTDIR
set -euo pipefail

app="$(cd "$(dirname "$0")/.." && pwd)"
web="$app/web"
target="wasm32-unknown-unknown"
target_dir="${CARGO_TARGET_DIR:-$web/target}"
out="${1:-}"
if [ -z "$out" ] || [ "$#" -ne 1 ]; then
  echo "usage: $0 OUTDIR" >&2
  exit 64
fi

mkdir -p "$out"
cargo build --locked --manifest-path "$web/Cargo.toml" -p openagents-first-frame-paint \
  --release --target "$target"
wasm-bindgen --target no-modules --no-typescript --out-dir "$out" \
  "$target_dir/$target/release/openagents_first_frame_paint.wasm"
mv "$out/openagents_first_frame_paint.js" "$out/painter.js"
mv "$out/openagents_first_frame_paint_bg.wasm" "$out/painter.wasm"
if command -v wasm-opt >/dev/null 2>&1; then
  wasm-opt -Oz --strip-debug --strip-producers --enable-bulk-memory \
    --enable-nontrapping-float-to-int --enable-sign-ext --enable-mutable-globals \
    --enable-reference-types "$out/painter.wasm" -o "$out/painter.wasm"
fi
gzip -9 -n -c "$out/painter.wasm" > "$out/painter.wasm.gz"
cp "$web/first-frame-paint/boot.js" "$out/boot.js"
