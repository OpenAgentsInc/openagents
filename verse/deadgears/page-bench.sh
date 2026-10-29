#!/usr/bin/env bash
# Builds the Dead Gears page (apps/openagents-web/deadgears.coax) and page-bench.c to WebAssembly, as coax/web/wasm.sh
# builds a realm (-O2, no fast math), and runs it in Node's V8 with WASI:
# the page with 200 more mutants must write a frame within 6 ms (dg-creatures).
#   verse/deadgears/page-bench.sh
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
clang="${COAXIAL_CLANG:-/opt/homebrew/opt/llvm/bin/clang}"
sysroot="${COAXIAL_SYSROOT:-/opt/homebrew/opt/wasi-libc/share/wasi-sysroot}"
resources="${COAXIAL_RESOURCES:-/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes}"
export PATH="/opt/homebrew/opt/lld/bin:$PATH"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
"$repo/coax/bin/coax" c "$here/../../apps/openagents-web/deadgears.coax" --library --header "$work/deadgears.h" > "$work/Rules.c"
cflags=(--target=wasm32-wasip1 --sysroot="$sysroot" -resource-dir="$resources" -std=c11 -O2
        -ffp-contract=off -fno-fast-math -Wno-unused-function -Wno-unused-variable
        -Wno-unused-but-set-variable -I"$work")
"$clang" "${cflags[@]}" -c "$work/Rules.c" -o "$work/Rules.o"
"$clang" "${cflags[@]}" -c "$here/page-bench.c" -o "$work/bench.o"
"$clang" --target=wasm32-wasip1 --sysroot="$sysroot" -resource-dir="$resources" -O2 \
  -o "$work/bench.wasm" "$work/Rules.o" "$work/bench.o" -Wl,-z,stack-size=4194304
cat > "$work/run.mjs" <<'JS'
import { readFile } from "node:fs/promises";
import { WASI } from "node:wasi";
const wasi = new WASI({ version: "preview1", args: [], env: {}, returnOnExit: true });
const module = await WebAssembly.compile(await readFile(process.argv[2]));
const instance = await WebAssembly.instantiate(module, wasi.getImportObject());
process.exit(wasi.start(instance));
JS
node --no-warnings "$work/run.mjs" "$work/bench.wasm"
