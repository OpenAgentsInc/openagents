#!/usr/bin/env bash
# Firm reason: 5 (thin compiler wrapper); C is the independent oracle (2).
set -euo pipefail
out=$1
mkdir -p "$out"
"${COAX:-coax/bin/coax}" c psionic/tests/guard-guide-session/native.coax --library --header "$out/session.h" > "$out/debug.c"
"${COAX_CC:-cc}" -std=c11 -O0 -g -I"$out" "$out/debug.c" psionic/tests/guard-guide-session/native.c -lm -o "$out/debug"
"$out/debug"
"${COAX:-coax/bin/coax}" c psionic/tests/guard-guide-session/native.coax --library --release --header "$out/session.h" > "$out/release.c"
"${COAX_CC:-cc}" -std=c11 -O2 -I"$out" "$out/release.c" psionic/tests/guard-guide-session/native.c -lm -o "$out/release"
"$out/release"
"${COAX_CC:-cc}" -std=c11 -O1 -g -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer -I"$out" "$out/debug.c" psionic/tests/guard-guide-session/native.c -lm -o "$out/sanitized"
"$out/sanitized"
