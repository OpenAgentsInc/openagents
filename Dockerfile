FROM rust:1.97.1-slim-trixie AS skeleton
RUN --mount=type=bind,target=/context sh /context/scripts/deploy/cargo-skeleton.sh /context /skeleton

FROM rust:1.97.1-slim-trixie AS builder-deps

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y \
        --no-install-recommends build-essential curl \
    && rm -rf /var/lib/apt/lists/*
COPY scripts/deploy/oa-kache.sh /usr/local/bin/oa-kache
RUN oa-kache install
WORKDIR /build
COPY --from=skeleton /skeleton/ ./
RUN --network=host --mount=type=cache,target=/var/cache/kache,sharing=private oa-kache run cargo build --locked --release -p nostr-relay --bin nostr-relay

FROM builder-deps AS builder
COPY Cargo.toml Cargo.lock ./
COPY migrations ./migrations
COPY crates ./crates
COPY nips ./nips
COPY tests ./tests
COPY vendor ./vendor
COPY apps/openagents/Cargo.toml ./apps/openagents/Cargo.toml
COPY apps/openagents/crates ./apps/openagents/crates
COPY questions ./questions
COPY docs/protocol/fixtures ./docs/protocol/fixtures
# Make real sources newer than the dependency skeleton.
RUN --network=host --mount=type=cache,target=/var/cache/kache,sharing=private find . -path ./target -prune -o -exec touch -c {} + \
    && oa-kache run cargo build --locked --release -p nostr-relay --bin nostr-relay \
    && strip target/release/nostr-relay

FROM scratch AS deps
COPY --from=builder-deps /build/Cargo.lock /build-deps.lock

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
