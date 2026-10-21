#!/usr/bin/env bash
# Build and package the visual renderer beside a web app's assets.
set -euo pipefail
app="$(cd "$(dirname "$0")/.." && pwd)"
out="$1"
web="$app/web"
target=wasm32-unknown-unknown
target_dir="${CARGO_TARGET_DIR:-$web/target}"
. "$app/scripts/build-web-renderers.sh"
build_renderers openagents-web-visuals openagents_web_visuals "$out"
wait_renderers
cp "$app/web/visuals/start.js" "$out/start.js"
python3 "$app/scripts/package-web-build.py" "$out" openagents_web_visuals openagents_web_visuals_webgl
