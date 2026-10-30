ARG CONFIDENTIAL_IMAGE
FROM ${CONFIDENTIAL_IMAGE} AS confidential
FROM debian:bookworm-slim AS build
RUN apt-get update && apt-get install -y --no-install-recommends gcc libc6-dev libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY . .
RUN sha256sum -c SOURCES.sha256 >/dev/null && \
 gcc -std=c11 -O2 -ffp-contract=off -fno-fast-math fleet-watch.c fleet-watch-face.c -lm -o /fleet-watch && \
 gcc -std=c11 -O2 -ffp-contract=off -fno-fast-math fleet-watch-probe.c fleet-watch-probe-face.c -lm -o /fleet-watch-probe && \
 gcc -std=c11 -O1 -g -Wno-unused-function -Wno-unused-variable -Wno-unused-but-set-variable -Itext -Itext/platform \
 text/text-probe.c text/text-probe-entry.c text/pylon-host.c text/platform/socket-native.c text/platform/tls-native.c \
 text/platform/entropy-native.c text/platform/entropy-foreign.c -lssl -lcrypto -lm -lpthread -o /pylon-text-probe
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends curl jq ca-certificates coreutils libssl3 && rm -rf /var/lib/apt/lists/* && useradd --system --uid 10001 --no-create-home pylon-watch
COPY --from=build /fleet-watch /fleet-watch-probe /pylon-text-probe /app/
COPY --from=confidential /opt/oa-confidential/ /opt/oa-confidential/
COPY fleet-watch-run.sh /app/run.sh
COPY probe-gptoss.sh /app/probe-gptoss.sh
COPY SOURCE_COMMIT COAX_SHA256 SOURCES.sha256 /usr/share/pylon-watch/
USER 10001:10001
ENV PYLON_WATCH_DRY_RUN=1 \
    OA_ATT_ORACLE_BIN=/opt/oa-confidential/oa-att \
    SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt
ENTRYPOINT ["/bin/bash","/app/run.sh"]
