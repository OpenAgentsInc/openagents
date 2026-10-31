#!/usr/bin/env bash
# Focused chapter 38; no full mutation, original models or compiler bootstrap.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
device=0
case "${1:-}" in '') ;; --cuda) device=1 ;; --metal) device=2 ;; *) echo 'usage: check_tensor_ir.sh [--cuda|--metal]' >&2;exit 2;;esac
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-tensor-ir.XXXXXX")
trap 'rm -rf "$out"' EXIT
python3 -I psionic/tests/tensor_ir_oracle.py
python3 -I psionic/tests/tensor_ir_oracle.py --header --compiler "$coax" > "$out/tensor-ir-oracle.h"
module=()
if [[ $device == 1 ]];then "$coax" cuda psionic/lib/tensor-ir-kernels.coax > "$out/kernels.cu";module=("$out/kernels.cu");fi
# A bounds-checked candidate hit an Apple pipeline compiler failure. The
# qualified Metal artifact uses release emission and the same graph fixtures.
if [[ $device == 2 ]];then "$coax" metal-compute psionic/lib/tensor-ir-kernels.coax --release > "$out/kernels.metal";module=("$out/kernels.metal");fi
asan=detect_leaks=0:halt_on_error=1
if [[ $device == 1 ]];then asan+=:protect_shadow_gap=0;fi
for mode in debug release;do
    flags=(--run-steps 10000000);opt=-O0
    if [[ $mode == release ]];then flags+=(--release);opt=-O3;fi
    "$coax" test psionic/lib/tensor-ir-host.coax "${flags[@]}"
    "$coax" c psionic/lib/tensor-ir-host.coax --library --header "$out/tensor-ir.h" "${flags[@]}" > "$out/tensor-ir.c"
    for kind in plain sanitized;do
        san=();if [[ $kind == sanitized ]];then san=(-fsanitize=address,undefined -fno-omit-frame-pointer);fi
        common=(-std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas "${san[@]}" -I"$out" -Ipsionic/platform)
        "$cc" "${common[@]}" -c "$out/tensor-ir.c" -o "$out/coax.o"
        "$cc" "${common[@]}" -c psionic/tests/tensor_ir.c -o "$out/test.o"
        "$cc" "${common[@]}" -Wno-misleading-indentation -c psionic/platform/cuda-host.c -o "$out/cuda.o"
        objects=("$out/coax.o" "$out/test.o" "$out/cuda.o");libs=(-lm)
        if [[ $(uname -s) == Darwin ]];then
            "$cc" "${common[@]}" -fobjc-arc -c psionic/platform/metal-host.m -o "$out/metal.o";objects+=("$out/metal.o");libs+=(-framework Metal -framework Foundation)
        else libs+=(-ldl);fi
        "$cc" "${san[@]}" "${objects[@]}" "${libs[@]}" -o "$out/native"
        ASAN_OPTIONS="$asan" UBSAN_OPTIONS=halt_on_error=1 "$out/native" "$device" "${module[@]}"
    done
    "$coax" run psionic/examples/tensor-program.coax "${flags[@]}" > "$out/example.out"
    cmp psionic/fixtures/tensor-program.out "$out/example.out"
done
python3 -I psionic/tests/book.py
echo "Tensor IR: debug/release, native plain/ASan/UBSan, backend $device and exact book output pass"
