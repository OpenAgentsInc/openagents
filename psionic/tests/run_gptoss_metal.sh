#!/usr/bin/env bash
# gpt-oss-20b on an Apple GPU through Metal (psionic/platform/gptoss-metal.c).
# Without a Metal device the harness prints SKIP and exits 0; there is no CPU
# fallback. The original weights are the caller's; nothing is downloaded.
#   psionic/tests/run_gptoss_metal.sh                       tiny owned model only
#   psionic/tests/run_gptoss_metal.sh ORIGINAL_CACHE [REPEAT] [OUT_DIR]
# With ORIGINAL_CACHE: chapter 45's recorded CPU vectors under the frozen
# 0.0005 rule, two greedy Harmony prompts, the forced-token benchmark.
# PSIONIC_DEVICE_QUALIFY=1 repeats layers 0 and 1 and the readout on the CPU;
# PSIONIC_DEVICE_PROFILE=1 times one prefill and 16 steps launch by launch;
# PSIONIC_CPU_GREEDY=1 also runs chapter 45's CPU engine greedily on both
# prompts (minutes per prompt) and requires the same token IDs.
set -euo pipefail
cd "$(dirname "$0")/../.."
if [[ $(uname) != Darwin ]]; then echo "SKIP: Metal needs macOS"; exit 0; fi
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-gptoss-metal.XXXXXX")
trap 'rm -rf "$out"' EXIT
python3 -I psionic/tests/gptoss_fixture.py --header > "$out/gptoss-tiny.h"
"$coax" c psionic/lib/gptoss/device-host.coax --library --release --run-steps 2000000 --header "$out/device-host.h" > "$out/device-host.c"
python3 -I psionic/tests/gptoss_device_bindings.py "$out/device-host.h" > "$out/gptoss-device-cpu.h"
"$coax" metal-compute psionic/lib/gptoss/device-kernels.coax --release > "$out/device-kernels.metal"
# Every kernel the plan names is in the Metal source.
python3 -I - "$out/device-kernels.metal" <<'EOF'
import re,sys
plan=open('psionic/lib/gptoss/device-plan.coax').read();metal=open(sys.argv[1]).read()
names=[n for n in re.findall(r'"(gk_[a-z0-9_]*)"',re.search(r'GD_NAMES :: .*?;',plan,re.S)[0])]
missing=[n for n in names if f'kernel void {n}(' not in metal];assert len(names)==45 and not missing,missing
print(f'coax metal-compute admits all {len(names)} gpt-oss device kernels')
EOF
"$cc" -fobjc-arc -mmacosx-version-min=15.0 -O2 -Wall -Wextra -Werror -c psionic/platform/metal-host.m -o "$out/metal-host.o"
"$cc" -std=c11 -O2 -g -ffp-contract=off -fno-fast-math -DPSIONIC_DEVICE_TEST -I"$out" -Ipsionic/platform \
    "$out/device-host.c" psionic/platform/gptoss-metal.c psionic/tests/gptoss_metal.c "$out/metal-host.o" \
    -framework Metal -framework Foundation -lm -o "$out/gptoss-metal"
"$out/gptoss-metal" tiny "$out/device-kernels.metal" psionic/fixtures/gptoss/tiny/model.safetensors | tee "$out/tiny.log"
if grep -q '^SKIP' "$out/tiny.log" || [[ $# -lt 1 ]]; then exit 0; fi
results=${3:-$out/original}
mkdir -p "$results"
"$out/gptoss-metal" original "$out/device-kernels.metal" "$1" "$results" "${2:-5}" | tee "$results/run.log"
# Frozen before the device was measured (chapter 45b's rule, unchanged).
python3 -I psionic/tests/gptoss_device_oracle.py 20b "$results" --profile gpt-oss-metal-f32-ff-v1
if [[ ${PSIONIC_CPU_GREEDY:-0} == 1 ]]; then
    "$coax" c psionic/lib/gptoss.coax --library --header "$out/gptoss.h" --release --run-steps 2000000 > "$out/gptoss.c"
    "$cc" -std=c11 -O3 -ffp-contract=off -fno-fast-math -Wno-unknown-pragmas -I"$out" -Ipsionic/platform \
        "$out/gptoss.c" psionic/platform/gptoss-memory.c psionic/tests/gptoss_cpu_greedy.c -lm -o "$out/cpu-greedy"
    for label in hi sum; do
        "$out/cpu-greedy" "$1" "$results/$label.prompt.i64" "$results/$label.cpu.i64" | tee "$results/$label.cpu.log"
        cmp "$results/$label.generated.i64" "$results/$label.cpu.i64"
        echo "greedy $label: the GPU's token IDs equal chapter 45's CPU engine's"
    done
fi
