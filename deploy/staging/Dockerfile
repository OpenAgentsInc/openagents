# The staging stack's second image: the gateway (account service and
# inference gateway) and the chat worker, run as sidecars beside the
# openagents-web image in the Cloud Run service openagents-web-1-staging.
# Built from the repository root by deploy/staging/cloudbuild.yaml; the
# context is what deploy/staging/stack.gcloudignore lets through.
FROM rust:1.97.1-slim-trixie AS builder

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y \
        --no-install-recommends build-essential pkg-config \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY . .
RUN cargo build --locked --release -p gateway --bin gateway \
        -p coder --bin coder-worker -p tenancy --example bootstrap_registry \
    && strip target/release/gateway target/release/coder-worker \
        target/release/examples/bootstrap_registry

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
