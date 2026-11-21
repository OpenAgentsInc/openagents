#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around coax, clang, and GPU host runners; holds no logic)
# End-to-end verification gate for Executor W1.3 (#13916):
#   1. Coax unit tests (debug & release) on scheduler, kernels, and GPU admission.
#   2. Zero-survivor mutation tests on #authority scheduler & GPU admission units.
#   3. Coax Metal 4 (fast math OFF) and CUDA (--fmad=false) kernel emission & compilation.
#   4. Live Apple Silicon Metal GPU execution (54/54 Ch 226 programs + Ch 233 sweep + benchmarks).
#   5. CUDA host build (-Wall -Wextra -Werror) + live NVIDIA GPU run if present, else clean PC_ABSENT.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
if [[ ! -x "$coax" ]]; then
    if [[ -n ${COAX:-} ]]; then echo "COAX is not executable: $coax" >&2; exit 1; fi
    coax/build.sh
fi
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-exec-gpu.XXXXXX")
trap 'rm -rf "$out"' EXIT

"$coax" version
for mode in debug release; do
    flags=()
    if [[ $mode == release ]]; then flags=(--release); fi
    "$coax" test psionic/lib/exec-vm-kernels.coax "${flags[@]}"
    "$coax" test psionic/lib/exec-vm-sched.coax "${flags[@]}"
    "$coax" test psionic/lib/exec-vm-opt-compile.coax "${flags[@]}"
    "$coax" test psionic/lib/exec-vm-gpu.coax "${flags[@]}"
done

"$coax" mutate psionic/lib/exec-vm-gpu.coax --root-only --native --jobs "${PSIONIC_JOBS:-8}"
"$coax" mutate psionic/lib/exec-vm-sched.coax --root-only --native --jobs "${PSIONIC_JOBS:-8}"

"$coax" metal-compute psionic/lib/exec-vm-kernels.coax --release > "$out/exec-vm-kernels.metal"
"$coax" cuda psionic/lib/exec-vm-kernels.coax > "$out/exec-vm-kernels-debug.cu"
"$coax" cuda psionic/lib/exec-vm-kernels.coax --release > "$out/exec-vm-kernels.cu"
if [[ $(uname) == Darwin ]] && xcrun -sdk macosx -f metal >/dev/null 2>&1; then
    xcrun -sdk macosx metal -c "$out/exec-vm-kernels.metal" -o "$out/exec-vm-kernels.air"
fi

"$coax" c psionic/platform/exec-vm-gpu-bridge.coax --library --release --header "$out/evg-bridge.h" > "$out/evg-bridge.c"
cflags=(-std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -I"$out" -Ipsionic/platform)
"$cc" "${cflags[@]}" -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -Wno-missing-field-initializers -c "$out/evg-bridge.c" -o "$out/evg-bridge.o"

if [[ $(uname) == Darwin ]]; then
    "$cc" -fobjc-arc -mmacosx-version-min=15.0 -O2 -Wall -Wextra -Werror -c psionic/platform/metal-host.m -o "$out/metal-host.o"
    "$cc" "${cflags[@]}" -DEXEC_GPU_METAL psionic/platform/exec-vm-gpu-host.c "$out/evg-bridge.o" "$out/metal-host.o" -framework Metal -framework Foundation -lm -o "$out/exec-vm-gpu-metal"
    "$out/exec-vm-gpu-metal" "$out/exec-vm-kernels.metal"
fi

"$cc" "${cflags[@]}" -DEXEC_GPU_CUDA psionic/platform/exec-vm-gpu-host.c "$out/evg-bridge.o" psionic/platform/cuda-host.c -ldl -lm -o "$out/exec-vm-gpu-cuda"
"$out/exec-vm-gpu-cuda" "$out/exec-vm-kernels.cu"

"$coax" run psionic/examples/exec-vm-gpu.coax --release
echo "Executor W1.3 gate passed"
