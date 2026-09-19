FROM rust:1.95.0-slim-trixie AS builder

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y \
        --no-install-recommends build-essential \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY migrations ./migrations
COPY crates ./crates
COPY nips ./nips
COPY tests ./tests
RUN cargo build --locked --release -p nostr-relay --bin nostr-relay \
    && strip target/release/nostr-relay

FROM debian:13-slim

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y \
        --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --uid 10001 --user-group --no-create-home \
        --home-dir /nonexistent --shell /usr/sbin/nologin nostr-relay
COPY --from=builder --chown=nostr-relay:nostr-relay \
    /build/target/release/nostr-relay /usr/local/bin/nostr-relay

USER 10001:10001
ENV NOSTR_RELAY_BIND_ADDR=0.0.0.0 \
    NOSTR_RELAY_PORT=8080
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/nostr-relay"]
