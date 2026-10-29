#!/usr/bin/env bash
# Original files are supplied by the caller; this script does not download them.
set -euo pipefail
cd "$(dirname "$0")/../.."
if [[ $# != 1 && $# != 2 ]]; then echo "usage: psionic/tests/run_elephant_quality.sh ORIGINAL_DIRECTORY [WORKERS]" >&2; exit 2; fi
original=$1
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-quality.XXXXXX")
trap 'rm -rf "$out"' EXIT
python3 -I psionic/tests/quality_corpus.py
python3 -I psionic/tests/quality_corpus.py --header > "$out/quality-corpus.h"
python3 -I psionic/tests/elephant_manifest.py --header > "$out/elephant-inventory.h"
for unit in decision-session decision-prepare decision-answer decision-json; do
    "$coax" c "psionic/lib/$unit.coax" --library --release --header "$out/$unit.h" > /dev/null
done
"$coax" c psionic/tests/elephant-quality-runtime.coax --library --release --header "$out/elephant-quality-runtime.h" > "$out/runtime.c"
"$cc" -std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror \
    -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -I"$out" -Ipsionic/platform \
    "$out/runtime.c" psionic/platform/decision-session-memory.c psionic/platform/decision-prepare-memory.c \
    psionic/platform/decision-answer-memory.c psionic/platform/decision-json-memory.c \
    psionic/tests/elephant_quality.c -lm -o "$out/elephant-quality"
"$out/elephant-quality" "$original" psionic/fixtures/text/ch05/qwen35.pst "${2:-1}"
