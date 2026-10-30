#!/usr/bin/env bash
# Focused chapter 26c; no full mutation, original models or compiler bootstrap.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
device=0
case "${1:-}" in '') ;; --cuda) device=1 ;; --metal) device=2 ;; *) echo 'usage: check_operator_registry.sh [--cuda|--metal]' >&2;exit 2;;esac
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-operator.XXXXXX")
trap 'rm -rf "$out"' EXIT
python3 - "$coax" "$out/operator-producer.h" <<'PYHEADER'
import hashlib,pathlib,sys
h=hashlib.sha256(pathlib.Path(sys.argv[1]).read_bytes()).digest()
pathlib.Path(sys.argv[2]).write_text('static const uint64_t qualified_compiler[4]={'+','.join('UINT64_C(0x'+h[i:i+8].hex()+')' for i in range(0,32,8))+'};\n')
PYHEADER
module=(psionic/examples/operators/squared-relu.coax "$out/base.cu")
"$coax" cuda psionic/lib/tensor-ir-kernels.coax > "$out/base.cu"
if [[ $device == 1 ]];then "$coax" cuda psionic/lib/tensor-ir-kernels.coax > "$out/kernels.cu";"$coax" cuda psionic/examples/operators/squared-relu.coax > "$out/custom.cu";module=("$out/custom.cu" "$out/kernels.cu");fi
# A bounds-checked candidate hit an Apple pipeline compiler failure. The
# qualified Metal artifact uses release emission and the same graph fixtures.
if [[ $device == 2 ]];then "$coax" metal-compute psionic/lib/tensor-ir-kernels.coax --release > "$out/kernels.metal";"$coax" metal-compute psionic/examples/operators/squared-relu.coax --release > "$out/custom.metal";module=("$out/custom.metal" "$out/kernels.metal");fi
asan=detect_leaks=0:halt_on_error=1
if [[ $device == 1 ]];then asan+=:protect_shadow_gap=0;fi
for mode in debug release;do
    flags=(--run-steps 30000000);opt=-O0
    if [[ $mode == release ]];then flags+=(--release);opt=-O3;fi
    "$coax" test psionic/lib/operator-training.coax "${flags[@]}"
    "$coax" c psionic/lib/operator-training.coax --library --header "$out/tensor-ir.h" "${flags[@]}" > "$out/operator.c"
    for kind in plain sanitized;do
        san=();if [[ $kind == sanitized ]];then san=(-fsanitize=address,undefined -fno-omit-frame-pointer);fi
        common=(-std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas "${san[@]}" -I"$out" -Ipsionic/platform)
        "$cc" "${common[@]}" -c "$out/operator.c" -o "$out/coax.o"
        "$cc" "${common[@]}" -c psionic/tests/operator_registry.c -o "$out/test.o"
        "$cc" "${common[@]}" -c psionic/platform/tensor-ir-runtime.c -o "$out/runtime.o"
        "$cc" "${common[@]}" -Wno-misleading-indentation -c psionic/platform/cuda-host.c -o "$out/cuda.o"
        objects=("$out/runtime.o" "$out/coax.o" "$out/test.o" "$out/cuda.o");libs=(-lm)
        if [[ $(uname -s) == Darwin ]];then
            "$cc" "${common[@]}" -fobjc-arc -c psionic/platform/metal-host.m -o "$out/metal.o";objects+=("$out/metal.o");libs+=(-framework Metal -framework Foundation)
        else libs+=(-ldl);fi
        "$cc" "${san[@]}" "${objects[@]}" "${libs[@]}" -o "$out/native"
        ASAN_OPTIONS="$asan" UBSAN_OPTIONS=halt_on_error=1 "$out/native" "$device" "${module[@]}"
    done
    "$coax" run psionic/examples/registered-operator.coax "${flags[@]}" > "$out/example.out"
    cmp psionic/fixtures/registered-operator.out "$out/example.out"
done
python3 -I psionic/tests/book.py
echo "Registered operators: debug/release, native plain/ASan/UBSan, backend $device and exact book output pass"
