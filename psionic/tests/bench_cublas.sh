#!/usr/bin/env bash
# Opt-in comparator (chapter 34): Psionic's projection kernels against cuBLAS
# on Elephant's shapes, under protocol cm-warm-v1 from psionic/lib/comparator.coax.
# cuBLAS is test-only and never part of check.sh, a library or a server; the
# harness skips when PSIONIC_CUBLAS (default ~/cublas-13.2) is absent.
#   psionic/tests/bench_cublas.sh            print the receipt
#   psionic/tests/bench_cublas.sh --record   also write psionic/fixtures/comparator/<host>.json
# The receipt names the commit, machine, driver, clocks and load average; it
# is never compared byte for byte.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
record=0
while [[ $# -gt 0 ]]; do case "$1" in --record) record=1; shift;; *) exit 2;; esac; done
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-cublas.XXXXXX")
trap 'rm -rf "$out"' EXIT
# Psionic's build identity: the git blob ids of every file the measured
# program is built from, which survive a rebase; the commit is recorded too.
sources=(psionic/lib/device/project.coax psionic/lib/comparator.coax psionic/tests/comparator-checks.coax psionic/tests/cublas_bench.c psionic/platform/cuda-host.c psionic/platform/cuda-host.h)
build=$(git hash-object "${sources[@]}" | paste -sd, -)
commit=$(git rev-parse HEAD)
"$coax" cuda psionic/lib/device/project.coax --release > "$out/project.cu"
"$coax" c psionic/lib/comparator.coax --release --library --header "$out/comparator.h" > "$out/comparator.c"
"$cc" -std=c11 -O2 -Wall -Wextra -Werror -Wno-missing-field-initializers -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas -ffp-contract=off -Ipsionic/platform -I"$out" \
    psionic/platform/cuda-host.c "$out/comparator.c" psionic/tests/cublas_bench.c -ldl -lm -o "$out/cublas-bench"
gpu() { nvidia-smi --query-gpu="$1" --format=csv,noheader 2>/dev/null | head -1 || echo unknown; }
{
    echo "{"
    echo "  \"schema\": \"psionic.comparator.v1\","
    echo "  \"host\": \"$(hostname -s)\","
    echo "  \"date\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\","
    echo "  \"commit\": \"$commit\","
    echo "  \"psionic_build\": \"$build\","
    echo "  \"compiler\": \"$("$coax" --version | head -1); $("$cc" --version | head -1)\","
    echo "  \"cpu\": \"$(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2- | sed 's/^ //')\","
    echo "  \"kernel\": \"$(uname -sr)\","
    echo "  \"driver\": \"$(gpu driver_version)\","
    echo "  \"clocks_before\": \"$(gpu clocks.sm,clocks.mem,clocks.max.sm,pstate,temperature.gpu,utilization.gpu)\","
    echo "  \"loadavg_before\": \"$(cat /proc/loadavg)\","
    LD_LIBRARY_PATH="${CUDA_HOME:-$HOME/cuda-13.2}/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" "$out/cublas-bench" "$out/project.cu" "$build"
    echo "  \"clocks_after\": \"$(gpu clocks.sm,clocks.mem,clocks.max.sm,pstate,temperature.gpu,utilization.gpu)\","
    echo "  \"loadavg_after\": \"$(cat /proc/loadavg)\""
    echo "}"
} > "$out/receipt.json"
if grep -q '^SKIP' "$out/receipt.json"; then grep '^SKIP' "$out/receipt.json"; exit 0; fi
python3 -I -c 'import json,sys; json.load(open(sys.argv[1]))' "$out/receipt.json"
cat "$out/receipt.json"
if [[ $record == 1 ]]; then mkdir -p psionic/fixtures/comparator; cp "$out/receipt.json" "psionic/fixtures/comparator/$(hostname -s).json"; fi
