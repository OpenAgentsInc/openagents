# syntax=docker/dockerfile:1.7

ARG RUST_VERSION=1.90
ARG DEBIAN_VERSION=bookworm

FROM rust:${RUST_VERSION}-${DEBIAN_VERSION} AS builder
WORKDIR /src

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p oa-node && \
    cp /src/target/release/oa-node /usr/local/bin/oa-node

FROM debian:${DEBIAN_VERSION}-slim AS runtime

ARG IMAGE_CREATED=unknown
ARG IMAGE_REVISION=unknown
ARG IMAGE_VERSION=dev

LABEL org.opencontainers.image.title="OpenAgents oa-node" \
      org.opencontainers.image.description="Managed OpenAgents Cloud node daemon scaffold" \
      org.opencontainers.image.vendor="OpenAgents" \
      org.opencontainers.image.source="https://github.com/OpenAgentsInc/openagents" \
      org.opencontainers.image.created="${IMAGE_CREATED}" \
      org.opencontainers.image.revision="${IMAGE_REVISION}" \
      org.opencontainers.image.version="${IMAGE_VERSION}"

RUN useradd --create-home --home-dir /var/lib/openagents/oa-node --shell /usr/sbin/nologin oa-node

COPY --from=builder /usr/local/bin/oa-node /usr/local/bin/oa-node

ENV OPENAGENTS_CLOUD_NODE_HOME=/var/lib/openagents/oa-node

USER oa-node:oa-node
WORKDIR /var/lib/openagents/oa-node

ENTRYPOINT ["/usr/local/bin/oa-node"]
CMD ["status", "--json"]
