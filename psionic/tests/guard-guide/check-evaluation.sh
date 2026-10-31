#!/usr/bin/env bash
# Firm reason: thin compiler wrapper (5); C is an independent oracle (2).
set -euo pipefail
mkdir -p "$1"
"${COAX:-coax/bin/coax}" c verse/everglade/guard-guide-eval-wire.coax --library --header "$1/guide-eval.h" > "$1/eval-debug.c"
"${CC:-cc}" -std=c11 -O0 -g -ffp-contract=off -fno-fast-math -I"$1" "$1/eval-debug.c" psionic/tests/guard-guide/evaluation-native.c -lm -o "$1/debug"
"$1/debug"
"${COAX:-coax/bin/coax}" c verse/everglade/guard-guide-eval-wire.coax --library --release --header "$1/guide-eval.h" > "$1/eval-release.c"
"${CC:-cc}" -std=c11 -O2 -ffp-contract=off -fno-fast-math -I"$1" "$1/eval-release.c" psionic/tests/guard-guide/evaluation-native.c -lm -o "$1/release"
"$1/release"
"${CC:-cc}" -std=c11 -O1 -g -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer -ffp-contract=off -fno-fast-math -I"$1" "$1/eval-debug.c" psionic/tests/guard-guide/evaluation-native.c -lm -o "$1/sanitized"
"$1/sanitized"
