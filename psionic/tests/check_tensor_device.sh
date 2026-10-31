#!/usr/bin/env bash
# Focused chapter 6 qualification; no mutation/full-model/compiler bootstrap.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
device=0
case "${1:-}" in
    '') ;;
    --metal) device=2 ;;
    --cuda) device=1 ;;
    *) echo 'usage: check_tensor_device.sh [--metal|--cuda]' >&2; exit 2 ;;
esac
asan_options=detect_leaks=0:halt_on_error=1
# CUDA reserves virtual addresses in ASan's shadow gap; keep instrumented host
# checks while allowing the driver to map that otherwise protected range.
if [[ $device == 1 ]];then asan_options+=:protect_shadow_gap=0;fi
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-tensor-device.XXXXXX")
trap 'rm -rf "$out"' EXIT
python3 -I psionic/tests/tensor_device_oracle.py
python3 -I psionic/tests/tensor_device_oracle.py --header > "$out/tensor-device-oracle.h"
module=()
if [[ $device == 2 ]];then
    "$coax" metal-compute psionic/lib/tensor-device-kernels.coax > "$out/kernels.metal"
    module=("$out/kernels.metal")
elif [[ $device == 1 ]];then
    "$coax" cuda psionic/lib/tensor-device-kernels.coax > "$out/kernels.cu"
    module=("$out/kernels.cu")
fi
for mode in debug release;do
    flags=(--run-steps 3000000);opt=-O0
    if [[ $mode == release ]];then flags+=(--release);opt=-O3;fi
    "$coax" test psionic/lib/tensor-device-host.coax "${flags[@]}"
    "$coax" c psionic/lib/tensor-device-host.coax --library --header "$out/tensor-device.h" "${flags[@]}" > "$out/tensor-device.c"
    for kind in plain sanitized;do
        san=();if [[ $kind == sanitized ]];then san=(-fsanitize=address,undefined -fno-omit-frame-pointer);fi
        common=(-std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas "${san[@]}" -I"$out" -Ipsionic/platform)
        "$cc" "${common[@]}" -c "$out/tensor-device.c" -o "$out/coax.o"
        "$cc" "${common[@]}" -c psionic/tests/tensor_device.c -o "$out/test.o"
        # Existing CUDA effects use compact C accepted by Clang; GCC warns on its layout.
        "$cc" "${common[@]}" -Wno-misleading-indentation -c psionic/platform/cuda-host.c -o "$out/cuda.o"
        native=("$out/coax.o" "$out/test.o" "$out/cuda.o");libs=(-lm)
        if [[ $(uname -s) == Darwin ]];then
            "$cc" "${common[@]}" -fobjc-arc -c psionic/platform/metal-host.m -o "$out/metal.o"
            native+=("$out/metal.o");libs+=(-framework Metal -framework Foundation)
        else libs+=(-ldl);fi
        "$cc" "${san[@]}" "${native[@]}" "${libs[@]}" -o "$out/native"
        ASAN_OPTIONS="$asan_options" UBSAN_OPTIONS=halt_on_error=1 "$out/native" "$device" "${module[@]}"
    done
    "$coax" run psionic/examples/tensor-devices.coax "${flags[@]}" > "$out/example.out"
    cmp psionic/fixtures/tensor-devices.out "$out/example.out"
done
python3 -I psionic/tests/book.py
echo "Tensor placement: debug/release, native plain/ASan/UBSan, backend $device and exact book output pass"
