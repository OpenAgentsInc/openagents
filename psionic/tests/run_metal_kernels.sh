#!/usr/bin/env bash
# Chapter 31: the chapter 26 and 30 kernels on an Apple GPU (coax
# metal-compute), against the same kernels compiled to C, the CPU reference
# profile and binary64. Without a Metal device the harness prints SKIP and
# exits 0. Usage:
#   psionic/tests/run_metal_kernels.sh [quick]
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
if [[ $(uname) != Darwin ]]; then echo "SKIP: Metal needs macOS"; exit 0; fi
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-metal-kernels.XXXXXX")
trap 'rm -rf "$out"' EXIT
"$coax" test psionic/tests/metal-ff.coax
for unit in project warp; do
    "$coax" metal-compute "psionic/lib/device/$unit.coax" --release > "$out/$unit.metal"
    "$coax" c "psionic/lib/device/$unit.coax" --library --release --header "$out/$unit.h" > "$out/$unit.c"
done
"$coax" metal-compute psionic/tests/metal-ff.coax --release > "$out/metal-ff.metal"
"$coax" c psionic/tests/metal-ff.coax --library --release --header "$out/metal-ff.h" > "$out/metal-ff.c"
"$coax" c psionic/lib/qwen-matvec.coax --library --release --header "$out/qwen-matvec.h" > "$out/qwen-matvec.c"
flags=(-std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unknown-pragmas -Wno-unused-function
    -Wno-unused-parameter -Wno-missing-field-initializers -I"$out" -Ipsionic/platform)
for unit in project warp metal-ff qwen-matvec; do "$cc" "${flags[@]}" -c "$out/$unit.c" -o "$out/$unit.o"; done
"$cc" -fobjc-arc -mmacosx-version-min=15.0 -O2 -Wall -Wextra -Werror -c psionic/platform/metal-host.m -o "$out/metal-host.o"
"$cc" "${flags[@]}" -fblocks psionic/tests/metal_kernels.c "$out"/project.o "$out"/warp.o "$out"/metal-ff.o "$out"/qwen-matvec.o "$out/metal-host.o" \
    -framework Metal -framework Foundation -lm -o "$out/metal-kernels"
"$out/metal-kernels" "$out/project.metal" "$out/warp.metal" "$out/metal-ff.metal" "$@"
