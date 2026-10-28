#!/usr/bin/env bash
# Coax → C11 → wasm32 for the browser: one Coax file plus its C face, and
# any Coax libraries the face calls, each its own C library and header
# (Haunt.h for haunt.coax), as the iOS app links metareal's.
#   coax/web/wasm.sh APP.coax FACE.c OUT.wasm [LIBRARY.coax...]
# Needs upstream LLVM (clang + wasm-ld) and wasi-libc; on this Mac:
#   brew install llvm lld wasi-libc wasi-runtimes
# Override with COAXIAL_CLANG, COAXIAL_SYSROOT, COAXIAL_RESOURCES.
set -euo pipefail
coax_file="$1"; face="$2"; out="$3"; shift 3
libraries=("$@")
repo="$(cd "$(dirname "$0")/../.." && pwd)"
clang="${COAXIAL_CLANG:-/opt/homebrew/opt/llvm/bin/clang}"
sysroot="${COAXIAL_SYSROOT:-/opt/homebrew/opt/wasi-libc/share/wasi-sysroot}"
resources="${COAXIAL_RESOURCES:-/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes}"
export PATH="/opt/homebrew/opt/lld/bin:$PATH"
name="$(basename "$coax_file" .coax)"
work="$(dirname "$out")/.c"
mkdir -p "$work"
"$repo/coax/bin/coax" c "$coax_file" --library --header "$work/$name.h" > "$work/$name.c"
# No fast math and no contraction: the browser computes what the checks computed.
# COAXIAL_MODE=dev (what coax serve uses) compiles at -O1 in parallel and
# links without the optimizer pass, for the fastest edit-to-screen loop;
# release (the default) builds at -O2.
mode="${COAXIAL_MODE:-release}"
opt=-O2; [[ "$mode" == dev ]] && opt=-O1
cflags=(--target=wasm32-wasip1 --sysroot="$sysroot" -resource-dir="$resources" -std=c11 "$opt"
        -ffp-contract=off -fno-fast-math -Wno-unused-function -Wno-unused-variable
        -Wno-unused-but-set-variable -I"$work")
"$clang" "${cflags[@]}" -c "$work/$name.c" -o "$work/$name.o" & coax_pid=$!
objects=("$work/$name.o" "$work/$name.face.o")
pids=()
for library in ${libraries[@]+"${libraries[@]}"}; do
  base="$(basename "$library" .coax)"; base="$(printf '%s' "${base:0:1}" | tr a-z A-Z)${base:1}"
  "$repo/coax/bin/coax" c "$library" --library --header "$work/$base.h" > "$work/$base.c"
  "$clang" "${cflags[@]}" -c "$work/$base.c" -o "$work/$base.o" & pids+=($!)
  objects+=("$work/$base.o")
done
"$clang" "${cflags[@]}" -c "$face" -o "$work/$name.face.o"
wait "$coax_pid"
for pid in ${pids[@]+"${pids[@]}"}; do wait "$pid"; done
link_opt=(); [[ "$mode" == dev ]] || link_opt=("$opt")
"$clang" --target=wasm32-wasip1 --sysroot="$sysroot" -resource-dir="$resources" ${link_opt[@]+"${link_opt[@]}"} \
  -mexec-model=reactor -o "$out.tmp" "${objects[@]}" \
  -Wl,--import-memory,--max-memory=67108864,-z,stack-size=1048576,--export=__heap_base,--gc-sections,--strip-debug
mv "$out.tmp" "$out"   # the server never serves a half-written module
