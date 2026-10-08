#!/usr/bin/env bash
# Build the local Rust input adapter and its generated browser glue.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
if [ "$#" -ne 1 ]; then
  echo "usage: $0 OUTPUT_DIRECTORY" >&2
  exit 64
fi
out="$1"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/work/openagents-target-agent0}"
cd "$root"
locked="$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' Cargo.lock)"
installed="$(wasm-bindgen --version | awk '{ print $2 }')"
if [ "$installed" != "$locked" ]; then
  echo "wasm-bindgen $installed does not match Cargo.lock's $locked" >&2
  exit 69
fi
openagents lease build --keep-target-dir -- cargo build --locked --release --target wasm32-unknown-unknown -p coder-chat-web
mkdir -p "$out"
wasm-bindgen --target web --no-typescript --out-dir "$out" \
  "$CARGO_TARGET_DIR/wasm32-unknown-unknown/release/coder_chat_web.wasm"
