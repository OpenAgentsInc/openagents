# The room server on Cloud Run (coax/web/room/deploy.sh stages src/: the Coax units already turned
# into C, room.c and the everglade world's (everglade.c, everglade-step.c, grid.c, river.c,
# guard-guide-policy.c), with room-host.c (and the room-tick.c it includes), room-world.c, room-math.c and verse/voice-host/ws.c).
FROM debian:bookworm-slim AS build
RUN apt-get update && apt-get install -y --no-install-recommends gcc libc6-dev binutils && rm -rf /var/lib/apt/lists/*
COPY src/ /src/
WORKDIR /src
# Every unit at -O2: the tick runs the city's step, town and spells for every room 30 times a second (#12717).
# Units that load the same Coax module each carry its #export_c procedures: one copy is the program's,
# the city's units' in order, then the room's; each later copy is made local to its unit (build.sh).
# The simulation's floats as the browser's local host computes them (build.sh STRICT, #12893): no fused
# a * b + c, and the C library's sines, exponentials and powers left to room-math.c.
ARG STRICT="-ffp-contract=off -fno-fast-math -fno-builtin-sin -fno-builtin-sinf -fno-builtin-cos -fno-builtin-cosf -fno-builtin-tan -fno-builtin-tanf -fno-builtin-exp -fno-builtin-expf -fno-builtin-log -fno-builtin-logf -fno-builtin-pow -fno-builtin-powf"
RUN gcc -std=c11 -O2 $STRICT -w -c room.c -o room.o \
 && for n in everglade everglade-step grid river guard-guide-policy; do gcc -std=c11 -O2 $STRICT -w -c $n.c -o $n.o & done; wait \
 && : > seen.syms \
 && for n in everglade everglade-step grid river guard-guide-policy room; do \
      nm -g --defined-only $n.o | awk '{print $3}' | sort -u > $n.syms; comm -12 $n.syms seen.syms > $n.dup; \
      if [ -s $n.dup ]; then objcopy --localize-symbols=$n.dup $n.o $n-own.o; else cp $n.o $n-own.o; fi; sort -u seen.syms $n.syms -o seen.syms; done \
 && gcc -std=c11 -O2 $STRICT -w -I. -c room-world.c -o room-world.o \
 && gcc -std=c11 -O2 $STRICT -w -c room-math.c -o room-math.o \
 && gcc -std=c11 -O2 -w -c room-catalog.c -o room-catalog.o \
 && gcc -std=c11 -O2 $STRICT -D_DEFAULT_SOURCE -Wall -I. -o /room room-host.c ws.c room-world.o room-catalog.o room-math.o room-own.o everglade-own.o everglade-step-own.o grid-own.o river-own.o guard-guide-policy-own.o -lm

FROM debian:bookworm-slim
# curl: the room posts players' HP to the API (room-host.c vitals_post, #12715)
RUN apt-get update && apt-get install -y --no-install-recommends curl ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /room /usr/local/bin/room
USER nobody
CMD ["room"]
