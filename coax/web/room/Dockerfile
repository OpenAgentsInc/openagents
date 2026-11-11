# The room server on Cloud Run (coax/web/room/deploy.sh stages src/ with stage.sh: the Coax units already turned
# into C, room.c and the everglade world's (everglade.c, everglade-step.c, grid.c, river.c,
# guard-guide-policy.c), with room-host.c (and the room-tick.c and room-wt.c it includes), room-world.c, room-math.c,
# verse/voice-host/ws.c, and WebTransport's wt.c with the vendored QUIC stack (third_party/, #13176).
FROM debian:bookworm-slim AS build
RUN apt-get update && apt-get install -y --no-install-recommends gcc libc6-dev binutils && rm -rf /var/lib/apt/lists/*
COPY src/ /src/
WORKDIR /src
# compile.sh: the same steps the VM runs (vm-deploy.sh)
RUN sh compile.sh /room

FROM debian:bookworm-slim
# curl: the room posts players' HP to the API (room-host.c vitals_post, #12715)
RUN apt-get update && apt-get install -y --no-install-recommends curl ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /room /usr/local/bin/room
COPY compat/ /usr/share/coax/compat/
USER nobody
CMD ["room"]
