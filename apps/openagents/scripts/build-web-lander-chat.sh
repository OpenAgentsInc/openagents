#!/usr/bin/env bash
# Build the chat (apps/openagents/web/lander-chat) that openagents.com's
# /lander10 and /lander11 load over their backgrounds, in two builds:
#
#   OUTDIR/webgpu/   WebGPU only (web/lander-chat/webgpu): what a browser
#                    with WebGPU loads
#   OUTDIR/webgl/    the WebGL2 scene renderer: for a browser without it
#
# Each holds `openagents_web_lander_chat.js` (wasm-bindgen's `--target web`
# glue), `openagents_web_lander_chat_bg.wasm`, and brotli and gzip copies of
# both. The site serves them under /lander8/build/ (`openagents-web
# --lander8 OUTDIR`).
#
# The WebGPU-only build patches wgpu with a
# copy of the crates.io release without the GL, Metal, Vulkan, and DX12
# backends, whose WebGPU surface also offers a premultiplied canvas. The
# copy is written to web/lander-chat/webgpu/wgpu/.
#
# Prerequisites: the wasm32 target, wasm-bindgen at web/Cargo.lock's
# version, and brotli. wasm-opt is used when present.
#
# Usage: apps/openagents/scripts/build-web-lander-chat.sh OUTDIR
set -euo pipefail

app="$(cd "$(dirname "$0")/.." && pwd)"
web="$app/web"
card="$web/lander-chat"
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

# wgpu, as the web workspace locks it, with only its default features changed.
wgpu_version="$(awk '/^name = "wgpu"$/ { getline; gsub(/version = |"/, ""); print; exit }' "$web/Cargo.lock")"
cargo fetch --locked --manifest-path "$web/Cargo.toml" --target "$target" >/dev/null
registry="${CARGO_HOME:-$HOME/.cargo}/registry/src"
source_dir="$(find "$registry" -maxdepth 2 -type d -name "wgpu-$wgpu_version" | head -n 1)"
if [ -z "$source_dir" ]; then
  echo "wgpu $wgpu_version is not in $registry" >&2
  exit 69
fi
patched="$card/webgpu/wgpu"
rm -rf "$patched"
cp -R "$source_dir" "$patched"
awk '
  /^default = \[$/ { print "default = [\"std\", \"parking_lot\", \"wgsl\", \"webgpu\"]"; skip = 1; replaced = 1; next }
  skip && /^\]$/ { skip = 0; next }
  !skip { print }
  END { if (!replaced) exit 1 }
' "$source_dir/Cargo.toml" > "$patched/Cargo.toml" || {
  echo "wgpu's Cargo.toml has no default feature list to replace" >&2
  exit 70
}
# The WebGPU backend lists only an opaque canvas among the surface's alpha
# modes, though it configures a premultiplied one when asked; list both, so
# the card's transparent window shows the page's glass behind it.
backend="$patched/src/backend/webgpu.rs"
opaque_only='alpha_modes: vec![wgt::CompositeAlphaMode::Opaque],'
if ! grep -qF "$opaque_only" "$backend"; then
  echo "wgpu's WebGPU backend no longer lists its alpha modes as this script expects" >&2
  exit 70
fi
awk -v from="$opaque_only" '
  index($0, from) { sub(/vec!\[wgt::CompositeAlphaMode::Opaque\]/, "vec![wgt::CompositeAlphaMode::Opaque, wgt::CompositeAlphaMode::PreMultiplied]") }
  { print }
' "$source_dir/src/backend/webgpu.rs" > "$backend"

build() { # MANIFEST PACKAGE NAME DEST
  # Each renderer is a separate Cargo workspace. Keep its compiler state
  # keyed to that manifest when both builds share the app target directory.
  oa_build_manifest="$1"
  . "$app/../../scripts/dev/cargo-build-env.sh"
  export CARGO_INCREMENTAL=0
  cargo build --locked --manifest-path "$1" -p "$2" --release --target "$target" ${renderer_features[@]+"${renderer_features[@]}"}
  mkdir -p "$4"
  wasm-bindgen --target web --no-typescript --out-name openagents_web_lander_chat --out-dir "$4" \
    "$target_dir/$target/release/$3.wasm"
  if command -v wasm-opt >/dev/null 2>&1; then
    wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int \
      --enable-sign-ext --enable-mutable-globals --enable-reference-types \
      "$4/openagents_web_lander_chat_bg.wasm" -o "$4/openagents_web_lander_chat_bg.wasm"
  fi
  for file in openagents_web_lander_chat.js openagents_web_lander_chat_bg.wasm; do
    gzip -9 -n -c "$4/$file" > "$4/$file.gz"
    brotli -q 11 -f -o "$4/$file.br" "$4/$file"
  done
}
build "$card/webgpu/Cargo.toml" openagents-web-lander-chat-webgpu openagents_web_lander_chat_webgpu "$out/webgpu"
renderer_features=(--no-default-features --features webgl,export)
build "$web/Cargo.toml" openagents-web-lander-chat openagents_web_lander_chat "$out/webgl"
ls -l "$out/webgpu" "$out/webgl"
