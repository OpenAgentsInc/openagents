#!/usr/bin/env bash
# Coax → C11 → wasm32 for the browser: one Coax file plus its C face, and
# any Coax libraries the face calls, each its own C library and header
# (Haunt.h for haunt.coax), as the iOS app links the engine's.
#   coax/web/wasm.sh APP.coax FACE.c OUT.wasm [LIBRARY.coax...]
# Needs upstream LLVM (clang + wasm-ld) and wasi-libc: toolchain.sh finds
# them (Homebrew on a Mac, nixpkgs on Linux). Override with COAXIAL_CLANG,
# COAXIAL_SYSROOT, COAXIAL_RESOURCES.
set -euo pipefail
coax_file="$1"; face="$2"; out="$3"; shift 3
libraries=("$@")
repo="$(cd "$(dirname "$0")/../.." && pwd)"
# Coax sources by their path in the repository (apps/openagents-web/x.coax),
# never the checkout's absolute path: the C embeds source paths for its
# runtime messages, so a module built in any checkout or temporary release
# tree has the same bytes and the same content-named URL (2026-10-30: every
# release renamed every engine and players downloaded them again).
relative() {
  local p; p="$(cd "$(dirname "$1")" && pwd -P)/$(basename "$1")"
  local r; r="$(cd "$repo" && pwd -P)"
  [[ "$p" == "$r"/* ]] && echo "${p#"$r"/}" || echo "$p"
}
coax_file="$(relative "$coax_file")"
for i in ${libraries[@]+"${!libraries[@]}"}; do libraries[i]="$(relative "${libraries[i]}")"; done
face="$(cd "$(dirname "$face")" && pwd)/$(basename "$face")"
mkdir -p "$(dirname "$out")"; out="$(cd "$(dirname "$out")" && pwd)/$(basename "$out")"
source "$repo/coax/web/toolchain.sh"
clang="$COAXIAL_CLANG"; sysroot="$COAXIAL_SYSROOT"; resources="$COAXIAL_RESOURCES"
name="$(basename "$coax_file" .coax)"
# Concurrent pages often compile the same libraries (and dev rebuilds may
# target the same output). Give each invocation its own headers, C and objects.
work_root="$(dirname "$out")/.c"
mkdir -p "$work_root"
work="$(mktemp -d "$work_root/$name.XXXXXXXX")"
trap 'rm -rf "$work"' EXIT
# COAXIAL_RUN_STEPS: a larger compile-time budget (coax --run-steps) for libraries whose
# tables are computed at compile time (secp256k1 signing, for /login's words account).
run_steps=(); [[ -n "${COAXIAL_RUN_STEPS:-}" ]] && run_steps=(--run-steps "$COAXIAL_RUN_STEPS")
(cd "$repo" && coax/bin/coax c "$coax_file" --library --header "$work/$name.h" ${run_steps[@]+"${run_steps[@]}"}) > "$work/$name.c"
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
  (cd "$repo" && coax/bin/coax c "$library" --library --header "$work/$base.h" ${run_steps[@]+"${run_steps[@]}"}) > "$work/$base.c"
  "$clang" "${cflags[@]}" -c "$work/$base.c" -o "$work/$base.o" & pids+=($!)
  objects+=("$work/$base.o")
done
"$clang" "${cflags[@]}" -c "$face" -o "$work/$name.face.o"
wait "$coax_pid"
for pid in ${pids[@]+"${pids[@]}"}; do wait "$pid"; done
link_opt=(); [[ "$mode" == dev ]] || link_opt=("$opt")
"$clang" --target=wasm32-wasip1 --sysroot="$sysroot" -resource-dir="$resources" ${link_opt[@]+"${link_opt[@]}"} \
  -mexec-model=reactor -o "$work/output.wasm" "${objects[@]}" \
  -Wl,--import-memory,--max-memory=67108864,-z,stack-size=1048576,--export=__heap_base,--gc-sections,--strip-debug
mv "$work/output.wasm" "$out"   # the server never serves a half-written module
