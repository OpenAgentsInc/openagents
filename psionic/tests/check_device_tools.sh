#!/usr/bin/env bash
# Chapter 78: device generation calls a tool. Checks in debug and release and
# both example transcripts; then the harness's refusal with no CUDA device
# visible (exit 5, no CPU fallback). With ORIGINAL_DIR and a CUDA device, the
# round trips on the original gpt-oss-20b against chapter 77's recorded CPU
# reference IDs.
# Usage: psionic/tests/check_device_tools.sh [ORIGINAL_DIR [SEEDS]]
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-device-tools.XXXXXX")
trap 'rm -rf "$out"' EXIT
for mode in "" --release; do
    "$coax" test psionic/lib/device-tools.coax $mode
    for example in device-tool-call device-tool-call-refuse; do
        "$coax" run "psionic/examples/$example.coax" $mode > "$out/$example.out"
        cmp "psionic/fixtures/device-tools/$example.out" "$out/$example.out"
    done
done
"$coax" cuda psionic/lib/gptoss/device-kernels.coax > "$out/device-kernels.cu"
"$coax" c psionic/tests/device-tools-host.coax --library --release --run-steps 2000000 --header "$out/device-host.h" > "$out/device-host.c"
"$cc" -std=c11 -O3 -ffp-contract=off -fno-fast-math -I"$out" -Ipsionic/platform "$out/device-host.c" \
    psionic/platform/cuda-host.c psionic/platform/gptoss-device.c psionic/tests/gptoss_device_tools.c -lm -ldl -o "$out/tools"
# No C copy of any model kernel is linked: the layers can only run on the device.
if nm "$out/tools" | grep -E ' T (gk_|mm_|dw_|pg_open_cpu|pg_cpu_launch)'; then echo "a CPU copy of a model kernel is linked" >&2; exit 1; fi
status=0
CUDA_VISIBLE_DEVICES= "$out/tools" "$out/device-kernels.cu" /nonexistent psionic/fixtures/gptoss/qualification/tools > "$out/absent.out" || status=$?
if [[ $status != 5 ]]; then echo "no device: expected exit 5, got $status" >&2; exit 1; fi
cmp psionic/fixtures/device-tools/no-device.out "$out/absent.out"
if [[ $# -lt 1 ]]; then echo "chapter 78 checks pass (no original weights given: device round trips not run)"; exit 0; fi
echo "load average before: $(cut -d' ' -f1-3 /proc/loadavg)"
"$out/tools" "$out/device-kernels.cu" "$1" psionic/fixtures/gptoss/qualification/tools "${2:-8}"
echo "load average after: $(cut -d' ' -f1-3 /proc/loadavg)"
