#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/../../.." && pwd)"
export CARGO_TARGET_DIR="${AETHYR_TARGET_DIR:-$(dirname "$root")/aethyr-target}"
cd "$root/crates/aethyr"
source "$root/scripts/dev/cargo-build-env.sh"
cargo build --locked -p af-home --target wasm32-unknown-unknown --release
python3 "$root/apps/aethyr/tools/build-grid-home.py" --wasm "$CARGO_TARGET_DIR/wasm32-unknown-unknown/release/af_home.wasm"
