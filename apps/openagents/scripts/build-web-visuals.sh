#!/usr/bin/env bash
# Build and package the visual renderer beside a web app's assets.
set -euo pipefail
app="$(cd "$(dirname "$0")/.." && pwd)"
out="$1"
target_dir="${CARGO_TARGET_DIR:-$app/web/target}"
cargo build --locked --manifest-path "$app/web/Cargo.toml" -p openagents-web-visuals --release --target wasm32-unknown-unknown
mkdir -p "$out"
wasm-bindgen --target web --no-typescript --out-dir "$out" "$target_dir/wasm32-unknown-unknown/release/openagents_web_visuals.wasm"
wasm-opt -Os --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext --enable-mutable-globals --enable-reference-types "$out/openagents_web_visuals_bg.wasm" -o "$out/openagents_web_visuals_bg.wasm"
cp "$app/web/visuals/start.js" "$out/start.js"
python3 "$app/scripts/package-web-build.py" "$out" openagents_web_visuals
