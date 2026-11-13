# The room server on Cloud Run (coax/web/room/deploy.sh stages src/ with stage.sh: the Coax units already turned
# into C, room.c and the controller's everglade-step.c, with room-host.c (and the room-tick.c and room-wt.c it includes), room-world.c, room-math.c,
# metaverse/voice-host/ws.c, and WebTransport's wt.c with the vendored QUIC stack (third_party/, #13176).
FROM debian:bookworm-slim AS build
RUN apt-get update && apt-get install -y --no-install-recommends gcc libc6-dev binutils libcurl4-openssl-dev && rm -rf /var/lib/apt/lists/*
COPY src/ /src/
WORKDIR /src
# compile.sh: the same steps the VM runs (vm-deploy.sh)
RUN sh compile.sh /room

FROM debian:bookworm-slim
# libcurl: the room posts players' HP to the API in process (room-host.c poster_run, #12715, #13300)
RUN apt-get update && apt-get install -y --no-install-recommends libcurl4 ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /room /usr/local/bin/room
COPY compat/ /usr/share/coax/compat/
# the content tables: loot, quests, dialogue, points (room-host.c content_load, #13294)
COPY content/ /usr/share/everglade/
USER nobody
CMD ["room"]
