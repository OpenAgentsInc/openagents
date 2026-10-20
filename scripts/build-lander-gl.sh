#!/usr/bin/env bash
# Build the /lander4 painter (crates/lander-gl) and check it in.
#
# 1. Builds crates/lander-gl for wasm32-unknown-unknown with the workspace's
#    `guest` profile: opt-level z, LTO, one codegen unit, panic=abort,
#    stripped.
# 2. Runs wasm-bindgen 0.2.128 (`--target no-modules`, so the page can start
#    it from a classic inline script with `initSync`).
# 3. Shrinks the module with `wasm-opt -Oz`.
# 4. Drops the glue's comments and indentation.
# 5. Writes crates/openagents-web/static/lander4/lander_gl_bg.wasm and
#    lander_gl.js, which openagents-web compiles in (pages/lander4.rs) and
#    inlines into the page. The website image builds nothing for it.
#
# Paths are remapped so the bytes don't depend on where the checkout or the
# Cargo home is.
#
# Prerequisites: the wasm32-unknown-unknown target, wasm-bindgen-cli
# 0.2.128, and binaryen's wasm-opt.
#
# Usage: CARGO_TARGET_DIR=... ./scripts/build-lander-gl.sh
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
target="wasm32-unknown-unknown"
target_dir="${CARGO_TARGET_DIR:-$root/target}"
cargo_home="${CARGO_HOME:-$HOME/.cargo}"
out="$root/crates/openagents-web/static/lander4"

want="wasm-bindgen 0.2.128"
have="$(wasm-bindgen --version 2>/dev/null || true)"
if [ "$have" != "$want" ]; then
  echo "need $want (cargo install wasm-bindgen-cli --version 0.2.128); found: ${have:-none}" >&2
  exit 69
fi
command -v wasm-opt > /dev/null || { echo "need wasm-opt (binaryen)" >&2; exit 69; }

RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$root=. --remap-path-prefix=$cargo_home=/cargo" \
  cargo build --locked -p lander-gl --lib --target "$target" --profile guest

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
wasm-bindgen --target no-modules --no-typescript --out-dir "$work" \
  "$target_dir/$target/guest/lander_gl.wasm"
mkdir -p "$out"
wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int \
  --enable-sign-ext --enable-mutable-globals --enable-reference-types \
  --enable-multivalue --strip-debug --strip-producers \
  -o "$out/lander_gl_bg.wasm" "$work/lander_gl_bg.wasm"
# The glue goes into every page, so drop its comments and indentation (it
# has no multi-line strings).
awk '!/^[[:space:]]*(\/\*\*|\*|\/\/)/ { sub(/^[[:space:]]+/, ""); if (length) print }' \
  "$work/lander_gl.js" > "$out/lander_gl.js"
wc -c "$out/lander_gl_bg.wasm" "$out/lander_gl.js"
