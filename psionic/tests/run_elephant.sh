#!/usr/bin/env bash
# Original files are supplied by the caller; this script does not download them.
set -euo pipefail
cd "$(dirname "$0")/../.."
if [[ $# != 1 ]]; then echo "usage: psionic/tests/run_elephant.sh ORIGINAL_DIRECTORY" >&2; exit 2; fi
original=$1
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-elephant.XXXXXX")
trap 'rm -rf "$out"' EXIT
python3 -I psionic/tests/elephant_manifest.py --header > "$out/elephant-inventory.h"
python3 -I psionic/tests/elephant_reference_fixture.py --header > "$out/elephant-request.h"
for unit in decision-session decision-prepare decision-answer; do
    "$coax" c "psionic/lib/$unit.coax" --library --release --header "$out/$unit.h" > "$out/$unit.c"
done
"$cc" -std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror \
    -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -I"$out" -Ipsionic/platform \
    "$out/decision-answer.c" psionic/platform/decision-session-memory.c \
    psionic/platform/decision-prepare-memory.c psionic/platform/decision-answer-memory.c \
    psionic/tests/elephant_api.c -lm -o "$out/elephant-api"
"$out/elephant-api" "$original" psionic/fixtures/text/ch05/qwen35.pst | tee "$out/result.log"
python3 -I psionic/tests/elephant_reference_fixture.py --check-output "$out/result.log"
