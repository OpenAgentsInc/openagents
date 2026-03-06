FROM rust:1.88-bookworm AS build

WORKDIR /work

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates pkg-config libssl-dev protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

COPY . .

RUN cargo build --locked --release -p nexus-relay

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --home-dir /nonexistent --shell /usr/sbin/nologin --uid 60000 nexus

COPY --from=build /work/target/release/nexus-relay /usr/local/bin/nexus-relay

USER nexus

ENTRYPOINT ["/usr/local/bin/nexus-relay"]
