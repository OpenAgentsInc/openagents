#!/usr/bin/env bash
# Opt-in large pinned data download and original Coax execution. No upstream code.
set -euo pipefail
cd "$(dirname "$0")/../.."
mode=${1:-both}
numpy_python=${2:-python3}
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
case "$mode" in 20b|120b|both|speculation|tools) ;; *) echo "usage: $0 [20b|120b|both|speculation|tools] [numpy-python]" >&2; exit 2 ;; esac
if [[ ! -x "$coax" ]]; then coax/build.sh; fi
out=${GPTOSS_RESULTS:-$(mktemp -d "${TMPDIR:-/tmp}/psionic-gptoss-original.XXXXXX")}
mkdir -p "$out"
echo "retaining original results in $out"
"$coax" version > "$out/compiler.txt"
shasum -a 256 "$coax" > "$out/compiler.sha256"
"$cc" --version > "$out/host-compiler.txt"
uname -a > "$out/host.txt"
python3 -I psionic/tests/gptoss_fixture.py --header > "$out/gptoss-tiny.h"
"$coax" c psionic/lib/gptoss.coax --library --header "$out/gptoss.h" --release --run-steps 2000000 > "$out/gptoss.c"
shasum -a 256 "$out/gptoss.c" > "$out/generated.sha256"
"$cc" -std=c11 -O3 -g -ffp-contract=off -fno-fast-math -Wno-unknown-pragmas \
    -I"$out" -Ipsionic/platform "$out/gptoss.c" psionic/platform/gptoss-memory.c \
    psionic/tests/gptoss_native.c -lm -o "$out/native"
"$cc" -std=c11 -O3 -g -ffp-contract=off -fno-fast-math -Wno-unknown-pragmas \
    -I"$out" -Ipsionic/platform "$out/gptoss.c" psionic/platform/gptoss-memory.c \
    psionic/tests/gptoss_lab.c -lm -o "$out/lab"
python3 -I psionic/tests/gptoss_record.py "$out" "$coax"
cache="$HOME/.cache/psionic/gptoss-original"
small="$cache/20b/6cee5e81ee83917806bbde320786a8fb61efebee"
large="$cache/120b/b5c939de8f754692c1647ca79fbf85e8c1e70f8a"
if [[ $mode == speculation ]]; then
    python3 -I psionic/tests/gptoss_artifacts.py --download both
    "$out/lab" speculate "$small" "$large" "$out" | tee "$out/speculation.log"
elif [[ $mode == tools ]]; then
    python3 -I psionic/tests/gptoss_artifacts.py --download 20b
    "$out/lab" tools "$small" unused "$out" | tee "$out/tools.log"
else
    python3 -I psionic/tests/gptoss_artifacts.py --download "$mode"
    for model in 20b 120b; do
        if [[ $mode != both && $mode != "$model" ]]; then continue; fi
        target=0;root="$small"
        if [[ $model == 120b ]]; then target=1;root="$large"; fi
        mkdir -p "$out/$model" "$out/$model-generation"
        "$out/native" original "$target" "$root" "$out/$model" | tee "$out/$model.log"
        OPENBLAS_NUM_THREADS=2 "$numpy_python" -I psionic/tests/gptoss_reference.py \
            "$root" "$out/$model" "$out/$model/reference" | tee "$out/$model-reference.log"
        "$out/native" generate "$target" "$root" "$out/$model-generation" | tee "$out/$model-generation.log"
    done
fi
echo "original gate passed; retained results: $out"
