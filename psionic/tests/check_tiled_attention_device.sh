#!/usr/bin/env bash
# Chapter 41 on the device: td_attend, the tiled-attention group kernel, at
# gpt-oss-20b's attention shapes against chapter 66/67's prefill attention
# kernels, its C build, chapter 41's CPU profile and a binary64 oracle, with
# chapter 34's warm-run statistics; with the original weights, layer 0 on a
# recorded prefill too. Without a CUDA device the device step prints SKIP.
# Firm reason: 5 (a thin wrapper: builds the CUDA modules and the C harness and runs them; no decision is made here)
# Usage: psionic/tests/check_tiled_attention_device.sh [ORIGINAL_DIR] [--mutate]
# ORIGINAL_DIR holds original/model.safetensors of gpt-oss-20b.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
original=""; mutate=0
for arg in "$@"; do
    if [[ $arg == --mutate ]]; then mutate=1; else original=$arg; fi
done
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-tiled-attention.XXXXXX")
trap 'rm -rf "$out"' EXIT
for unit in tiled-attention-kernels tiled-attention-plan; do
    "$coax" test "psionic/lib/device/$unit.coax"
    "$coax" test "psionic/lib/device/$unit.coax" --release
done
"$coax" cuda psionic/lib/device/tiled-attention-kernels.coax --release > "$out/td.cu"
grep -q '__shared__' "$out/td.cu"
"$coax" cuda psionic/lib/gptoss/device-kernels.coax --release > "$out/gk.cu"
flags=(-std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -Wno-missing-field-initializers -Wno-misleading-indentation)
"$coax" c psionic/lib/device/tiled-attention-plan.coax --library --release --header "$out/tiled-attention-plan.h" > "$out/tiled-attention-plan.c"
"$coax" c psionic/lib/comparator.coax --library --release --header "$out/comparator.h" > "$out/comparator.c"
"$cc" "${flags[@]}" -I"$out" -Ipsionic/platform "$out/tiled-attention-plan.c" "$out/comparator.c" psionic/platform/cuda-host.c \
    psionic/tests/tiled_attention_device.c -ldl -lm -o "$out/tiled-attention-device"
build=$(git hash-object psionic/lib/device/tiled-attention-kernels.coax psionic/lib/gptoss/device-kernels.coax psionic/tests/tiled_attention_device.c | paste -sd, -)
if [[ -n $original ]]; then
    # Tooling only: the absolute byte offsets of layer 0's attention tensors.
    offsets=$(python3 -I - "$original/original/model.safetensors" <<'EOF'
import json, struct, sys
with open(sys.argv[1], "rb") as f:
    n = struct.unpack("<Q", f.read(8))[0]
    header = json.loads(f.read(n))
names = ["embedding.weight", "block.0.attn.norm.scale", "block.0.attn.qkv.weight", "block.0.attn.qkv.bias", "block.0.attn.sinks"]
print(" ".join(str(8 + n + header[k]["data_offsets"][0]) for k in names))
EOF
)
    # shellcheck disable=SC2086
    "$out/tiled-attention-device" "$out/td.cu" "$out/gk.cu" "$build" "$original/original/model.safetensors" $offsets \
        psionic/fixtures/gptoss/qualification/20b/generation.i64
else
    "$out/tiled-attention-device" "$out/td.cu" "$out/gk.cu" "$build"
fi
if [[ $mutate == 1 ]]; then
    for unit in tiled-attention-kernels tiled-attention-plan; do "$coax" mutate "psionic/lib/device/$unit.coax" --root-only; done
fi
echo "chapter 41 device check passes"
