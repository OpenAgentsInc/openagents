#!/usr/bin/env bash
# Build /pylons' Grid layer (crates/pylons-web) for the browser.
#
# Writes into OUTDIR:
#
#   pylons_web.js           the wasm-bindgen glue (an ES module)
#   pylons_web_bg.wasm      the module, size-optimized
#   *.br, *.gz              brotli and gzip copies of both
#
# The site serves them from `openagents-web --pylons DIR` as DIR/layer/
# (default /srv/pylons/layer), so pass DIR/layer here. The GPUI card goes
# beside it in DIR/card (apps/openagents/scripts/build-web-pylon-card.sh).
#
# Prerequisites: the pinned toolchain's wasm32-unknown-unknown target, the
# `wasm-bindgen` CLI at the version Cargo.lock pins, and `brotli`.
# `wasm-opt` (Binaryen) is used when present.
#
# Usage: ./scripts/build-pylons-web.sh OUTDIR
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

locked="$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' "$root/Cargo.lock")"
installed="$(wasm-bindgen --version 2>/dev/null | awk '{ print $2 }')"
if [ "$installed" != "$locked" ]; then
  echo "wasm-bindgen ${installed:-is not installed}; this build needs $locked: cargo install wasm-bindgen-cli --version $locked --locked" >&2
  exit 69
fi
command -v brotli >/dev/null 2>&1 || { echo "brotli is not installed" >&2; exit 69; }

cd "$root"
cargo build --locked --profile "$profile" --target "$target" -p pylons-web
mkdir -p "$out"
wasm-bindgen --target web --no-typescript --out-dir "$out" \
  "$target_dir/$target/$profile/pylons_web.wasm"
if command -v wasm-opt >/dev/null 2>&1; then
  wasm-opt -Oz \
    --enable-bulk-memory --enable-mutable-globals --enable-nontrapping-float-to-int \
    --enable-sign-ext --enable-reference-types --enable-multivalue \
    "$out/pylons_web_bg.wasm" -o "$out/pylons_web_bg.wasm"
else
  echo "wasm-opt not found; the module is not size-optimized" >&2
fi
for file in pylons_web.js pylons_web_bg.wasm; do
  gzip -9 -n -c "$out/$file" > "$out/$file.gz"
  brotli -q 11 -f -o "$out/$file.br" "$out/$file"
done
ls -l "$out"
