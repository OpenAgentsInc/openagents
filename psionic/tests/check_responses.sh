#!/bin/sh
# Focused changed-code verification. No original CPU inference or mutation roots.
set -eu
cd "$(dirname "$0")/../.."
COAX=${COAX:-coax/bin/coax}
PYTHON=${RESPONSES_PYTHON:-python3}
OUT=${RESPONSES_BUILD:-$(mktemp -d /tmp/psionic-responses.XXXXXX)}
mkdir -p "$OUT"
for mode in debug release; do
    flags=""
    if [ "$mode" = release ]; then flags="--release"; fi
    "$COAX" test psionic/lib/responses/fixture-host.coax $flags --run-steps 2000000 > "$OUT/checks-$mode.log"
    tail -n 1 "$OUT/checks-$mode.log"
    "$COAX" c psionic/lib/responses/fixture-host.coax --library $flags --header "$OUT/device-host.h" --run-steps 2000000 > "$OUT/fixture.c"
    for sanitizer in plain asan ubsan; do
        sanitize=""
        case "$sanitizer" in asan) sanitize="-fsanitize=address -fno-omit-frame-pointer";; ubsan) sanitize="-fsanitize=undefined -fno-sanitize-recover=all";; esac
        "${COAX_CC:-cc}" -std=c11 -O1 -g -ffp-contract=off -fno-fast-math -pthread $sanitize -DPSIONIC_RESPONSES_FIXTURE -I"$OUT" psionic/platform/responses-server.c "$OUT/fixture.c" -lm -o "$OUT/responses-$mode-$sanitizer"
        "$PYTHON" psionic/tests/responses_api.py --binary "$OUT/responses-$mode-$sanitizer" --table psionic/fixtures/harmony/gptoss.hst
    done
done
"$PYTHON" psionic/tests/book.py
printf 'Focused Responses checks passed; artifacts: %s\n' "$OUT"
