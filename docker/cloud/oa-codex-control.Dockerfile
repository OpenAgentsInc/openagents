# syntax=docker/dockerfile:1.7

ARG RUST_VERSION=1.90
ARG DEBIAN_VERSION=bookworm
ARG NODE_VERSION=24.13.1

FROM rust:${RUST_VERSION}-${DEBIAN_VERSION} AS builder
WORKDIR /src

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p oa-codex-control && \
    cp /src/target/release/oa-codex-control /usr/local/bin/oa-codex-control

FROM node:${NODE_VERSION}-${DEBIAN_VERSION}-slim AS runtime

ARG IMAGE_CREATED=unknown
ARG IMAGE_REVISION=unknown
ARG IMAGE_VERSION=dev
ARG NODE_VERSION=24.13.1

LABEL org.opencontainers.image.title="OpenAgents oa-codex-control" \
      org.opencontainers.image.description="Managed OpenAgents Cloud Codex control daemon (cloud#95 always-on control node)" \
      org.opencontainers.image.vendor="OpenAgents" \
      org.opencontainers.image.source="https://github.com/OpenAgentsInc/openagents" \
      org.opencontainers.image.created="${IMAGE_CREATED}" \
      org.opencontainers.image.revision="${IMAGE_REVISION}" \
      org.opencontainers.image.version="${IMAGE_VERSION}"

# The live GCE per-session provisioner shells out to the `gcloud` CLI using the
# in-VM Application Default Credentials (the instance service account). The
# control daemon also drives git over HTTPS for repo checkout/writeback once a
# write grant is resolved, so git + ca-certificates are required at runtime.
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      ca-certificates \
      curl \
      git \
      iproute2 \
      iptables \
      openssh-client \
      procps \
      python3 \
      apt-transport-https \
      gnupg \
 && echo "deb [signed-by=/usr/share/keyrings/cloud.google.gpg] https://packages.cloud.google.com/apt cloud-sdk main" \
      > /etc/apt/sources.list.d/google-cloud-sdk.list \
 && curl -fsSL https://packages.cloud.google.com/apt/doc/apt-key.gpg \
      | gpg --dearmor -o /usr/share/keyrings/cloud.google.gpg \
 && apt-get update \
 && apt-get install -y --no-install-recommends google-cloud-cli \
 && command -v gcloud \
 && test "$(node --version)" = "v${NODE_VERSION}" \
 && command -v ssh \
 && command -v scp \
 && command -v ssh-keygen \
 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/local/bin/oa-codex-control /usr/local/bin/oa-codex-control
COPY scripts/cloud/managed-sandbox-provider-proxy.py /usr/local/bin/managed-sandbox-provider-proxy.py
COPY scripts/cloud/managed-sandbox-control-entrypoint.sh /usr/local/bin/managed-sandbox-control-entrypoint.sh
COPY scripts/cloud/managed-sandbox-io-driver.mjs /usr/local/bin/managed-sandbox-io-driver.mjs
COPY scripts/cloud/managed-sandbox-turn-driver.mjs /usr/local/bin/managed-sandbox-turn-driver.mjs
COPY scripts/cloud/managed-sandbox-phase2-driver.mjs /usr/local/bin/managed-sandbox-phase2-driver.mjs
RUN chmod 0755 \
      /usr/local/bin/managed-sandbox-provider-proxy.py \
      /usr/local/bin/managed-sandbox-control-entrypoint.sh \
      /usr/local/bin/managed-sandbox-io-driver.mjs \
      /usr/local/bin/managed-sandbox-turn-driver.mjs \
      /usr/local/bin/managed-sandbox-phase2-driver.mjs

# State root for the durable local job registry (job.json / events.jsonl).
ENV OA_CODEX_CONTROL_STATE_ROOT=/var/lib/openagents/codex-control
RUN install -d -m 0750 /var/lib/openagents/codex-control

# Bind on all interfaces inside the VM; the GCE firewall restricts the source.
ENV OA_CODEX_CONTROL_BIND=0.0.0.0:8787

ENTRYPOINT ["/usr/local/bin/managed-sandbox-control-entrypoint.sh"]
