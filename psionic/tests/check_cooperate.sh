#!/usr/bin/env bash
# Chapter 68: cooperative group kernels and device capability admission.
# Checks in debug and release, CUDA emission, the exact refusals of kernels
# outside the group-kernel rules, the example transcript, and (with a CUDA
# driver and NVRTC) the kernels on the device against the C build, bit for
# bit. Without a device the device step prints SKIP.
# Usage: psionic/tests/check_cooperate.sh [--mutate]
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-cooperate.XXXXXX")
trap 'rm -rf "$out"' EXIT
for unit in cooperate admit; do
    "$coax" test "psionic/lib/device/$unit.coax"
    "$coax" test "psionic/lib/device/$unit.coax" --release
done
"$coax" cuda psionic/lib/device/cooperate.coax --release > "$out/cooperate.cu"
grep -q '__shared__' "$out/cooperate.cu"
grep -q '__syncthreads' "$out/cooperate.cu"
# Kernels outside the rules are refused with exit status 1 and these exact
# diagnostics.
for bad in early-return race shared-value size; do
    status=0
    "$coax" cuda "psionic/fixtures/cooperate-refused/$bad.coax" > /dev/null 2> "$out/$bad.err" || status=$?
    if [[ $status != 1 ]]; then echo "cooperate-refused/$bad: exit status $status, expected 1" >&2; exit 1; fi
    cmp "psionic/fixtures/cooperate-refused/$bad.err" "$out/$bad.err"
done
echo "refusals: 4 kernels refused with the recorded diagnostics"
for mode in "" --release; do
    "$coax" run psionic/examples/cooperate.coax $mode > "$out/cooperate.out"
    cmp psionic/fixtures/cooperate.out "$out/cooperate.out"
done
flags=(-std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -Wno-missing-field-initializers -Wno-misleading-indentation)
"$coax" c psionic/lib/device/cooperate.coax --library --release --header "$out/cooperate.h" > "$out/cooperate.c"
"$coax" c psionic/lib/device/admit.coax --library --release --header "$out/admit.h" > "$out/admit.c"
"$cc" "${flags[@]}" -I"$out" -Ipsionic/platform "$out/cooperate.c" "$out/admit.c" psionic/platform/cuda-host.c \
    psionic/tests/cooperate_device.c -ldl -lm -o "$out/cooperate-device"
"$out/cooperate-device" "$out/cooperate.cu"
if [[ ${1:-} == --mutate ]]; then
    for unit in cooperate admit; do "$coax" mutate "psionic/lib/device/$unit.coax"; done
fi
echo "chapter 68 passes"
