#!/usr/bin/env bash
# Build Kiln's transcript (apps/openagents/web/kiln-transcript): the desktop's
# transcript drawing a Kiln agent run in the Kiln page's `#gpui-mount` box.
# One build, with the WebGL2 renderer, which every browser Kiln supports
# has.
#
# Writes into OUTDIR `openagents_web_kiln_transcript.js` (wasm-bindgen's
# `--target web` glue), `openagents_web_kiln_transcript_bg.wasm`, and brotli
# and gzip copies of both. Kiln's builds (`scripts/build-kiln.sh`,
# `scripts/build-kiln-web.sh`) put it under the page's `transcript/`.
#
# Prerequisites: the wasm32 target, wasm-bindgen at web/Cargo.lock's
# version, and brotli. wasm-opt is used when present.
#
# Usage: apps/openagents/scripts/build-web-kiln-transcript.sh OUTDIR
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

locked="$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' "$web/Cargo.lock")"
installed="$(wasm-bindgen --version 2>/dev/null | awk '{ print $2 }')"
if [ "$installed" != "$locked" ]; then
  echo "wasm-bindgen ${installed:-is not installed}; this build needs $locked: cargo install wasm-bindgen-cli --version $locked --locked" >&2
  exit 69
fi
command -v brotli >/dev/null 2>&1 || { echo "brotli is not installed" >&2; exit 69; }

cargo build --locked --manifest-path "$web/Cargo.toml" -p openagents-web-kiln-transcript \
  --release --target "$target"
mkdir -p "$out"
wasm-bindgen --target web --no-typescript --out-dir "$out" \
  "$target_dir/$target/release/openagents_web_kiln_transcript.wasm"
if command -v wasm-opt >/dev/null 2>&1; then
  wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int \
    --enable-sign-ext --enable-mutable-globals --enable-reference-types \
    "$out/openagents_web_kiln_transcript_bg.wasm" -o "$out/openagents_web_kiln_transcript_bg.wasm"
fi
for file in openagents_web_kiln_transcript.js openagents_web_kiln_transcript_bg.wasm; do
  gzip -9 -n -c "$out/$file" > "$out/$file.gz"
  brotli -q 11 -f -o "$out/$file.br" "$out/$file"
done
ls -l "$out"
