# The coder-sync server image, built by deploy/coder-sync/cloudbuild.yaml
# from the source trees scripts/coder-sync/deploy.py exports.
#
# The build-deps stage builds the server's command against the
# dependency-only skeleton (scripts/deploy/cargo-skeleton.sh), so the
# compiled dependencies are a layer keyed by the manifests and lock files
# alone, kept between builds in a registry cache
# (scripts/deploy/cloud-buildx.sh, #12177). Keep its command identical to
# the build stage's. Both run under the shared remote compile
# cache (#12176) through scripts/deploy/oa-kache.sh.
FROM rust:1.98.1-slim-trixie AS skeleton
RUN --mount=type=bind,target=/context \
    sh /context/scripts/deploy/cargo-skeleton.sh /context /skeleton deploy/coder-sync

FROM rust:1.98.1-slim-trixie AS build-deps
RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends curl \
    && rm -rf /var/lib/apt/lists/*
COPY scripts/deploy/oa-kache.sh /usr/local/bin/oa-kache
RUN oa-kache install
COPY --from=skeleton /skeleton/ /app/
WORKDIR /app/deploy/coder-sync
# The image compiles only the opt-in server, using its checked-in lockfile.
RUN --network=host --mount=type=cache,target=/var/cache/kache,sharing=private oa-kache run cargo build --release --locked --features server

FROM build-deps AS build
# The server's own source, the SDK it signs in with, and the sync clients from
# the OpenAgents apps (apps/openagents). The two workspace manifests are
# needed for inherited package fields. Keep in step with scripts/coder-sync/deploy.py.
COPY Cargo.toml /app/Cargo.toml
# deploy.py exports only the local dependency trees needed by this workspace.
COPY crates/ /app/crates/
COPY support/workspace-hack/ /app/support/workspace-hack/
COPY nips/openagents/schemas/ /app/nips/openagents/schemas/
# jev embeds its decision questions from the repository root.
COPY questions/ /app/questions/
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
# Every source file is touched first, so no stand-in the deps stage
# compiled can look newer than the real crate.
RUN --network=host --mount=type=cache,target=/var/cache/kache,sharing=private find /app -path /app/deploy/coder-sync/target -prune -o -exec touch -c {} + \
    && oa-kache run cargo build --release --locked --features server

# The dependency layer alone: cloud-buildx.sh builds this target first and
# exports it to the registry cache.
FROM scratch AS deps
COPY --from=build-deps /app/deploy/coder-sync/Cargo.lock /build-deps.lock

FROM debian:trixie-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /app/deploy/coder-sync/target/release/coder-sync /usr/local/bin/coder-sync
USER 65532:65532
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/coder-sync"]
