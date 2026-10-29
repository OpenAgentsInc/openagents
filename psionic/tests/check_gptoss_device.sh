#!/usr/bin/env bash
# Focused checks only. No original CPU decisions and no full mutation runs.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-gptoss-device.XXXXXX")
trap 'rm -rf "$out"' EXIT
python3 -I psionic/tests/gptoss_fixture.py --header > "$out/gptoss-tiny.h"
libs=(-lm)
if [[ $(uname -s) == Linux ]]; then libs+=(-ldl); fi
for mode in debug release; do
    flags=(--run-steps 2000000); opt=-O0
    if [[ $mode == release ]]; then flags+=(--release); opt=-O3; fi
    "$coax" test psionic/lib/gptoss/device-kernels.coax "${flags[@]}"
    "$coax" test psionic/lib/gptoss/device-prefill.coax "${flags[@]}"
    "$coax" test psionic/lib/gptoss/device-replay.coax "${flags[@]}"
    "$coax" c psionic/lib/gptoss/device-host.coax --library --header "$out/device-host.h" "${flags[@]}" > "$out/device-host.c"
    python3 -I psionic/tests/gptoss_device_bindings.py "$out/device-host.h" > "$out/gptoss-device-cpu.h"
    for native in plain sanitized; do
        san=(); if [[ $native == sanitized ]]; then san=(-fsanitize=address,undefined -fno-omit-frame-pointer); fi
        "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -DPSIONIC_DEVICE_TEST "${san[@]}" \
            -I"$out" -Ipsionic/platform "$out/device-host.c" psionic/platform/cuda-host.c \
            psionic/platform/gptoss-device.c psionic/tests/gptoss_device.c "${libs[@]}" -o "$out/native-$native"
        ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 \
            "$out/native-$native" cpu psionic/fixtures/gptoss/tiny/model.safetensors unused
    done
    "$coax" run psionic/examples/gptoss-device-plan.coax "${flags[@]}" > "$out/example.out"
    cmp psionic/fixtures/gptoss-device-plan.out "$out/example.out"
done
"$cc" -std=c11 -O3 -ffp-contract=off -fno-fast-math -I"$out" -Ipsionic/platform \
    "$out/device-host.c" psionic/platform/cuda-host.c psionic/platform/gptoss-device.c \
    psionic/platform/gptoss-chat.c "${libs[@]}" -o "$out/chat"
if [[ ${PSIONIC_CUDA:-0} == 1 ]]; then
    "$coax" cuda psionic/lib/gptoss/device-kernels.coax > "$out/device-kernels.cu"
    "$out/native-plain" tiny "$out/device-kernels.cu" psionic/fixtures/gptoss/tiny/model.safetensors unused
    "$cc" -std=c11 -O3 -ffp-contract=off -fno-fast-math -DPSIONIC_DEVICE_TEST \
        -I"$out" -Ipsionic/platform "$out/device-host.c" psionic/platform/cuda-host.c \
        psionic/tests/gptoss_replay.c "${libs[@]}" -o "$out/replay"
    "$out/replay" "$out/device-kernels.cu" psionic/fixtures/gptoss/tiny/model.safetensors
    if [[ ${PSIONIC_DEVICE_PRODUCT_BENCH:-0} == 1 ]]; then
        "$cc" -std=c11 -O2 -Ipsionic/platform psionic/platform/cuda-host.c \
            psionic/tests/gptoss_device_product_bench.c "${libs[@]}" -o "$out/product"
        "$out/product" "$out/device-kernels.cu" 4 8 128
    fi
    if [[ -n ${PSIONIC_GPTOSS_ORIGINAL:-} ]]; then
        mkdir "$out/original"
        "$out/native-plain" original "$out/device-kernels.cu" "$PSIONIC_GPTOSS_ORIGINAL" "$out/original" bench | tee "$out/original/run.log"
        python3 -I psionic/tests/gptoss_device_benchmark.py "$out/original/run.log" "$out/original/benchmark.json"
        python3 -I psionic/tests/gptoss_device_oracle.py 20b "$out/original"
        if [[ -n ${PSIONIC_DEVICE_RECORD:-} ]]; then mkdir -p "$PSIONIC_DEVICE_RECORD"; cp -R "$out/original/." "$PSIONIC_DEVICE_RECORD/"; fi
    fi
fi
python3 -I psionic/tests/book.py
echo 'GPT-OSS device focused checks pass; CPU emulation uses tiny owned weights only.'
