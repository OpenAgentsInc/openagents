#!/usr/bin/env bash
# Firm reason: 5 (thin compiler wrapper; every check and decision is Coax).
set -euo pipefail
out=$1
shift
mkdir -p "$out"
"${COAX:-coax/bin/coax}" c psionic/tests/guard-guide/policy-native.coax --library "$@" --header "$out/guide.h" > "$out/guide.c"
"${COAX_CC:-cc}" -std=c11 -O1 -g -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer -Icoax/platform "$out/guide.c" coax/platform/tool-native.c -lm -lpthread -o "$out/check"
exec "$out/check"
