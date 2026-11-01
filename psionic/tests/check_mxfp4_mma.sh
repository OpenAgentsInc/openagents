#!/usr/bin/env bash
# Chapter 69: original MXFP4 expert products on the tensor cores under
# MXFP4-BF16-MMA-v1. Checks in debug and release, CUDA emission, the example
# transcript, and (with a CUDA driver and NVRTC) the products on the device
# against the binary64 reference and chapter 66's kernels at gpt-oss-20b
# expert shapes. Without a device the device step prints SKIP.
# Usage: psionic/tests/check_mxfp4_mma.sh [ORIGINAL_DIR] [--mutate]
# ORIGINAL_DIR holds original/model.safetensors of gpt-oss-20b; when given,
# layer 0's expert tensors are compared too.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
original=""; mutate=0
for arg in "$@"; do
    if [[ $arg == --mutate ]]; then mutate=1; else original=$arg; fi
done
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-mxfp4-mma.XXXXXX")
trap 'rm -rf "$out"' EXIT
for unit in mxfp4-mma mxfp4-mma-plan; do
    "$coax" test "psionic/lib/device/$unit.coax"
    "$coax" test "psionic/lib/device/$unit.coax" --release
done
"$coax" cuda psionic/lib/device/mxfp4-mma.coax --release > "$out/mxfp4-mma.cu"
grep -q 'mma.sync.aligned.m16n8k16' "$out/mxfp4-mma.cu"
for mode in "" --release; do
    "$coax" run psionic/examples/mxfp4-mma.coax $mode > "$out/mxfp4-mma.out"
    cmp psionic/fixtures/mxfp4-mma.out "$out/mxfp4-mma.out"
done
flags=(-std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -Wno-missing-field-initializers -Wno-misleading-indentation)
"$coax" c psionic/lib/device/mxfp4-mma-plan.coax --library --release --header "$out/mxfp4-mma-plan.h" > "$out/mxfp4-mma-plan.c"
"$cc" "${flags[@]}" -I"$out" -Ipsionic/platform "$out/mxfp4-mma-plan.c" psionic/platform/cuda-host.c \
    psionic/tests/mxfp4_mma_device.c -ldl -lm -o "$out/mxfp4-mma-device"
if [[ -n $original ]]; then
    # Tooling only: the absolute byte offsets of layer 0's expert tensors.
    offsets=$(python3 -I - "$original/original/model.safetensors" <<'EOF'
import json, struct, sys
with open(sys.argv[1], "rb") as f:
    n = struct.unpack("<Q", f.read(8))[0]
    header = json.loads(f.read(n))
names = ["mlp1_weight.blocks", "mlp1_weight.scales", "mlp1_bias", "mlp2_weight.blocks", "mlp2_weight.scales", "mlp2_bias"]
print(" ".join(str(8 + n + header["block.0.mlp." + k]["data_offsets"][0]) for k in names))
EOF
)
    # shellcheck disable=SC2086
    "$out/mxfp4-mma-device" "$out/mxfp4-mma.cu" "$original/original/model.safetensors" $offsets
else
    "$out/mxfp4-mma-device" "$out/mxfp4-mma.cu"
fi
if [[ $mutate == 1 ]]; then
    for unit in mxfp4-mma mxfp4-mma-plan; do "$coax" mutate "psionic/lib/device/$unit.coax" --root-only; done
fi
echo "chapter 69 passes"
