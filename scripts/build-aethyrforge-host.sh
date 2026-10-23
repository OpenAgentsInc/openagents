#!/bin/sh
# Build only the public Rust protocol SDK. No private engine access is required.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
output=${1:?Usage: build-aethyrforge-host.sh ABSOLUTE_OUTPUT_DIRECTORY}
case "$output" in /*) ;; *) echo 'Use an absolute output directory.' >&2; exit 1;; esac
: "${CARGO_TARGET_DIR:=$HOME/work/openagents-target-1}"
export CARGO_TARGET_DIR
case "$CARGO_TARGET_DIR" in "$root"|"$root"/*) echo 'Use a shared target directory outside the checkout.' >&2; exit 1;; esac
locked=$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' "$root/Cargo.lock")
installed=$(wasm-bindgen --version | awk '{print $2}')
[ "$installed" = "$locked" ] || { echo "Install wasm-bindgen-cli $locked to match Cargo.lock." >&2; exit 1; }
cd "$root"
. "$root/scripts/dev/cargo-build-env.sh"
export CARGO_INCREMENTAL=0
cd "$root"
cargo build --locked --profile wasm-tiny --target wasm32-unknown-unknown -p aethyrforge-host
mkdir -p "$output"
wasm-bindgen --target web --no-typescript --out-dir "$output" "$CARGO_TARGET_DIR/wasm32-unknown-unknown/wasm-tiny/aethyrforge_host.wasm"
cp crates/aethyrforge-host/examples/host.html "$output/index.html"
cp crates/aethyrforge-host/examples/mock.html "$output/mock.html"
for variant in session version payload bytes object; do
  cp crates/aethyrforge-host/examples/adversarial.html "$output/forged-$variant.html"
done

# Optional public Kiln companion for creator releases. No private files are read.
if [ "${AETHYRFORGE_BUILD_KILN:-0}" = 1 ]; then
  CC_wasm32_unknown_unknown=${CC_wasm32_unknown_unknown:-/opt/homebrew/opt/llvm/bin/clang}
  AR_wasm32_unknown_unknown=${AR_wasm32_unknown_unknown:-/opt/homebrew/opt/llvm/bin/llvm-ar}
  export CC_wasm32_unknown_unknown AR_wasm32_unknown_unknown
  cargo build --locked --profile kiln-web --target wasm32-unknown-unknown -p kiln --lib
  wasm-bindgen --target web --no-typescript --out-dir "$output" "$CARGO_TARGET_DIR/wasm32-unknown-unknown/kiln-web/kiln.wasm"
fi
