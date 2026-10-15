#!/usr/bin/env bash
# Build the CAD engine for the browser (crates/cad-wasm, wasm32-unknown-unknown)
# and run its tools under Node: a part is rebuilt and checked, its faces are
# named, a DXF drawing comes in, and it exports glTF, STEP, 3MF and STL, all
# as WebAssembly. Prints the sizes and the answers it checked.
#
# Prerequisites: `rustup target add wasm32-unknown-unknown`, Node 20 or newer,
# and the wasm-bindgen CLI at the version Cargo.lock resolves
# (`cargo install wasm-bindgen-cli --version <that version>`).
#
# Usage: ./scripts/cad-wasm-test.sh [OUT_DIR]
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
out="${1:-${TMPDIR:-/tmp}/cad-wasm}"
target="${CARGO_TARGET_DIR:-$root/target}"

want="$(awk '/^name = "wasm-bindgen"$/{getline; gsub(/version = |"/, ""); print; exit}' Cargo.lock)"
have="$(wasm-bindgen --version 2>/dev/null | awk '{print $2}')"
if [ "$want" != "$have" ]; then
  echo "cad-wasm-test: wasm-bindgen CLI is '${have:-missing}', Cargo.lock wants $want:" >&2
  echo "  cargo install wasm-bindgen-cli --version $want" >&2
  exit 1
fi

cargo build --release --target wasm32-unknown-unknown -p cad-wasm
rm -rf "$out"
wasm-bindgen --target nodejs --out-dir "$out" \
  "$target/wasm32-unknown-unknown/release/cad_wasm.wasm"
node "$root/scripts/cad-wasm-test.mjs" "$out/cad_wasm.js"
