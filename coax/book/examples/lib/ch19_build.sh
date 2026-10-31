#!/usr/bin/env bash
# Build ch17_glow.coax for both of its consumers, then run the C preview.
set -euo pipefail
cd "$(dirname "$0")"
coax=../../../bin/coax
out=${TMPDIR:-/tmp}/coax-glow
mkdir -p "$out"

# The CPU side: a header and an object file for C, C++ or Swift.
"$coax" header ch17_glow.coax -o "$out/ch17_glow.h"
"$coax" lib ch17_glow.coax --release -o "$out/ch17_glow.o"
${COAX_CC:-cc} -std=c11 -O2 -I"$out" -o "$out/preview" ch19_preview.c "$out/ch17_glow.o" -lm

# The GPU side: the same procedures as Metal, then the hand-written stages.
"$coax" metal ch17_glow.coax > "$out/Glow.metal"
cat ch17_stage.metal >> "$out/Glow.metal"

"$out/preview"
