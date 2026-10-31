#!/usr/bin/env bash
# Firm reason5: compiler/native tool wrapper; all policy is Coax.
set -euo pipefail
work="${1:?new output directory required}"
mkdir -p "$work"
coax="${COAX:-coax/bin/coax}"
"$coax" c psionic/lib/robotics/panda-loop.coax --library --header "$work/panda-loop.h" > "$work/panda-loop.c"
"${CC:-cc}" -std=c11 -O0 -g -ffp-contract=off -I"$work" "$work/panda-loop.c" psionic/tests/robotics/panda_loop_native.c -lm -o "$work/debug"
"$work/debug"
"$coax" c psionic/lib/robotics/panda-loop.coax --release --library --header "$work/panda-loop.h" > "$work/panda-loop.c"
"${CC:-cc}" -std=c11 -O2 -ffp-contract=off -fno-fast-math -I"$work" "$work/panda-loop.c" psionic/tests/robotics/panda_loop_native.c -lm -o "$work/release"
"$work/release"
"${CC:-cc}" -std=c11 -O1 -g -ffp-contract=off -fsanitize=address,undefined -I"$work" "$work/panda-loop.c" psionic/tests/robotics/panda_loop_native.c -lm -o "$work/sanitize"
"$work/sanitize"
