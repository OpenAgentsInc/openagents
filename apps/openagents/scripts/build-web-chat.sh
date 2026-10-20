#!/usr/bin/env bash
# Build the signed-in GPUI web chat (apps/openagents/web/chat) for the
# browser.
#
# Writes into OUTDIR:
#
#   openagents_web_chat.js        wasm-bindgen's `--target web` glue
#   openagents_web_chat_bg.wasm   the module
#   start.js                      the page's one script: checks, loads, launches
#   fonts/*.ttf                   the Geist faces the chat draws, fetched apart
#   *.gz, *.br                    gzip and brotli copies, sent when asked
#
# openagents.com's server shows them at /gpui when started with
# `openagents-web --gpui-chat OUTDIR` (default /srv/gpui-chat in the image).
#
# Prerequisites: as build-web-demo.sh, the pinned toolchain's
# wasm32-unknown-unknown target and the `wasm-bindgen` CLI at the version
# web/Cargo.lock pins. `wasm-opt` and `brotli` are used when present.
#
# Usage: apps/openagents/scripts/build-web-chat.sh OUTDIR
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

cargo build --locked --manifest-path "$web/Cargo.toml" -p openagents-web-chat \
  --release --target "$target"

mkdir -p "$out/fonts"
wasm-bindgen --target web --no-typescript --out-dir "$out" \
  "$target_dir/$target/release/openagents_web_chat.wasm"
if command -v wasm-opt >/dev/null 2>&1; then
  wasm-opt -Os --enable-bulk-memory --enable-nontrapping-float-to-int \
    --enable-sign-ext --enable-mutable-globals --enable-reference-types \
    "$out/openagents_web_chat_bg.wasm" -o "$out/openagents_web_chat_bg.wasm"
fi
cp "$web/chat/start.js" "$out/start.js"
# The faces start.js asks for, by the names in its FONTS list.
fonts="$(sed -n 's/^  "\([A-Za-z-]*\.ttf\)",$/\1/p' "$web/chat/start.js")"
for font in $fonts; do
  cp "$app/crates/ui/assets/fonts/$font" "$out/fonts/$font"
done
compress() {
  gzip -9 -n -c "$1" > "$1.gz"
  if command -v brotli >/dev/null 2>&1; then
    brotli -q 11 -f -o "$1.br" "$1"
  fi
}
for file in openagents_web_chat.js openagents_web_chat_bg.wasm start.js; do
  compress "$out/$file"
done
for font in $fonts; do
  compress "$out/fonts/$font"
done
ls -l "$out" "$out/fonts"
