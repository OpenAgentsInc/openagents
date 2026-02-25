# syntax=docker/dockerfile:1.7

FROM rust:bookworm AS builder

WORKDIR /workspace

# Control service is a workspace member and depends on shared workspace crates.
COPY Cargo.toml Cargo.lock ./
COPY proto ./proto
COPY crates ./crates
COPY apps ./apps

RUN cargo build --release \
  --manifest-path apps/openagents.com/service/Cargo.toml \
  --bin openagents-control-service

FROM debian:bookworm-slim AS runtime

WORKDIR /app/service

RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates \
  && rm -rf /var/lib/apt/lists/*

COPY --from=builder /workspace/target/release/openagents-control-service /usr/local/bin/openagents-control-service
COPY --from=builder /workspace/apps/openagents.com/service/static ./static

ENV OA_CONTROL_STATIC_DIR=/app/service/static

EXPOSE 8787

ENTRYPOINT ["/usr/local/bin/openagents-control-service"]
