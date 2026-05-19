# syntax=docker/dockerfile:1.7

FROM rust:1.88-bookworm AS build-base

ARG SCCACHE_VERSION=0.8.2
ARG SCCACHE_GCS_BUCKET=""

WORKDIR /work

ENV CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl pkg-config libssl-dev protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

RUN set -eux; \
    if [ -z "${SCCACHE_GCS_BUCKET}" ]; then exit 0; fi; \
    arch="$(dpkg --print-architecture)"; \
    case "${arch}" in \
      amd64) sccache_arch="x86_64-unknown-linux-musl" ;; \
      arm64) sccache_arch="aarch64-unknown-linux-musl" ;; \
      *) echo "unsupported sccache architecture: ${arch}" >&2; exit 1 ;; \
    esac; \
    curl -fsSL "https://github.com/mozilla/sccache/releases/download/v${SCCACHE_VERSION}/sccache-v${SCCACHE_VERSION}-${sccache_arch}.tar.gz" \
      | tar -xz --strip-components=1 -C /usr/local/bin "sccache-v${SCCACHE_VERSION}-${sccache_arch}/sccache"; \
    chmod 755 /usr/local/bin/sccache

FROM build-base AS deps

ARG BUILD_PROFILE=fast-release
ARG SCCACHE_GCS_BUCKET=""
ARG SCCACHE_GCS_KEY_PREFIX="nexus-relay"

ENV SCCACHE_DIR=/var/cache/sccache \
    SCCACHE_NO_DAEMON=1 \
    SCCACHE_GCS_BUCKET=${SCCACHE_GCS_BUCKET} \
    SCCACHE_GCS_KEY_PREFIX=${SCCACHE_GCS_KEY_PREFIX}

COPY .nexus-build-plan/ .

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/var/cache/sccache \
    set -eux; \
    cargo fetch --locked

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/var/cache/sccache \
    set -eux; \
    if [ -n "${SCCACHE_GCS_BUCKET}" ]; then export RUSTC_WRAPPER=/usr/local/bin/sccache; fi; \
    cargo build --locked --profile "${BUILD_PROFILE}" -p nexus-relay -p nexus-control; \
    if [ -n "${SCCACHE_GCS_BUCKET}" ]; then sccache --show-stats || true; fi

FROM deps AS builder

ARG BUILD_PROFILE=fast-release
ARG SOURCE_REV=unknown
ARG SCCACHE_GCS_BUCKET=""
ARG SCCACHE_GCS_KEY_PREFIX="nexus-relay"

ENV SCCACHE_DIR=/var/cache/sccache \
    SCCACHE_NO_DAEMON=1 \
    SCCACHE_GCS_BUCKET=${SCCACHE_GCS_BUCKET} \
    SCCACHE_GCS_KEY_PREFIX=${SCCACHE_GCS_KEY_PREFIX}

COPY . .

# The deps layer builds placeholder local crates from .nexus-build-plan. After
# overlaying the full source tree, remove those local artifacts so Cargo cannot
# reuse a placeholder nexus-relay binary in the final image.
RUN set -eux; \
    cargo clean --profile "${BUILD_PROFILE}" \
      -p nexus-relay \
      -p nexus-control \
      -p nostr-rs-relay \
      -p nostr \
      -p nostr-client \
      -p openagents-kernel-core \
      -p openagents-kernel-proto \
      -p openagents-provider-substrate \
      -p openagents-validator-service \
      -p psionic-train-contract

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/var/cache/sccache \
    set -eux; \
    printf '%s\n' "${SOURCE_REV}" > /work/.nexus-source-rev; \
    if [ -n "${SCCACHE_GCS_BUCKET}" ]; then export RUSTC_WRAPPER=/usr/local/bin/sccache; fi; \
    cargo build --locked --profile "${BUILD_PROFILE}" -p nexus-relay -p nexus-control; \
    if [ -n "${SCCACHE_GCS_BUCKET}" ]; then sccache --show-stats || true; fi

FROM debian:bookworm-slim

ARG BUILD_PROFILE=fast-release

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --home-dir /nonexistent --shell /usr/sbin/nologin --uid 60000 nexus

COPY --from=builder /work/target/${BUILD_PROFILE}/nexus-relay /usr/local/bin/nexus-relay
COPY --from=builder /work/target/${BUILD_PROFILE}/nexus-control /usr/local/bin/nexus-control
COPY --from=builder /work/target/${BUILD_PROFILE}/nexus-health-agent /usr/local/bin/nexus-health-agent
COPY --from=builder /work/target/${BUILD_PROFILE}/nexus-health-agent-server /usr/local/bin/nexus-health-agent-server

USER nexus

ENTRYPOINT ["/usr/local/bin/nexus-relay"]
