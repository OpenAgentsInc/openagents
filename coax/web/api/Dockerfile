# The openagents.com API service (api.c): a static-ish C binary with libpq,
# libsecp256k1 and OpenSSL's libcrypto from Debian, and its migrations.
FROM debian:bookworm-slim AS build
RUN apt-get update && apt-get install -y --no-install-recommends gcc libc6-dev libpq-dev libsecp256k1-dev libssl-dev libcurl4-openssl-dev && rm -rf /var/lib/apt/lists/*
COPY src/ /src/
RUN gcc -std=c11 -O2 -Wall -Wextra -D_DEFAULT_SOURCE -I/usr/include/postgresql -o /api /src/api.c /src/analytics.c /src/admin.c /src/player.c /src/npc.c /src/room.c /src/private.c /src/drops.c /src/villa.c /src/chat.c /src/realms.c /src/inference.c /src/nostr.c /src/relay.c -lpq -lsecp256k1 -lssl -lcrypto -lcurl -lm

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends libpq5 libsecp256k1-1 libssl3 libcurl4 ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /api /usr/local/bin/api
COPY migrations/ /migrations/
USER nobody
CMD ["api", "--migrations", "/migrations"]
