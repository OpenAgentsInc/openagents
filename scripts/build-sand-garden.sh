#!/usr/bin/env bash
# Build the Rust sand editor and its browser glue.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
: "${1:?usage: build-sand-garden.sh OUTDIR}"
cd "$root"
locked="$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' Cargo.lock)"
if [ "$(wasm-bindgen --version)" != "wasm-bindgen $locked" ]; then
  echo "Install wasm-bindgen-cli $locked before building." >&2
  exit 69
fi
cargo build --locked --release --target wasm32-unknown-unknown -p sand-garden
mkdir -p "$1"
wasm-bindgen --target web --no-typescript --out-dir "$1" \
  "${CARGO_TARGET_DIR:-$root/target}/wasm32-unknown-unknown/release/sand_garden.wasm"

for file in sand_garden.js sand_garden_bg.wasm; do gzip -9 -n -c "$1/$file" > "$1/$file.gz"; done
