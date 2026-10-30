FROM debian:bookworm-slim AS build
RUN apt-get update && apt-get install -y --no-install-recommends gcc libc6-dev libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY . .
RUN sha256sum -c SOURCES.sha256 >/dev/null && \
    gcc -std=c11 -O1 -g -Wno-unused-function -Wno-unused-variable -Wno-unused-but-set-variable -Dmain=coax_fleet_main -c fleet.c -o fleet.o && \
    gcc -std=c11 -O1 -g -I. -Iplatform fleet-host.c platform/socket-native.c platform/tls-native.c platform/entropy-native.c platform/entropy-foreign.c platform/process-native.c fleet.o -lssl -lcrypto -lm -lpthread -o /psionic-fleet
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends libssl3 ca-certificates && rm -rf /var/lib/apt/lists/* && useradd --system --uid 10001 --no-create-home fleet
COPY --from=build /psionic-fleet /usr/local/bin/psionic-fleet
COPY start.sh /usr/local/bin/start-fleet
COPY SOURCE_COMMIT COAX_SHA256 SOURCES.sha256 /usr/share/psionic-fleet/
USER 10001:10001
ENV SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt
ENTRYPOINT ["/bin/sh", "/usr/local/bin/start-fleet"]
