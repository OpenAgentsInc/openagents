#!/usr/bin/env bash
# Chapter 130: profile chapter 129's training step. Checks in debug and
# release, CUDA emission of the timed kernels, the example transcripts, and
# the profiling harness: the CPU target always, the GPU with a CUDA driver
# and NVRTC (otherwise SKIP). The harness writes a receipt (JSON) to
# RECEIPT, default $TMPDIR; times there are measurements, never a transcript.
# Usage: psionic/tests/check_train_profile.sh [--mutate] [RUNS [ROWS [RECEIPT]]]
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
mutate=0
if [[ ${1:-} == --mutate ]]; then mutate=1; shift; fi
runs=${1:-20}
rows=${2:-4}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-train-profile.XXXXXX")
trap 'rm -rf "$out"' EXIT
receipt=${3:-$out/receipt.json}
export COAX_BUILD=${COAX_BUILD:-coax $(basename "$(dirname "$(readlink -f "$coax")")")}
for unit in profile-kernels train-profile; do
    "$coax" test "psionic/lib/transformer/$unit.coax"
    "$coax" test "psionic/lib/transformer/$unit.coax" --release
done
"$coax" cuda psionic/lib/transformer/profile-kernels.coax --release > "$out/profile-kernels.cu"
for name in train-profile train-profile-refuse; do
    for mode in "" --release; do
        "$coax" run "psionic/examples/$name.coax" $mode > "$out/$name.out"
        cmp "psionic/fixtures/$name.out" "$out/$name.out"
    done
done
flags=(-std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -Wno-missing-field-initializers -Wno-misleading-indentation)
"$coax" c psionic/tests/train-profile-device.coax --library --release --header "$out/train-profile.h" > "$out/train-profile.c"
"$cc" "${flags[@]}" -I"$out" -Ipsionic/platform "$out/train-profile.c" psionic/platform/cuda-host.c \
    psionic/tests/train_profile_device.c -ldl -lm -o "$out/train-profile-device"
"$out/train-profile-device" "$out/profile-kernels.cu" "$receipt" "$runs" "$rows"
if [[ $mutate == 1 ]]; then
    for unit in profile-kernels train-profile; do "$coax" mutate "psionic/lib/transformer/$unit.coax" --root-only; done
fi
echo "chapter 130 passes"
