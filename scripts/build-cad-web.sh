#!/usr/bin/env bash
# Build the CAD engine for the browser (crates/cad-wasm) the way
# openagents.com serves it at /cad (#11649).
#
# Writes into OUTDIR:
#
#   cad_wasm.js          the wasm-bindgen glue (an ES module)
#   cad_wasm_bg.wasm     the engine, size-optimized with wasm-opt
#   worker.js            runs the engine off the page's thread
#   start.js             the page's one script: the sliders, the drawing, the files
#   cad.css              the page's styles
#   *.br, *.gz           brotli and gzip copies, which the site sends when asked
#   sizes.txt            the module's size at each step
#
# The site serves them at /cad/ from the directory it was started with
# (`openagents-web --cad OUTDIR`, or /srv/cad in the image).
#
# Prerequisites: the wasm32-unknown-unknown target, the `wasm-bindgen` CLI at
# the version Cargo.lock pins (`cargo install wasm-bindgen-cli --version
# VERSION --locked`), `wasm-opt` (Binaryen) and `brotli`.
#
# Usage: ./scripts/build-cad-web.sh OUTDIR
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
target="wasm32-unknown-unknown"
target_dir="${CARGO_TARGET_DIR:-$root/target}"
out="${1:-}"
if [ -z "$out" ]; then
  echo "usage: $0 OUTDIR" >&2
  exit 64
fi

locked="$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' "$root/Cargo.lock")"
installed="$(wasm-bindgen --version 2>/dev/null | awk '{ print $2 }')"
if [ "$installed" != "$locked" ]; then
  echo "wasm-bindgen is '${installed:-missing}', Cargo.lock wants $locked; run: cargo install wasm-bindgen-cli --version $locked --locked" >&2
  exit 69
fi
for tool in wasm-opt brotli; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "$tool is not installed (Binaryen's wasm-opt, and brotli)" >&2
    exit 69
  fi
done

cd "$root"
# The cad-web profile (Cargo.toml): optimized for size, whole-program.
cargo build --locked --profile cad-web --target "$target" -p cad-wasm
built="$target_dir/$target/cad-web/cad_wasm.wasm"
mkdir -p "$out"
wasm-bindgen --target web --no-typescript --out-dir "$out" "$built"

# The features Rust's wasm32 target enables by default.
wasm-opt -Oz \
  --enable-bulk-memory --enable-mutable-globals --enable-nontrapping-float-to-int \
  --enable-sign-ext --enable-reference-types --enable-multivalue \
  "$out/cad_wasm_bg.wasm" -o "$out/cad_wasm_bg.wasm"

cp crates/cad-wasm/web/worker.js crates/cad-wasm/web/start.js crates/cad-wasm/web/cad.css "$out/"
for file in cad_wasm.js cad_wasm_bg.wasm worker.js start.js cad.css; do
  gzip -9 -n -c "$out/$file" > "$out/$file.gz"
  brotli -q 11 -f -o "$out/$file.br" "$out/$file"
done

size() { wc -c < "$1" | tr -d ' '; }
{
  echo "built $(size "$built")"
  echo "optimized $(size "$out/cad_wasm_bg.wasm")"
  echo "gzip $(size "$out/cad_wasm_bg.wasm.gz")"
  echo "brotli $(size "$out/cad_wasm_bg.wasm.br")"
} | tee "$out/sizes.txt"
ls -l "$out"
