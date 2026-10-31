#!/usr/bin/env bash
# Firm reason: Thin compiler wrapper; corpus, CLI, scoring and checks are Coax.
set -euo pipefail
out=$1
shift
mkdir -p "$out"
"${COAX:-coax/bin/coax}" c psionic/tests/guard-live/tool.coax --library "$@" --header "$out/tool.h" > "$out/tool.c"
"${COAX_CC:-cc}" -std=c11 -O1 -g -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer -I"$out" "$out/tool.c" psionic/tests/guard-live/effects.c -lm -o "$out/guard-evaluation"
