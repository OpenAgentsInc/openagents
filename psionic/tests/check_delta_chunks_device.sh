#!/usr/bin/env bash
# Chapter 42 on the device: dkd_chunk, the chunked Gated DeltaNet group
# kernel, inside chapter 44's linear-layer core at Elephant's shape against
# chapter 44's dl_scan path, its C build, chapter 42's CPU chunked profile and
# chapter 11's recurrence, with chapter 34's warm-run statistics over chunk
# sizes 4, 8, 16 and 32; with the original weights, linear layer 0 on the
# recorded decision prompt too. Without a CUDA device the device step prints SKIP.
# Firm reason: 5 (a thin wrapper: builds the CUDA module and the C harness and runs them; no decision is made here)
# Usage: psionic/tests/check_delta_chunks_device.sh [ORIGINAL_DIR] [--mutate]
# ORIGINAL_DIR holds Elephant's model.safetensors.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
original=""; mutate=0
for arg in "$@"; do
    if [[ $arg == --mutate ]]; then mutate=1; else original=$arg; fi
done
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-delta-chunks.XXXXXX")
trap 'rm -rf "$out"' EXIT
for unit in delta-chunks-kernels delta-chunks-plan; do
    "$coax" test "psionic/lib/device/$unit.coax"
done
"$coax" test psionic/tests/delta-chunks-device-runtime.coax
"$coax" cuda psionic/lib/device/delta-chunks-kernels.coax --release > "$out/dk.cu"
grep -q '__shared__' "$out/dk.cu"
python3 -I psionic/tests/elephant_reference_fixture.py --header > "$out/elephant-request.h"
"$coax" c psionic/tests/delta-chunks-device-runtime.coax --library --release --header "$out/delta-chunks-device-runtime.h" > "$out/delta-chunks-device-runtime.c"
"$coax" c psionic/lib/comparator.coax --library --release --header "$out/comparator.h" > "$out/comparator.c"
flags=(-std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -Wno-missing-field-initializers -Wno-misleading-indentation)
"$cc" "${flags[@]}" -I"$out" -Ipsionic/platform "$out/delta-chunks-device-runtime.c" "$out/comparator.c" psionic/platform/cuda-host.c \
    psionic/tests/delta_chunks_device.c -ldl -lm -o "$out/delta-chunks-device"
build=$(git hash-object psionic/lib/device/delta-chunks-kernels.coax psionic/lib/device/layer.coax psionic/tests/delta_chunks_device.c | paste -sd, -)
if [[ -n $original ]]; then
    "$out/delta-chunks-device" "$out/dk.cu" "$build" "$original/model.safetensors"
else
    "$out/delta-chunks-device" "$out/dk.cu" "$build"
fi
if [[ $mutate == 1 ]]; then
    for unit in delta-chunks-kernels delta-chunks-plan; do "$coax" mutate "psionic/lib/device/$unit.coax" --root-only; done
fi
echo "chapter 42 device check passes"
