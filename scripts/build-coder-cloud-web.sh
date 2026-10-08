#!/usr/bin/env bash
# Build the Rust workspace privacy runtime and generated loader glue.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
if [ "$#" -ne 1 ]; then
  echo "usage: $0 OUTPUT_DIRECTORY" >&2
  exit 64
fi
out="$1"
target_dir="${CARGO_TARGET_DIR:-$HOME/work/openagents-target-agent3}"
export CARGO_TARGET_DIR="$target_dir"
cd "$root"

locked="$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' Cargo.lock)"
installed="$(wasm-bindgen --version | awk '{ print $2 }')"
if [ "$installed" != "$locked" ]; then
  echo "wasm-bindgen $installed does not match Cargo.lock's $locked" >&2
  exit 69
fi
openagents lease build --keep-target-dir -- cargo build --locked --target wasm32-unknown-unknown -p coder-cloud-web
mkdir -p "$out"
wasm-bindgen --target web --no-typescript --out-dir "$out" \
  "$target_dir/wasm32-unknown-unknown/debug/coder_cloud_web.wasm"
