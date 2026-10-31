# The Orbit League room server on Cloud Run (coax/web/room/deploy.sh stages
# src/: room.c and room.h, the Coax room already turned into C, with
# room-host.c and verse/voice-host/ws.c and ws.h).
FROM debian:bookworm-slim AS build
RUN apt-get update && apt-get install -y --no-install-recommends gcc libc6-dev && rm -rf /var/lib/apt/lists/*
COPY src/ /src/
WORKDIR /src
RUN gcc -std=c11 -O2 -w -c room.c -o room.o \
 && gcc -std=c11 -O2 -D_DEFAULT_SOURCE -Wall -I. -o /room room-host.c ws.c room.o -lm

FROM debian:bookworm-slim
COPY --from=build /room /usr/local/bin/room
USER nobody
CMD ["room"]
