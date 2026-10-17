ARG CACHE_IMAGE=rust:1.98.1-slim-trixie
FROM ${CACHE_IMAGE} AS cargo-cache
# Also works with an empty Rust image on the first build.
RUN mkdir -p /usr/local/cargo/registry /usr/local/cargo/git /app/deploy/coder-sync/target

FROM rust:1.98.1-slim-trixie AS build
WORKDIR /app/deploy/coder-sync
COPY --from=cargo-cache /usr/local/cargo/registry/ /usr/local/cargo/registry/
COPY --from=cargo-cache /usr/local/cargo/git/ /usr/local/cargo/git/
COPY --from=cargo-cache /app/deploy/coder-sync/target/ /app/deploy/coder-sync/target/
# The server's own source, the SDK it signs in with, the push gateway's wire
# types, and the sync clients from
# the OpenAgents apps (apps/openagents). The two workspace manifests are
# needed for inherited package fields. Keep in step with scripts/coder-sync/deploy.py.
COPY Cargo.toml /app/Cargo.toml
COPY crates/coder-sync/ /app/crates/coder-sync/
COPY crates/openagents-sdk/ /app/crates/openagents-sdk/
COPY crates/openagents-login/ /app/crates/openagents-login/
COPY crates/agent-contract/ /app/crates/agent-contract/
COPY crates/agent-lab/ /app/crates/agent-lab/
COPY crates/lab-kit/ /app/crates/lab-kit/
COPY crates/openagents-dashboard/ /app/crates/openagents-dashboard/
COPY crates/nostr/ /app/crates/nostr/
COPY crates/nostr-transport/ /app/crates/nostr-transport/
COPY crates/push-gateway/ /app/crates/push-gateway/
COPY apps/openagents/Cargo.toml /app/apps/openagents/Cargo.toml
COPY apps/openagents/crates/brand/ /app/apps/openagents/crates/brand/
COPY apps/openagents/crates/client/ /app/apps/openagents/crates/client/
COPY apps/openagents/crates/doc/ /app/apps/openagents/crates/doc/
COPY apps/openagents/crates/orb/ /app/apps/openagents/crates/orb/
COPY apps/openagents/crates/proto/ /app/apps/openagents/crates/proto/
COPY apps/openagents/crates/rpc/ /app/apps/openagents/crates/rpc/
COPY apps/openagents/crates/sync/ /app/apps/openagents/crates/sync/
COPY apps/openagents/crates/voice-session/ /app/apps/openagents/crates/voice-session/
COPY deploy/coder-sync/ /app/deploy/coder-sync/
# The image compiles only the opt-in server, using its checked-in lockfile.
RUN cargo build --release --locked --features server

FROM debian:trixie-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /app/deploy/coder-sync/target/release/coder-sync /usr/local/bin/coder-sync
USER 65532:65532
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/coder-sync"]
