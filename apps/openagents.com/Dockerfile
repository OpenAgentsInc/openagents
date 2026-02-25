# syntax=docker/dockerfile:1

# Rust-only production image for apps/openagents.com.
# Builds:
# - openagents control service binary (apps/openagents.com/service)
FROM rust:bookworm AS builder

WORKDIR /workspace

RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates curl pkg-config libssl-dev \
  && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY proto ./proto
COPY crates ./crates
COPY apps ./apps

RUN cargo build --release --manifest-path apps/openagents.com/service/Cargo.toml

FROM debian:bookworm-slim AS runtime

WORKDIR /app

RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates \
  && rm -rf /var/lib/apt/lists/* \
  && useradd --system --create-home --uid 10001 openagents

COPY --from=builder /workspace/target/release/openagents-control-service /usr/local/bin/openagents-control-service
COPY --from=builder /workspace/apps/openagents.com/service/static /app/service/static

ENV OA_CONTROL_BIND_ADDR=0.0.0.0:8080
ENV OA_CONTROL_STATIC_DIR=/app/service/static

EXPOSE 8080

USER openagents

CMD ["openagents-control-service"]
