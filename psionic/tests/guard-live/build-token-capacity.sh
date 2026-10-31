#!/usr/bin/env bash
# Firm reason: thin compiler wrapper (5); all analysis and input policy are Coax.
set -euo pipefail
mkdir -p "$1"
"${COAX:-coax/bin/coax}" c psionic/tests/guard-live/token-capacity.coax --library --header "$1/token-capacity.h" > "$1/token-capacity.c"
"${COAX_CC:-cc}" -std=c11 -O2 -ffp-contract=off -fno-fast-math -I"$1" "$1/token-capacity.c" psionic/tests/guard-live/token-capacity-host.c -lm -o "$1/guard-token-capacity"
