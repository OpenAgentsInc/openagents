# syntax=docker/dockerfile:1.7

ARG RUST_VERSION=1.90
ARG DEBIAN_VERSION=bookworm

FROM rust:${RUST_VERSION}-${DEBIAN_VERSION} AS builder
WORKDIR /src

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p oa-cloud-run-bridge && \
    cp /src/target/release/oa-cloud-run-bridge /usr/local/bin/oa-cloud-run-bridge

FROM debian:${DEBIAN_VERSION}-slim AS runtime

ARG IMAGE_CREATED=unknown
ARG IMAGE_REVISION=unknown
ARG IMAGE_VERSION=dev

LABEL org.opencontainers.image.title="OpenAgents oa-cloud-run-bridge" \
      org.opencontainers.image.description="Narrow bearer-token-gated HTTPS reverse proxy from the public Cloud Run edge to the internal oa-codex-control node (openagents#8503)" \
      org.opencontainers.image.vendor="OpenAgents" \
      org.opencontainers.image.source="https://github.com/OpenAgentsInc/openagents" \
      org.opencontainers.image.created="${IMAGE_CREATED}" \
      org.opencontainers.image.revision="${IMAGE_REVISION}" \
      org.opencontainers.image.version="${IMAGE_VERSION}"

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --create-home --home-dir /var/lib/openagents/oa-cloud-run-bridge --shell /usr/sbin/nologin oa-cloud-run-bridge

COPY --from=builder /usr/local/bin/oa-cloud-run-bridge /usr/local/bin/oa-cloud-run-bridge

# Cloud Run sets PORT at runtime; the binary binds 0.0.0.0:$PORT by default.
# OA_BRIDGE_CONTROL_TOKEN and OA_BRIDGE_CONTROL_URL are supplied at deploy
# time from Secret Manager / --set-env-vars — never baked into the image.
USER oa-cloud-run-bridge:oa-cloud-run-bridge
WORKDIR /var/lib/openagents/oa-cloud-run-bridge

ENTRYPOINT ["/usr/local/bin/oa-cloud-run-bridge"]
