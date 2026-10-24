#!/usr/bin/env bash
# Builds the render lab's Rust renderer for the browser into apps/render-lab/wasm/.
set -euo pipefail
# The web package is apps/aethyr; its Rust workspace is crates/aethyr.
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
monorepo="$(cd "$repo/../.." && pwd -P)"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$(dirname "$monorepo")/aethyr-target}"
(cd "$monorepo/crates/aethyr" && cargo build --locked -p af-render-lab --target wasm32-unknown-unknown --release)
cd "$repo"
mkdir -p apps/render-lab/wasm
out=apps/render-lab/wasm/render-lab.wasm
cp "$CARGO_TARGET_DIR/wasm32-unknown-unknown/release/af_render_lab.wasm" "$out"
optimizer="${AETHYRFORGE_WASM_OPT:-wasm-opt}"
if command -v "$optimizer" >/dev/null 2>&1; then
  "$optimizer" -O3 --strip-debug --strip-producers "$out" -o "$out"
else
  echo "wasm-opt not found; leaving the module unoptimized." >&2
fi
python3 -c "import os;print(f'Built $out: {os.path.getsize(\"$out\"):,} bytes')"
