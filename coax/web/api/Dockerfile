# The openagents.com API service (api.c): a static-ish C binary with libpq,
# libsecp256k1 and OpenSSL's libcrypto from Debian, and its migrations.
# Its Coax modules (NIP-98, caller, mesh, Anime Lab, seat and progress policies) arrive as C
# in gen/: deploy.sh compiles them with the commit's checked Coax compiler.
FROM debian:bookworm-slim AS build
RUN apt-get update && apt-get install -y --no-install-recommends gcc libc6-dev libpq-dev libsecp256k1-dev libssl-dev libcurl4-openssl-dev zlib1g-dev && rm -rf /var/lib/apt/lists/*
COPY src/ /src/
COPY gen/ /src/
RUN gcc -std=c11 -O2 -Wall -Wextra -D_DEFAULT_SOURCE -I/usr/include/postgresql -o /api /src/api.c /src/analytics.c /src/admin.c /src/player.c /src/rooms.c /src/npc.c /src/room.c /src/private.c /src/drops.c /src/villa.c /src/chat.c /src/realms.c /src/access.c /src/makers.c /src/inference.c /src/mesh.c /src/mesh-policy.generated.c /src/caller-policy.generated.c /src/nip98.generated.c /src/anime.c /src/anime-director-policy.generated.c /src/seats.c /src/seat-policy.generated.c /src/progress-policy.generated.c /src/nostr.c /src/email.c /src/jobs.c /src/relay.c /src/verse_packs.c -lpq -lsecp256k1 -lssl -lcrypto -lcurl -lz -lm

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends libpq5 libsecp256k1-1 libssl3 libcurl4 zlib1g ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /api /usr/local/bin/api
COPY migrations/ /migrations/
USER nobody
CMD ["api", "--migrations", "/migrations"]
