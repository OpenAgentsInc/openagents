# syntax=docker/dockerfile:1

# Rust-only production image for apps/openagents.com.
# Builds:
# - openagents control service binary (apps/openagents.com/service)
# - web-shell static dist (apps/openagents.com/web-shell/dist)
FROM rust:bookworm AS builder

WORKDIR /workspace

RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates curl pkg-config libssl-dev python3 \
  && rm -rf /var/lib/apt/lists/*

RUN rustup target add wasm32-unknown-unknown
RUN curl -sSf https://rustwasm.github.io/wasm-pack/installer/init.sh | sh

COPY Cargo.toml Cargo.lock ./
COPY proto ./proto
COPY crates ./crates
COPY apps/openagents.com/service ./apps/openagents.com/service
COPY apps/openagents.com/web-shell ./apps/openagents.com/web-shell

RUN apps/openagents.com/web-shell/build-dist.sh
RUN cargo build --release --manifest-path apps/openagents.com/service/Cargo.toml

FROM debian:bookworm-slim AS runtime

WORKDIR /app

RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates \
  && rm -rf /var/lib/apt/lists/* \
  && useradd --system --create-home --uid 10001 openagents

COPY --from=builder /workspace/target/release/openagents-control-service /usr/local/bin/openagents-control-service
COPY --from=builder /workspace/apps/openagents.com/web-shell/dist /app/web-shell/dist

ENV OA_CONTROL_BIND_ADDR=0.0.0.0:8080
ENV OA_CONTROL_STATIC_DIR=/app/web-shell/dist

EXPOSE 8080

USER openagents

CMD ["openagents-control-service"]
