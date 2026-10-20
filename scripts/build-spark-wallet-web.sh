#!/usr/bin/env bash
# Build the opt-in browser wallet; generated JavaScript only loads Rust Wasm.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd -P)"
out="${1:?usage: scripts/build-spark-wallet-web.sh OUTDIR}"
[[ "$out" = /* ]] || out="$PWD/$out"
cd "$root"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/work/openagents-target-1}"
. scripts/dev/cargo-build-env.sh
export CARGO_INCREMENTAL=0
locked="$(awk '/^name = "wasm-bindgen"$/ {getline; gsub(/version = |"/, ""); print; exit}' Cargo.lock)"
[[ "$(wasm-bindgen --version)" = "wasm-bindgen $locked" ]]
if [[ -z "${CC_wasm32_unknown_unknown:-}" ]]; then
  for compiler in /opt/homebrew/opt/llvm/bin/clang /usr/bin/clang; do
    if [[ -x "$compiler" ]] && "$compiler" --print-targets | rg -q wasm32; then export CC_wasm32_unknown_unknown="$compiler"; break; fi
  done
fi
[[ -n "${CC_wasm32_unknown_unknown:-}" ]]
export AR_wasm32_unknown_unknown="${AR_wasm32_unknown_unknown:-$(dirname "$CC_wasm32_unknown_unknown")/llvm-ar}"
profile="${OPENAGENTS_WALLET_WEB_PROFILE:-release}"
if [[ "$profile" = debug ]]; then cargo build --locked --target wasm32-unknown-unknown -p spark-wallet-web
else cargo build --locked --release --target wasm32-unknown-unknown -p spark-wallet-web; fi
mkdir -p "$out"
wasm-bindgen --target web --no-typescript --out-dir "$out" "$CARGO_TARGET_DIR/wasm32-unknown-unknown/$profile/spark_wallet_web.wasm"
printf '%s\n' 'import init from "./spark_wallet_web.js"; await init();' > "$out/start.js"
cp crates/spark-wallet-web/wallet.css "$out/wallet.css"
cp crates/spark-wallet-web/index.html "$out/index.html"
