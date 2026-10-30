#!/usr/bin/env bash
# Chapter 30: the original Elephant decision on a CUDA device. The caller
# supplies the original files; nothing is downloaded. Without a CUDA driver or
# NVRTC the harness prints SKIP and exits 0. Usage:
#   psionic/tests/run_elephant_device.sh ORIGINAL_DIRECTORY [REPEAT]
# PSIONIC_DEVICE_PROFILE=1 adds one run timed launch by launch.
set -euo pipefail
cd "$(dirname "$0")/../.."
if [[ $# -lt 1 || $# -gt 2 ]]; then echo "usage: psionic/tests/run_elephant_device.sh ORIGINAL_DIRECTORY [REPEAT]" >&2; exit 2; fi
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-elephant-device.XXXXXX")
trap 'rm -rf "$out"' EXIT
python3 -I psionic/tests/elephant_manifest.py --header > "$out/elephant-inventory.h"
python3 -I psionic/tests/elephant_reference_fixture.py --header > "$out/elephant-request.h"
for unit in decision-session decision-prepare decision-answer; do
    "$coax" c "psionic/lib/$unit.coax" --library --release --header "$out/$unit.h" > "$out/$unit.c"
done
"$coax" c psionic/lib/device/plan.coax --library --release --header "$out/plan.h" > "$out/plan.c"
"$coax" cuda psionic/lib/device/project.coax --release > "$out/project.cu"
"$coax" cuda psionic/lib/device/layer.coax --release > "$out/layer.cu"
"$coax" cuda psionic/lib/device/warp.coax --release > "$out/warp.cu"
"$cc" -std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unknown-pragmas -Wno-unused-function \
    -Wno-unused-parameter -Wno-missing-field-initializers -I"$out" -Ipsionic/platform \
    "$out/decision-answer.c" "$out/plan.c" psionic/platform/decision-session-memory.c psionic/platform/decision-prepare-memory.c \
    psionic/platform/decision-answer-memory.c psionic/platform/cuda-host.c psionic/platform/elephant-gpu.c psionic/platform/verified-file.c psionic/tests/elephant_device.c -ldl -lm -o "$out/elephant-device"
"$out/elephant-device" "$1" psionic/fixtures/text/ch05/qwen35.pst "$out/project.cu" "$out/layer.cu" "$out/warp.cu" "${2:-3}" | tee "$out/result.log"
if grep -q '^SKIP' "$out/result.log"; then exit 0; fi
# Frozen before the device was measured: 0.0005 absolute on each decision
# statistic, against the independent reference and the recorded CPU result.
python3 -I psionic/tests/elephant_reference_fixture.py --check-output "$out/result.log"
python3 -I psionic/tests/elephant_reference_fixture.py --check-cpu "$out/result.log"
