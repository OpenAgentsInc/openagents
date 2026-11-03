# The room server on Cloud Run (coax/web/room/deploy.sh stages src/: the Coax units already turned
# into C, room.c and the everglade world's (everglade.c, everglade-step.c, grid.c, river.c,
# guard-guide-policy.c), with room-host.c, room-world.c and verse/voice-host/ws.c).
FROM debian:bookworm-slim AS build
RUN apt-get update && apt-get install -y --no-install-recommends gcc libc6-dev binutils && rm -rf /var/lib/apt/lists/*
COPY src/ /src/
WORKDIR /src
# Every unit at -O2: the tick runs the city's step, town and spells for every room 30 times a second (#12717).
# The room's unit and the city's both carry analytics.coax's exports: the room's copies are made local.
RUN gcc -std=c11 -O2 -w -c room.c -o room.o \
 && for n in everglade everglade-step grid river guard-guide-policy; do gcc -std=c11 -O2 -w -c $n.c -o $n.o & done; wait \
 && nm -g --defined-only room.o | awk '{print $3}' | sort > room.syms \
 && nm -g --defined-only everglade.o | awk '{print $3}' | sort > city.syms \
 && comm -12 room.syms city.syms > both.syms \
 && objcopy --localize-symbols=both.syms room.o room-own.o \
 && gcc -std=c11 -O2 -w -I. -c room-world.c -o room-world.o \
 && gcc -std=c11 -O2 -w -c room-catalog.c -o room-catalog.o \
 && gcc -std=c11 -O2 -D_DEFAULT_SOURCE -Wall -I. -o /room room-host.c ws.c room-world.o room-catalog.o room-own.o everglade.o everglade-step.o grid.o river.o guard-guide-policy.o -lm

FROM debian:bookworm-slim
# curl: the room posts players' HP to the API (room-host.c vitals_post, #12715)
RUN apt-get update && apt-get install -y --no-install-recommends curl ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /room /usr/local/bin/room
USER nobody
CMD ["room"]
