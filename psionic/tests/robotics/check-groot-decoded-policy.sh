#!/usr/bin/env bash
# Firm reason5: thin build/effect wrapper; new composed output, existing adapter.
set -euo pipefail
weights="${1:?usage: check-groot-decoded-policy.sh NORMALIZED_ACTIONS.f32 NEW_OUTPUT_DIRECTORY}"
work="${2:?}";[[ ! -e "$work" ]] || exit 2;mkdir -p "$work"
coax="${COAX:-coax/bin/coax}";cc="${COAX_CC:-${CC:-cc}}"
"$coax" c psionic/lib/robotics/groot-embodiment.coax --library --release --header "$work/groot-embodiment.h" > "$work/adapter.c"
"$cc" -std=c11 -O2 -ffp-contract=off -fno-fast-math -I "$work" "$work/adapter.c" psionic/tests/robotics/groot_decode.c -lm -o "$work/decode"
"$work/decode" "$weights" "$work/decoded-actions.f32"
