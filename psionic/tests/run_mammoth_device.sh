#!/usr/bin/env bash
# Chapter 46: the original Mammoth decision on a CUDA device. The caller
# supplies the original files; nothing is downloaded. Without a CUDA driver or
# NVRTC the harness prints SKIP and exits 0. Usage:
#   psionic/tests/run_mammoth_device.sh ORIGINAL_DIRECTORY [REPEAT]
# PSIONIC_DEVICE_BUDGET_MIB=N places within N MiB, streaming the rest.
set -euo pipefail
cd "$(dirname "$0")/../.."
if [[ $# -lt 1 || $# -gt 2 ]]; then echo "usage: psionic/tests/run_mammoth_device.sh ORIGINAL_DIRECTORY [REPEAT]" >&2; exit 2; fi
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-mammoth-device.XXXXXX")
trap 'rm -rf "$out"' EXIT
python3 -I psionic/tests/mammoth_manifest.py --header > "$out/mammoth-inventory.h"
python3 -I psionic/tests/mammoth_reference_fixture.py --header > "$out/mammoth-request.h"
for unit in decision-session decision-prepare decision-answer decision-plan; do
    "$coax" c "psionic/lib/$unit.coax" --library --release --header "$out/$unit.h" > "$out/$unit.c"
done
# decision-plan.c holds the session, preparation and answer; the engine's
# header names it decision-answer.h.
cp "$out/decision-plan.h" "$out/decision-answer.h"
"$coax" c psionic/lib/device/plan.coax --library --release --header "$out/plan.h" > "$out/plan.c"
"$coax" cuda psionic/lib/device/project.coax --release > "$out/project.cu"
"$coax" cuda psionic/lib/device/layer.coax --release > "$out/layer.cu"
"$coax" cuda psionic/lib/device/warp.coax --release > "$out/warp.cu"
"$cc" -std=gnu11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unknown-pragmas -Wno-unused-function \
    -Wno-unused-parameter -Wno-missing-field-initializers -I"$out" -Ipsionic/platform \
    "$out/decision-plan.c" "$out/plan.c" psionic/platform/decision-session-memory.c psionic/platform/decision-prepare-memory.c \
    psionic/platform/decision-answer-memory.c psionic/platform/cuda-host.c psionic/platform/elephant-gpu.c psionic/tests/mammoth_device.c -ldl -lm -o "$out/mammoth-device"
"$out/mammoth-device" "$1" psionic/fixtures/text/ch05/qwen35.pst "$out/project.cu" "$out/layer.cu" "$out/warp.cu" "${2:-3}" | tee "$out/result.log"
if grep -q '^SKIP' "$out/result.log"; then exit 0; fi
# Frozen before the first device run: the choice and 0.0005 absolute on each
# decision statistic, against chapter 15's recorded CPU result and its
# independent reference.
python3 -I psionic/tests/mammoth_reference_fixture.py --check-cpu "$out/result.log"
python3 -I psionic/tests/mammoth_reference_fixture.py --check-output "$out/result.log"
