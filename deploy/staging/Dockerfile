# The staging stack's second image: the gateway (account service and
# inference gateway) and the chat worker, run as sidecars beside the
# openagents-web image in the Cloud Run service openagents-web-1-staging.
# Built from the repository root by deploy/staging/cloudbuild.yaml; the
# context is what deploy/staging/stack.gcloudignore lets through.
#
# The builder-deps stage builds the same command against the
# dependency-only skeleton (scripts/deploy/cargo-skeleton.sh), so the
# compiled dependencies are a layer keyed by the manifests and Cargo.lock
# alone, kept between builds in cloudbuild.yaml's registry cache (#12177).
# Keep its command identical to the builder's.
# Both run under the shared remote compile cache (#12176) through
# scripts/deploy/oa-kache.sh.
FROM rust:1.97.1-slim-trixie AS skeleton
RUN --mount=type=bind,target=/context \
    sh /context/scripts/deploy/cargo-skeleton.sh /context /skeleton

FROM rust:1.97.1-slim-trixie AS builder-deps

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y \
        --no-install-recommends build-essential pkg-config curl \
    && rm -rf /var/lib/apt/lists/*
COPY scripts/deploy/oa-kache.sh /usr/local/bin/oa-kache
RUN oa-kache install
WORKDIR /build
COPY --from=skeleton /skeleton/ ./
RUN --network=host --mount=type=cache,target=/var/cache/kache,sharing=private oa-kache run \
    cargo build --locked --release -p gateway --bin gateway \
        -p coder --bin coder-worker -p tenancy --example bootstrap_registry

FROM builder-deps AS builder
COPY . .
# Every source file is touched first, so no stand-in the deps stage
# compiled can look newer than the real crate.
RUN --network=host --mount=type=cache,target=/var/cache/kache,sharing=private find . -path ./target -prune -o -exec touch -c {} + \
    && oa-kache run cargo build --locked --release -p gateway --bin gateway \
        -p coder --bin coder-worker -p tenancy --example bootstrap_registry \
    && strip target/release/gateway target/release/coder-worker \
        target/release/examples/bootstrap_registry

# The dependency layer alone: cloudbuild.yaml builds this target first and
# exports it to the registry cache.
FROM scratch AS deps
COPY --from=builder-deps /build/Cargo.lock /builder-deps.lock

FROM debian:13-slim

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y \
        --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --uid 10001 --user-group --create-home \
        --home-dir /home/stack --shell /usr/sbin/nologin stack
COPY --from=builder /build/target/release/gateway /usr/local/bin/gateway
COPY --from=builder /build/target/release/coder-worker /usr/local/bin/coder-worker
COPY --from=builder /build/target/release/examples/bootstrap_registry /usr/local/bin/bootstrap_registry
COPY --from=builder /build/knowledge/openagents /srv/knowledge/openagents
COPY deploy/staging/gateway.sh deploy/staging/worker.sh /usr/local/bin/

USER 10001:10001
ENTRYPOINT ["/usr/local/bin/gateway.sh"]
