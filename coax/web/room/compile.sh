#!/bin/sh
# Firm reason: 5 (a thin wrapper: the C compiler's steps for the room binary)
# Compiles the staged room sources (stage.sh's OUT/src, run from that directory) into OUT, the
# `room` binary, on Debian bookworm's gcc: Cloud Run's image (Dockerfile) and the VM (vm-deploy.sh)
# run the same steps.
#   cd OUT/src && sh compile.sh /usr/local/bin/room
# Every unit at -O2: the tick runs the controller's step for every seat 30 times a second (#12717).
# Units that load the same Coax module each carry its #export_c procedures: one copy is the program's,
# the controller's unit, then the room's; each later copy is made local to its unit (build.sh).
# The simulation's floats as the browser's local host computes them (build.sh STRICT, #12893): no fused
# a * b + c, and the C library's sines, exponentials and powers left to room-math.c.
set -eu
out="${1:?usage: sh compile.sh OUT}"
# -g1 (#13298): each unit's functions and lines, so a crash's addresses resolve to Coax files and lines
# (stage.sh keeps the #line of each statement); the code is the same bytes as without it. The build's own
# folder is recorded as ".", so a line reads coax/web/room/room.coax:412 on any machine.
DBG="-g1 -fdebug-prefix-map=$(pwd)=."
STRICT="-ffp-contract=off -fno-fast-math -fno-builtin-sin -fno-builtin-sinf -fno-builtin-cos -fno-builtin-cosf -fno-builtin-tan -fno-builtin-tanf -fno-builtin-exp -fno-builtin-expf -fno-builtin-log -fno-builtin-logf -fno-builtin-pow -fno-builtin-powf"
gcc -std=c11 -O2 $DBG $STRICT -w -c room.c -o room.o &
for n in everglade-step; do gcc -std=c11 -O2 $DBG $STRICT -w -c $n.c -o $n.o & done
wait
: > seen.syms
for n in everglade-step room; do
  nm -g --defined-only $n.o | awk '{print $3}' | sort -u > $n.syms; comm -12 $n.syms seen.syms > $n.dup
  if [ -s $n.dup ]; then objcopy --localize-symbols=$n.dup $n.o $n-own.o; else cp $n.o $n-own.o; fi
  sort -u seen.syms $n.syms -o seen.syms
done
gcc -std=c11 -O2 $DBG $STRICT -w -I. -c room-world.c -o room-world.o
gcc -std=c11 -O2 $DBG $STRICT -w -c room-math.c -o room-math.o
gcc -std=c11 -O2 $DBG -Wall -c room-crash.c -o room-crash.o   # crash capture (#13298)
# WebTransport (#13176): the vendored QUIC stack (third_party/build.sh, into quic/libquic.a) and wt.c
CC=gcc bash third_party/build.sh quic
gcc -std=c11 -O2 -Wall -D_DEFAULT_SOURCE -DNGTCP2_STATICLIB -DNGHTTP3_STATICLIB -Ithird_party/ngtcp2/lib/includes -Ithird_party/ngtcp2/crypto/includes -Ithird_party/nghttp3/lib/includes -Ithird_party/picotls/include $DBG -c wt.c -o wt.o
# --build-id: the name the release's symbols are kept under (vm-deploy.sh symbols)
# OA_ROOM_SEED_TICK_US: a seeded bad build for a canary's own check (vm-deploy.sh deploy --staging --seed-tick-us, #13299)
SEED="${OA_ROOM_SEED_TICK_US:+-DROOM_SEEDED_TICK_US=$OA_ROOM_SEED_TICK_US}"
gcc -std=c11 -O2 $DBG $STRICT $SEED -D_DEFAULT_SOURCE -Wall -I. -Wl,--build-id=sha1 -o "$out" room-host.c ws.c room-world.o room-math.o room-crash.o wt.o quic/libquic.a everglade-step-own.o room-own.o -lm -lcurl
