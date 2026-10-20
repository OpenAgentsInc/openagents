#!/usr/bin/env bash
# Build /lander5's WebGL2 runner (crates/lander5-web) for the browser.
#
# Writes into OUTDIR:
#
#   lander5_web.js          the wasm-bindgen glue (an ES module)
#   lander5_web_bg.wasm     the module, size-optimized
#   worker/                 the same with the `worker` feature, which
#                           /lander6 inlines to hand the Grid to a worker
#
# The site reads both from the directory it was started with
# (`openagents-web --lander5 OUTDIR`, default /srv/lander5) and inlines them,
# with the shaders, into the one HTML response /lander5 sends.
#
# Prerequisites: the pinned toolchain's wasm32-unknown-unknown target and the
# `wasm-bindgen` CLI at the version Cargo.lock pins (`cargo install
# wasm-bindgen-cli --version VERSION --locked`). `wasm-opt` (Binaryen) is
# used when present. The crate has no C, so no WebAssembly clang is needed.
#
# Usage: ./scripts/build-lander5-web.sh OUTDIR
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
target="wasm32-unknown-unknown"
profile="wasm-tiny"
target_dir="${CARGO_TARGET_DIR:-$root/target}"

out="${1:-}"
if [ -z "$out" ] || [ "$#" -ne 1 ]; then
  echo "usage: $0 OUTDIR" >&2
  exit 64
fi

# The CLI must be the library's exact version, or the glue does not match.
locked="$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' "$root/Cargo.lock")"
if ! command -v wasm-bindgen >/dev/null 2>&1; then
  echo "wasm-bindgen is not installed; run: cargo install wasm-bindgen-cli --version $locked --locked" >&2
  exit 69
fi
installed="$(wasm-bindgen --version | awk '{ print $2 }')"
if [ "$installed" != "$locked" ]; then
  echo "wasm-bindgen $installed does not match Cargo.lock's $locked; run: cargo install wasm-bindgen-cli --version $locked --locked" >&2
  exit 69
fi

cd "$root"
build() { # OUTDIR [FEATURES]
  cargo build --locked --profile "$profile" --target "$target" -p lander5-web ${2:+--features "$2"}
  mkdir -p "$1"
  wasm-bindgen --target web --no-typescript --out-dir "$1" \
    "$target_dir/$target/$profile/lander5_web.wasm"
  if command -v wasm-opt >/dev/null 2>&1; then
    # The features Rust's wasm32 target enables by default.
    wasm-opt -Oz \
      --enable-bulk-memory --enable-mutable-globals --enable-nontrapping-float-to-int \
      --enable-sign-ext --enable-reference-types --enable-multivalue \
      "$1/lander5_web_bg.wasm" -o "$1/lander5_web_bg.wasm"
  else
    echo "wasm-opt not found; the module is not size-optimized" >&2
  fi
}
build "$out/worker" worker
build "$out"

ls -l "$out/lander5_web.js" "$out/lander5_web_bg.wasm" "$out/worker"
