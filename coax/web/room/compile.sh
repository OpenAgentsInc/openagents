#!/bin/sh
# Firm reason: 5 (a thin wrapper: the C compiler's steps for the room binary)
# Compiles the staged room sources (stage.sh's OUT/src, run from that directory) into OUT, the
# `room` binary, on Debian bookworm's gcc: Cloud Run's image (Dockerfile) and the VM (vm-deploy.sh)
# run the same steps.
#   cd OUT/src && sh compile.sh /usr/local/bin/room
# Every unit at -O2: the tick runs the city's step, town and spells for every room 30 times a second (#12717).
# Units that load the same Coax module each carry its #export_c procedures: one copy is the program's,
# the city's units' in order, then the room's; each later copy is made local to its unit (build.sh).
# The simulation's floats as the browser's local host computes them (build.sh STRICT, #12893): no fused
# a * b + c, and the C library's sines, exponentials and powers left to room-math.c.
set -eu
out="${1:?usage: sh compile.sh OUT}"
STRICT="-ffp-contract=off -fno-fast-math -fno-builtin-sin -fno-builtin-sinf -fno-builtin-cos -fno-builtin-cosf -fno-builtin-tan -fno-builtin-tanf -fno-builtin-exp -fno-builtin-expf -fno-builtin-log -fno-builtin-logf -fno-builtin-pow -fno-builtin-powf"
gcc -std=c11 -O2 $STRICT -w -c room.c -o room.o &
for n in everglade everglade-step grid river guard-guide-policy; do gcc -std=c11 -O2 $STRICT -w -c $n.c -o $n.o & done
wait
: > seen.syms
for n in everglade everglade-step grid river guard-guide-policy room; do
  nm -g --defined-only $n.o | awk '{print $3}' | sort -u > $n.syms; comm -12 $n.syms seen.syms > $n.dup
  if [ -s $n.dup ]; then objcopy --localize-symbols=$n.dup $n.o $n-own.o; else cp $n.o $n-own.o; fi
  sort -u seen.syms $n.syms -o seen.syms
done
gcc -std=c11 -O2 $STRICT -w -I. -c room-world.c -o room-world.o
gcc -std=c11 -O2 $STRICT -w -c room-math.c -o room-math.o
gcc -std=c11 -O2 -w -c room-catalog.c -o room-catalog.o
gcc -std=c11 -O2 $STRICT -D_DEFAULT_SOURCE -Wall -I. -o "$out" room-host.c ws.c room-world.o room-catalog.o room-math.o room-own.o everglade-own.o everglade-step-own.o grid-own.o river-own.o guard-guide-policy-own.o -lm
