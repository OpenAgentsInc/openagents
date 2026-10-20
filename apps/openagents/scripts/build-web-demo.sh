#!/usr/bin/env bash
# Build a GPUI app for the browser (apps/openagents/web): the desktop chat
# UI (`--app demo`, the default) or AgentGraph (`--app agent-graph`).
#
# Writes into OUTDIR, NAME being openagents_web_demo or
# openagents_web_agent_graph:
#
#   NAME.js          wasm-bindgen's `--target web` glue
#   NAME_bg.wasm     the module
#   start.js         the page's one script: imports the glue
#   *.gz, *.br       gzip and brotli copies, sent when asked
#
# openagents.com's server shows the demo at /demo/gpui when started with
# `openagents-web --gpui-demo OUTDIR`, and the graph at /agent/foreman with
# `--agent-graph OUTDIR`. With `--with-page`, also writes `index.html`, so
# any static server rooted at OUTDIR shows the app.
#
# Prerequisites: the pinned toolchain's wasm32-unknown-unknown target and the
# `wasm-bindgen` CLI at the version web/Cargo.lock pins (`cargo install
# wasm-bindgen-cli --version VERSION --locked`). `wasm-opt` (Binaryen) is
# used when present, and `brotli` when present.
#
# Usage: apps/openagents/scripts/build-web-demo.sh [--with-page] [--app APP] OUTDIR
set -euo pipefail

app="$(cd "$(dirname "$0")/.." && pwd)"
web="$app/web"
target="wasm32-unknown-unknown"
target_dir="${CARGO_TARGET_DIR:-$web/target}"

with_page=false
app_name=demo
out=""
while [ $# -gt 0 ]; do
  case "$1" in
    --with-page) with_page=true ;;
    --app) shift; app_name="${1:-}" ;;
    -*) echo "unknown option $1" >&2; exit 64 ;;
    *) out="$1" ;;
  esac
  shift
done
case "$app_name" in
  demo|agent-graph) ;;
  *) echo "unknown app $app_name (demo or agent-graph)" >&2; exit 64 ;;
esac
package="openagents-web-$app_name"
name="$(echo "$package" | tr - _)"
crate="$web/$app_name"
if [ -z "$out" ]; then
  echo "usage: $0 [--with-page] [--app demo|agent-graph] OUTDIR" >&2
  exit 64
fi

locked="$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' "$web/Cargo.lock")"
installed="$(wasm-bindgen --version 2>/dev/null | awk '{ print $2 }')"
if [ "$installed" != "$locked" ]; then
  echo "wasm-bindgen ${installed:-is not installed}; this build needs $locked: cargo install wasm-bindgen-cli --version $locked --locked" >&2
  exit 69
fi

cargo build --locked --manifest-path "$web/Cargo.toml" -p "$package" \
  --release --target "$target"

mkdir -p "$out"
wasm-bindgen --target web --no-typescript --out-dir "$out" \
  "$target_dir/$target/release/$name.wasm"
if command -v wasm-opt >/dev/null 2>&1; then
  wasm-opt -Os --enable-bulk-memory --enable-nontrapping-float-to-int \
    --enable-sign-ext --enable-mutable-globals --enable-reference-types \
    "$out/${name}_bg.wasm" -o "$out/${name}_bg.wasm"
fi
cp "$crate/start.js" "$out/start.js"
for file in "$name.js" "${name}_bg.wasm" start.js; do
  gzip -9 -n -c "$out/$file" > "$out/$file.gz"
  if command -v brotli >/dev/null 2>&1; then
    brotli -q 11 -f -o "$out/$file.br" "$out/$file"
  fi
done
if [ "$with_page" = true ]; then
  cp "$crate/index.html" "$out/index.html"
fi
ls -l "$out"
