#!/usr/bin/env bash
# Build ch14_glow.coax for both of its consumers, then run the C preview.
set -euo pipefail
cd "$(dirname "$0")"
coax=../../../bin/coax
out=${TMPDIR:-/tmp}/coax-glow
mkdir -p "$out"

# The CPU side: a header and an object file for C, C++ or Swift.
"$coax" header ch14_glow.coax -o "$out/ch14_glow.h"
"$coax" lib ch14_glow.coax --release -o "$out/ch14_glow.o"
${COAX_CC:-cc} -std=c11 -O2 -I"$out" -o "$out/preview" ch16_preview.c "$out/ch14_glow.o" -lm

# The GPU side: the same procedures as Metal, then the hand-written stages.
"$coax" metal ch14_glow.coax > "$out/Glow.metal"
cat ch14_stage.metal >> "$out/Glow.metal"

"$out/preview"
