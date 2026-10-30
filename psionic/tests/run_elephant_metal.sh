#!/usr/bin/env bash
# Chapter 31: the original Elephant decision on an Apple GPU through Metal.
# The caller supplies the original files; nothing is downloaded. Without a
# Metal device the harness prints SKIP and exits 0. Usage:
#   psionic/tests/run_elephant_metal.sh ORIGINAL_DIRECTORY [REPEAT]
# PSIONIC_DEVICE_PROFILE=1 adds one run timed launch by launch;
# PSIONIC_DEVICE_QUALIFY=1 repeats layers 0 and 3 and the readout on the CPU.
set -euo pipefail
cd "$(dirname "$0")/../.."
if [[ $# -lt 1 || $# -gt 2 ]]; then echo "usage: psionic/tests/run_elephant_metal.sh ORIGINAL_DIRECTORY [REPEAT]" >&2; exit 2; fi
if [[ $(uname) != Darwin ]]; then echo "SKIP: Metal needs macOS"; exit 0; fi
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-elephant-metal.XXXXXX")
trap 'rm -rf "$out"' EXIT
python3 -I psionic/tests/elephant_manifest.py --header > "$out/elephant-inventory.h"
python3 -I psionic/tests/elephant_reference_fixture.py --header > "$out/elephant-request.h"
for unit in decision-session decision-prepare decision-answer; do
    "$coax" c "psionic/lib/$unit.coax" --library --release --header "$out/$unit.h" > "$out/$unit.c"
done
"$coax" c psionic/lib/device/plan.coax --library --release --header "$out/plan.h" > "$out/plan.c"
for unit in project layer warp; do "$coax" metal-compute "psionic/lib/device/$unit.coax" --release > "$out/$unit.metal"; done
flags=(-std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unknown-pragmas -Wno-unused-function
    -Wno-unused-parameter -Wno-missing-field-initializers -I"$out" -Ipsionic/platform)
"$cc" -fobjc-arc -mmacosx-version-min=15.0 -O2 -Wall -Wextra -Werror -c psionic/platform/metal-host.m -o "$out/metal-host.o"
"$cc" "${flags[@]}" "$out/decision-answer.c" "$out/plan.c" psionic/platform/decision-session-memory.c psionic/platform/decision-prepare-memory.c \
    psionic/platform/decision-answer-memory.c psionic/platform/elephant-metal.c psionic/tests/elephant_metal.c "$out/metal-host.o" \
    -framework Metal -framework Foundation -lm -o "$out/elephant-metal"
"$out/elephant-metal" "$1" psionic/fixtures/text/ch05/qwen35.pst "$out/project.metal" "$out/layer.metal" "$out/warp.metal" "${2:-3}" | tee "$out/result.log"
if grep -q '^SKIP' "$out/result.log"; then exit 0; fi
# Frozen before the device was measured (chapter 30's rule, unchanged): 0.0005
# absolute on each decision statistic, against the independent reference and
# the recorded CPU result.
python3 -I psionic/tests/elephant_reference_fixture.py --check-output "$out/result.log"
python3 -I psionic/tests/elephant_reference_fixture.py --check-cpu "$out/result.log"
