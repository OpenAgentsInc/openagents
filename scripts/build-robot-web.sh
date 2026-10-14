#!/usr/bin/env bash
# Build the Rust Robot Lab player into an explicitly selected folder.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
out="${1:?Give an output folder for the Robot Lab player}"
: "${CARGO_TARGET_DIR:?Set CARGO_TARGET_DIR to the shared Cargo build folder}"
locked="$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' "$root/Cargo.lock")"
if ! command -v wasm-bindgen >/dev/null 2>&1; then
  echo "Install wasm-bindgen-cli $locked before building the player." >&2
  exit 69
fi
if [ "$(wasm-bindgen --version | awk '{ print $2 }')" != "$locked" ]; then
  echo "Use wasm-bindgen-cli $locked to match this checkout." >&2
  exit 69
fi
cd "$root"
cargo build --locked -p robot-web --target wasm32-unknown-unknown --profile guest
mkdir -p "$out"
wasm-bindgen --target web --no-typescript --out-dir "$out" \
  "$CARGO_TARGET_DIR/wasm32-unknown-unknown/guest/robot_web.wasm"
cp crates/robot-web/start.js "$out/start.js"
