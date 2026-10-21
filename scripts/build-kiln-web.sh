#!/usr/bin/env bash
# Build Kiln (crates/kiln, docs/kiln/) for openagents.com's /admin/kiln/
# (#12440): the static directory scripts/build-kiln.sh writes, without the
# local server, and with brotli and gzip copies beside each script and
# module.
#
# Writes into OUTDIR:
#
#   index.html  review.html  worker.js
#   pkg/kiln.js  pkg/kiln_bg.wasm
#   tools/manifest.json  tools/<sha256>.wasm ...
#   *.br, *.gz             compressed copies of each .js and .wasm
#
# The site serves it from `openagents-web --kiln DIR` (default /srv/kiln)
# to site admins only, and writes config.json itself from its environment.
#
# Prerequisites: the wasm32-wasip1 and wasm32-unknown-unknown targets, the
# `wasm-bindgen` CLI at the version Cargo.lock pins, a clang with the
# WebAssembly target (secp256k1's C), `brotli`, and `gzip`.
#
# Usage: ./scripts/build-kiln-web.sh OUTDIR
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
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

if [ -z "${CC_wasm32_unknown_unknown:-}" ]; then
  for candidate in "$(command -v clang || true)" /opt/homebrew/opt/llvm/bin/clang /usr/local/opt/llvm/bin/clang; do
    if [ -x "$candidate" ] && "$candidate" --print-targets 2>/dev/null | grep -q wasm32; then
      CC_wasm32_unknown_unknown=$candidate
      break
    fi
  done
  [ -n "${CC_wasm32_unknown_unknown:-}" ] || { echo "no clang with the WebAssembly target; install LLVM or set CC_wasm32_unknown_unknown" >&2; exit 69; }
  export CC_wasm32_unknown_unknown
fi
if [ -z "${AR_wasm32_unknown_unknown:-}" ] && [ -x "$(dirname "$CC_wasm32_unknown_unknown")/llvm-ar" ]; then
  export AR_wasm32_unknown_unknown="$(dirname "$CC_wasm32_unknown_unknown")/llvm-ar"
fi

cd "$root"
cargo build --locked -p kiln-tools --bins --profile guest --target wasm32-wasip1
cargo build --locked -p kiln --lib --profile kiln-web --target wasm32-unknown-unknown

rm -rf "$out"
mkdir -p "$out/tools" "$out/pkg"
wasm-bindgen --target web --no-typescript --out-dir "$out/pkg" \
  "$target_dir/wasm32-unknown-unknown/kiln-web/kiln.wasm"
cp crates/kiln/www/index.html crates/kiln/www/review.html crates/kiln/www/worker.js "$out/"

digest() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1"; else shasum -a 256 "$1"; fi | awk '{print $1}'
}

# Each tool is served as tools/<sha256>.wasm; the manifest names them.
manifest='{"tools": {'
sep=""
for tool in cat grep ls patch rhai test view write; do
  file="$target_dir/wasm32-wasip1/guest/$tool.wasm"
  hash=$(digest "$file")
  bytes=$(wc -c < "$file" | tr -d ' ')
  cp "$file" "$out/tools/$hash.wasm"
  manifest="$manifest$sep\"$tool\": {\"sha256\": \"$hash\", \"bytes\": $bytes}"
  sep=", "
done
printf '%s}}\n' "$manifest" > "$out/tools/manifest.json"

find "$out" -type f \( -name '*.js' -o -name '*.wasm' \) | while read -r file; do
  gzip -9 -k -f "$file"
  brotli -q 11 -k -f "$file"
done
test -s "$out/pkg/kiln_bg.wasm.br"
echo "built $out"
