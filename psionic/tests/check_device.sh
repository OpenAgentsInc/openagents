#!/usr/bin/env bash
# Focused gate for chapters 26 and 30: device kernels and plan checks, CUDA
# emission, examples and the host build. The device runs only when a CUDA
# driver and NVRTC are present; otherwise the harness reports SKIP.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
if [[ ! -x "$coax" ]]; then
    if [[ -n ${COAX:-} ]]; then echo "COAX is not executable: $coax" >&2; exit 1; fi
    coax/build.sh
fi
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-device.XXXXXX")
trap 'rm -rf "$out"' EXIT
"$coax" version
for unit in project layer plan; do
    "$coax" test "psionic/lib/device/$unit.coax"
    "$coax" test "psionic/lib/device/$unit.coax" --release
done
for unit in project layer; do
    "$coax" cuda "psionic/lib/device/$unit.coax" > "$out/$unit.cu"
    "$coax" cuda "psionic/lib/device/$unit.coax" --release > "$out/$unit-release.cu"
done
for example in device-matvec device-plan; do
    for mode in "" --release; do
        "$coax" run "psionic/examples/$example.coax" $mode > "$out/$example.out"
        cmp "psionic/fixtures/$example.out" "$out/$example.out"
    done
done
# The plan's C and the host adapter build without CUDA headers.
"$coax" c psionic/lib/device/plan.coax --library --release --header "$out/plan.h" > "$out/plan.c"
"$cc" -std=c11 -O2 -Wall -Wextra -Werror -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter \
    -I"$out" -Ipsionic/platform -c "$out/plan.c" -o "$out/plan.o"
"$cc" -std=c11 -O2 -Wall -Wextra -Werror -Ipsionic/platform -c psionic/platform/cuda-host.c -o "$out/cuda-host.o"
psionic/tests/run_device_project.sh quick
for unit in project layer plan; do "$coax" mutate "psionic/lib/device/$unit.coax"; done
echo "Device chapters pass: debug/release checks, CUDA emission, examples, host build, device projection and zero-survivor mutation"
