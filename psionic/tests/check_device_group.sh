#!/usr/bin/env bash
# Chapter 132: a device group of host threads. Checks in debug and release,
# the example transcripts, a small run under ThreadSanitizer, then the
# release harness: the chain all-reduce at 1, 2, 4, 8 and 16 devices against
# chapter 118's CPU run bit for bit, the tree against itself and within the
# frozen tolerance, a late step refused, and a speed receipt; then a group
# of a host thread and the CUDA device, within the frozen tolerance.
# Usage: psionic/tests/check_device_group.sh [--mutate] [STEPS]
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
mutate=0
if [[ ${1:-} == --mutate ]]; then mutate=1; shift; fi
steps=${1:-6}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-device-group.XXXXXX")
trap 'rm -rf "$out"' EXIT
unit=psionic/lib/transformer/device-group.coax
"$coax" test "$unit"
"$coax" test "$unit" --release
for name in device-group device-group-refuse; do
    for mode in "" --release; do
        "$coax" run "psionic/examples/$name.coax" $mode > "$out/$name.out"
        cmp "psionic/fixtures/$name.out" "$out/$name.out"
    done
done
flags=(-std=c11 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -Wno-missing-field-initializers -Wno-misleading-indentation)
"$coax" c "$unit" --library --release --header "$out/device-group.h" > "$out/device-group.c"
"$cc" "${flags[@]}" -O1 -g -fsanitize=thread -I"$out" "$out/device-group.c" psionic/tests/device_group_threads.c -lm -pthread -o "$out/threads-tsan"
TSAN_OPTIONS=halt_on_error=1 "$out/threads-tsan" 2 4 > /dev/null
"$cc" "${flags[@]}" -O2 -I"$out" "$out/device-group.c" psionic/tests/device_group_threads.c -lm -pthread -o "$out/threads"
"$out/threads" "$steps"
# The GPU as a member: prints SKIP without a CUDA device or NVRTC.
"$coax" cuda psionic/lib/transformer/train-kernels.coax --release > "$out/train-kernels.cu"
"$cc" "${flags[@]}" -O2 -I"$out" -Ipsionic/platform "$out/device-group.c" psionic/platform/cuda-host.c psionic/tests/device_group_gpu.c -ldl -lm -pthread -o "$out/gpu"
"$out/gpu" "$out/train-kernels.cu" "$steps"
if [[ $mutate == 1 ]]; then "$coax" mutate "$unit" --root-only; fi
echo "chapter 132 passes"
