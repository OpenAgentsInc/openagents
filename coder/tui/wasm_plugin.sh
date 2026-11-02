#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: compiles a Coax file to C, then C to a Wasm module with the host compiler)
# Builds a Coder Wasm plugin (Coder book chapter 15) from Coax. Each EXPORT is
# a `NAME :: () -> int #export_c` procedure; host functions are #foreign
# (coder_print, coder_now_ms) and become the module's env imports. The memory
# gets a 4 MB maximum, which admission requires.
#   coder/tui/wasm_plugin.sh PLUGIN.coax OUT.wasm EXPORT [EXPORT...]
set -euo pipefail
[[ $# -ge 3 ]] || { echo "usage: coder/tui/wasm_plugin.sh PLUGIN.coax OUT.wasm EXPORT [EXPORT...]" >&2; exit 2; }
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../.." && pwd)"
src="$1"; out="$2"; shift 2
llvm="${COAX_WASM_CLANG:-/opt/homebrew/opt/llvm/bin/clang}"
sysroot="${COAX_WASI_SYSROOT:-/opt/homebrew/opt/wasi-libc/share/wasi-sysroot}"
resources="${COAX_WASI_RESOURCES:-/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes}"
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
"$repo/coax/bin/coax" c "$src" --library > "$work/plugin.c"
exports=(); for e in "$@"; do exports+=("-Wl,--export=$e"); done
"$llvm" --target=wasm32-wasip1 --sysroot="$sysroot" -resource-dir="$resources" -std=c11 -O2 -mexec-model=reactor \
  -Wl,--allow-undefined -Wl,--max-memory=4194304 "${exports[@]}" "$work/plugin.c" -o "$out"
