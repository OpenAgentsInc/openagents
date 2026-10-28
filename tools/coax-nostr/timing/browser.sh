#!/usr/bin/env bash
# Explicit headless test tool; not a normal compiler/app dependency.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
output="${1:-$repo/coax/bin/nostr-timing-2026-10-28}"
mkdir -p "$output"; output="$(cd "$output" && pwd)"
compiler="${COAX_TEST_COMPILER:-$repo/coax/bin/coax}"
cc="${COAX_WASM_CC:-/opt/homebrew/opt/llvm/bin/clang}"
sysroot="${COAX_WASI_SYSROOT:-/opt/homebrew/opt/wasi-libc/share/wasi-sysroot}"
resources="${COAX_WASI_RESOURCES:-/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes}"
export PATH="/opt/homebrew/opt/lld/bin:$PATH"
"$compiler" c "$here/fixtures.coax" --library --release > "$output/timing.c"
cp "$here/browser.c" "$output/browser.c"
"$cc" --target=wasm32-wasip1 --sysroot="$sysroot" -resource-dir="$resources" \
  -std=c11 -O2 "$output/browser.c" -mexec-model=reactor \
  -Wl,--export-memory,--initial-memory=50331648,--max-memory=67108864 \
  -lm -o "$output/timing.wasm"
bun "$here/browser.ts" "$output"
