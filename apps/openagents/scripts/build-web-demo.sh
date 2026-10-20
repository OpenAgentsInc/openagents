#!/usr/bin/env bash
# Build a GPUI app for the browser (apps/openagents/web): the desktop chat
# UI (`--app demo`, the default) or AgentGraph (`--app agent-graph`).
#
# Writes into OUTDIR, NAME being openagents_web_demo or
# openagents_web_agent_graph:
#
#   NAME.js          wasm-bindgen's `--target web` glue
#   NAME_bg.wasm     the module
#   start.js         the page's one script: imports the glue, fetches the
#                    font files the page names (`data-gpui-font` preloads;
#                    the site serves them from build-web-fonts.sh's output)
#   *.gz, *.br       gzip and brotli copies, sent when asked
#
# For AgentGraph it also writes `first-frame/`: the first-frame painter
# (`painter.js`, wasm-bindgen's `--target no-modules` glue, `painter.wasm`,
# and `boot.js`), Foreman's first frame for each size class
# (`foreman-*.ffs.gz`, captured natively by `first-frame-capture`),
# `manifest.json`, and the capture tool itself, which the site runs for
# other box sizes. The site carries these inline in /agent/foreman, so the
# graph shows before the module has downloaded.
#
# openagents.com's server shows the demo at /demo/gpui when started with
# `openagents-web --gpui-demo OUTDIR`, and the graph at /agent/foreman with
# `--agent-graph OUTDIR`. With `--with-page`, also writes `index.html`, so
# any static server rooted at OUTDIR shows the app.
#
# Prerequisites: the pinned toolchain's wasm32-unknown-unknown target and the
# `wasm-bindgen` CLI at the version web/Cargo.lock pins (`cargo install
# wasm-bindgen-cli --version VERSION --locked`). `wasm-opt` (Binaryen), `brotli`, and Python 3 are required.
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

for tool in wasm-opt brotli python3; do
  command -v "$tool" >/dev/null 2>&1 || { echo "$tool is not installed" >&2; exit 69; }
done

cargo build --locked --manifest-path "$web/Cargo.toml" -p "$package" \
  --release --target "$target"

mkdir -p "$out"
wasm-bindgen --target web --no-typescript --out-dir "$out" \
  "$target_dir/$target/release/$name.wasm"
wasm-opt -Os --enable-bulk-memory --enable-nontrapping-float-to-int \
    --enable-sign-ext --enable-mutable-globals --enable-reference-types \
    "$out/${name}_bg.wasm" -o "$out/${name}_bg.wasm"
cp "$crate/start.js" "$out/start.js"
if [ "$app_name" = demo ]; then
  cp "$crate/assets/fonts/"*.otf "$crate/assets/fonts/"*.ttf "$out/"
fi
python3 "$app/scripts/package-web-build.py" "$out" "$name"
if [ "$app_name" = agent-graph ]; then
  first="$out/first-frame"
  mkdir -p "$first"
  "$app/scripts/build-first-frame-painter.sh" "$first"
  # Native, no GPU: GPUI's headless platform lays out and paints the view.
  # The tool stays beside the frames: the site runs it for box sizes no
  # class covers (`/agent/foreman/first-frame`).
  cargo build --locked --manifest-path "$web/Cargo.toml" -p openagents-first-frame-capture \
    --bin first-frame-capture --profile capture
  cp "$target_dir/capture/first-frame-capture" "$first/first-frame-capture"
  "$first/first-frame-capture" "$first"
fi
if [ "$with_page" = true ]; then
  cp "$crate/index.html" "$out/index.html"
  # The page names the faces it needs; a static server finds them here.
  mkdir -p "$out/fonts"
  for font in $(sed -n 's/.*data-gpui-font="\([A-Za-z-]*\.ttf\)".*/\1/p' "$crate/index.html"); do
    cp "$app/crates/ui/assets/fonts/$font" "$out/fonts/$font"
  done
fi
ls -l "$out"
