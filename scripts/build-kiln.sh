#!/bin/sh
# Build the Kiln tech demo (docs/kiln/README.md): the WASI tools for
# wasm32-wasip1, packaged by content hash; the page module for
# wasm32-unknown-unknown with wasm-bindgen; and the page itself.
#
#   scripts/build-kiln.sh [--serve PORT]
#
# Output goes to $KILN_OUT (default $CARGO_TARGET_DIR/kiln-site). With
# --serve, kiln-dev then serves it on 127.0.0.1:PORT. Set KILN_DEV=0 to
# skip the native server build when another browser host serves these files.
#
# Needs: the wasm32-wasip1 and wasm32-unknown-unknown targets, wasm-bindgen
# matching the lock (0.2.128), and a clang with the WebAssembly target for
# secp256k1 (Homebrew's llvm, or CC_wasm32_unknown_unknown).
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)
cd "$root"
: "${CARGO_TARGET_DIR:=$HOME/work/openagents-target-1}"
export CARGO_TARGET_DIR
: "${CARGO_BUILD_JOBS:=6}"
export CARGO_BUILD_JOBS
. "$root/scripts/dev/cargo-build-env.sh"
# Not `kiln-web`: Cargo writes the `kiln-web` profile's host builds there.
out=${KILN_OUT:-$CARGO_TARGET_DIR/kiln-site}

if [ -z "${CC_wasm32_unknown_unknown:-}" ]; then
  for candidate in /opt/homebrew/opt/llvm/bin/clang /opt/homebrew/opt/llvm@*/bin/clang /usr/local/opt/llvm/bin/clang "$(command -v clang || true)"; do
    if [ -x "$candidate" ] && "$candidate" --print-targets 2>/dev/null | grep -q wasm32; then
      CC_wasm32_unknown_unknown=$candidate
      break
    fi
  done
  [ -n "${CC_wasm32_unknown_unknown:-}" ] || { echo "no clang with the WebAssembly target; install LLVM or set CC_wasm32_unknown_unknown" >&2; exit 1; }
  export CC_wasm32_unknown_unknown
  AR_wasm32_unknown_unknown="$(dirname "$CC_wasm32_unknown_unknown")/llvm-ar"
  export AR_wasm32_unknown_unknown
fi

cargo build -p kiln-tools --bins --profile guest --target wasm32-wasip1
cargo build -p kiln --lib --profile kiln-web --target wasm32-unknown-unknown
if [ "${KILN_DEV:-1}" != 0 ] || [ "${1:-}" = "--serve" ]; then
  cargo build -p kiln --bin kiln-dev
fi

rm -rf "$out"
mkdir -p "$out/tools" "$out/pkg"
wasm-bindgen --target web --no-typescript --out-dir "$out/pkg" \
  "$CARGO_TARGET_DIR/wasm32-unknown-unknown/kiln-web/kiln.wasm"
cp crates/kiln/www/index.html crates/kiln/www/review.html crates/kiln/www/worker.js "$out/"

# Each tool is served as tools/<sha256>.wasm; the manifest names them.
manifest="{\"tools\": {"
sep=""
for tool in cat grep ls patch rhai test view write; do
  file="$CARGO_TARGET_DIR/wasm32-wasip1/guest/$tool.wasm"
  hash=$(shasum -a 256 "$file" | awk '{print $1}')
  bytes=$(wc -c < "$file" | tr -d ' ')
  cp "$file" "$out/tools/$hash.wasm"
  manifest="$manifest$sep\"$tool\": {\"sha256\": \"$hash\", \"bytes\": $bytes}"
  sep=", "
done
printf '%s}}\n' "$manifest" > "$out/tools/manifest.json"

# The transcript: the desktop's chat components drawing the run, built in
# the GPUI web workspace with its own toolchain and target. KILN_TRANSCRIPT=0
# skips it; the page then shows the raw log.
if [ "${KILN_TRANSCRIPT:-1}" != 0 ]; then
  (
    cd "$root/apps/openagents/web"
    unset RUSTC CARGO_BUILD_BUILD_DIR
    CARGO_TARGET_DIR="${KILN_APP_TARGET:-$HOME/work/openagents-target-1-app}"
    export CARGO_TARGET_DIR
    . "$root/scripts/dev/cargo-build-env.sh"
    "$root/apps/openagents/scripts/build-web-kiln-transcript.sh" "$out/transcript" >/dev/null
  )
fi

echo "built $out"
du -sh "$out/pkg/kiln_bg.wasm" "$out/tools" "$out/transcript" 2>/dev/null | sed 's/^/  /'

if [ "${1:-}" = "--serve" ]; then
  exec "$CARGO_TARGET_DIR/debug/kiln-dev" --dir "$out" --port "${2:-8787}"
fi
if [ "${KILN_DEV:-1}" != 0 ]; then
  echo "serve it: $CARGO_TARGET_DIR/debug/kiln-dev --dir $out --port 8787"
fi
