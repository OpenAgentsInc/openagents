#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around coax, clang, and trace-distill GPU host runners; holds no logic)
# End-to-end verification gate for Executor W3.2 (#13918, `trace-distill-v1`):
#   1. Coax unit tests (debug & release) on policy, single-source GPU kernels, and 3-arm finite-diff / W1.2 bridge.
#   2. Zero-survivor mutation check on `#authority` `psionic/lib/trace-distill-policy.coax`.
#   3. Coax Metal 4 (fast math OFF) and CUDA (--fmad=false) kernel emission & compilation.
#   4. Live Apple Silicon Metal 4 GPU run (GPU-vs-CPU parity, 10.72M exact save/resume,
#      W1.2/Ch234/Ch247 corpus admission, all 3 pre-registered arms under identical token budget,
#      and 288 standalone autoregressive Ch 234 rollouts across 1x/2x/4x/8x on 3 seen + 3 unseen families).
#   5. CUDA host build (-Wall -Wextra -Werror) + live NVIDIA GPU run if present, else clean PC_ABSENT.
# Usage: psionic/tests/check_trace_distill.sh [STEPS]
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
steps=${1:-320}
if [[ ! -x "$coax" ]]; then
    if [[ -n ${COAX:-} ]]; then echo "COAX is not executable: $coax" >&2; exit 1; fi
    coax/build.sh
fi
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-trace-distill.XXXXXX")
trap 'rm -rf "$out"' EXIT

"$coax" version
for mode in debug release; do
    flags=()
    if [[ $mode == release ]]; then flags=(--release); fi
    "$coax" test psionic/lib/trace-distill-policy.coax "${flags[@]}"
    "$coax" test psionic/lib/trace-distill-kernels.coax "${flags[@]}"
    "$coax" test psionic/lib/trace-distill.coax "${flags[@]}"
done

"$coax" mutate psionic/lib/trace-distill-policy.coax --root-only --native --jobs "${PSIONIC_JOBS:-8}"

"$coax" metal-compute psionic/lib/trace-distill-kernels.coax --release > "$out/trace-distill-kernels.metal"
"$coax" cuda psionic/lib/trace-distill-kernels.coax > "$out/trace-distill-kernels-debug.cu"
"$coax" cuda psionic/lib/trace-distill-kernels.coax --release > "$out/trace-distill-kernels.cu"
if [[ $(uname) == Darwin ]] && xcrun -sdk macosx -f metal >/dev/null 2>&1; then
    xcrun -sdk macosx metal -c "$out/trace-distill-kernels.metal" -o "$out/trace-distill-kernels.air"
fi

"$coax" c psionic/lib/trace-distill.coax --library --release --header "$out/trace-distill-bridge.h" > "$out/trace-distill-bridge.c"
cflags=(-std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -I"$out" -Ipsionic/platform)
"$cc" "${cflags[@]}" -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -Wno-missing-field-initializers -c "$out/trace-distill-bridge.c" -o "$out/bridge.o"

if [[ $(uname) == Darwin ]]; then
    "$cc" -fobjc-arc -mmacosx-version-min=15.0 -O2 -Wall -Wextra -Werror -c psionic/platform/metal-host.m -o "$out/metal-host.o"
    "$cc" "${cflags[@]}" -DTD_GPU_METAL psionic/platform/trace-distill-gpu-host.c "$out/bridge.o" "$out/metal-host.o" -framework Metal -framework Foundation -lm -o "$out/trace-distill-gpu-metal"
    "$out/trace-distill-gpu-metal" "$out/trace-distill-kernels.metal" --steps "$steps" --ckpt-dir "$out"
fi

"$cc" "${cflags[@]}" -DTD_GPU_CUDA psionic/platform/trace-distill-gpu-host.c "$out/bridge.o" psionic/platform/cuda-host.c -ldl -lm -o "$out/trace-distill-gpu-cuda"
"$out/trace-distill-gpu-cuda" "$out/trace-distill-kernels.cu" --steps "$steps" --ckpt-dir "$out"

echo "Executor W3.2 (trace-distill-v1) gate passed"
