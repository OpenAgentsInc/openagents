#!/usr/bin/env bash
# Time chapter 10's decision path on caller-supplied original files; downloads nothing.
# Usage: psionic/tests/bench_elephant.sh ORIGINAL_DIRECTORY [REPEAT [RUNS]]
set -euo pipefail
cd "$(dirname "$0")/../.."
if [[ $# -lt 1 || $# -gt 3 ]]; then echo "usage: psionic/tests/bench_elephant.sh ORIGINAL_DIRECTORY [REPEAT [RUNS]]" >&2; exit 2; fi
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-elephant-bench.XXXXXX")
trap 'rm -rf "$out"' EXIT
python3 -I psionic/tests/elephant_manifest.py --header > "$out/elephant-inventory.h"
for unit in decision-session decision-prepare decision-answer; do
    "$coax" c "psionic/lib/$unit.coax" --library --release --header "$out/$unit.h" > "$out/$unit.c"
done
"$cc" -std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror \
    -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -I"$out" -Ipsionic/platform \
    "$out/decision-answer.c" psionic/platform/decision-session-memory.c \
    psionic/platform/decision-prepare-memory.c psionic/platform/decision-answer-memory.c \
    psionic/tests/elephant_bench.c -lm -o "$out/elephant-bench"
"$out/elephant-bench" "$1" psionic/fixtures/text/ch05/qwen35.pst "${2:-1}" "${3:-1}"
