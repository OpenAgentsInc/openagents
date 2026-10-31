# The openagents.com API service (api.c): a static-ish C binary with libpq,
# libsecp256k1 and OpenSSL's libcrypto from Debian, and its migrations.
# Its Coax modules (nip98.coax, mesh-policy.coax) are compiled here from
# source: the first stage bootstraps the Coax compiler from its own sources
# (coax/build.sh, unchanged, with its checks) and emits the modules' C,
# which the service is built from (the committed *.generated.c caches are
# for builds without a compiler).
FROM debian:bookworm-slim AS coax
RUN apt-get update && apt-get install -y --no-install-recommends gcc g++ libc6-dev perl ca-certificates && rm -rf /var/lib/apt/lists/*
COPY coax/ /work/coax/
COPY scripts/ /work/scripts/
WORKDIR /work
RUN COAX_RELEASE_CACHE=/tmp/coax-cache COAX_MUTATE_JOBS="$(nproc)" coax/build.sh
RUN mkdir /out && for m in nip98 mesh-policy; do \
      coax/bin/coax c coax/web/api/$m.coax --run-steps 250000000 --library --header /out/$m.generated.h > /out/$m.generated.c || exit 1; \
    done

FROM debian:bookworm-slim AS build
RUN apt-get update && apt-get install -y --no-install-recommends gcc libc6-dev libpq-dev libsecp256k1-dev libssl-dev libcurl4-openssl-dev zlib1g-dev && rm -rf /var/lib/apt/lists/*
COPY src/ /src/
COPY --from=coax /out/ /src/
RUN gcc -std=c11 -O2 -Wall -Wextra -D_DEFAULT_SOURCE -I/usr/include/postgresql -o /api /src/api.c /src/analytics.c /src/admin.c /src/player.c /src/npc.c /src/room.c /src/private.c /src/drops.c /src/villa.c /src/chat.c /src/realms.c /src/replies.c /src/makers.c /src/inference.c /src/mesh.c /src/mesh-policy.generated.c /src/nip98.generated.c /src/nostr.c /src/email.c /src/jobs.c /src/relay.c -lpq -lsecp256k1 -lssl -lcrypto -lcurl -lz -lm

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends libpq5 libsecp256k1-1 libssl3 libcurl4 zlib1g ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /api /usr/local/bin/api
COPY migrations/ /migrations/
USER nobody
CMD ["api", "--migrations", "/migrations"]
