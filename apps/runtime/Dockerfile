# syntax=docker/dockerfile:1.7

FROM rust:bookworm AS builder

WORKDIR /workspace

COPY Cargo.toml Cargo.lock ./
COPY apps/runtime apps/runtime
COPY proto proto

RUN cargo build --release \
  --manifest-path apps/runtime/Cargo.toml \
  --bin openagents-runtime-service \
  --bin runtime-migrate \
  --bin runtime-smoke

FROM debian:bookworm-slim AS runtime

WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /workspace/target/release/openagents-runtime-service /usr/local/bin/openagents-runtime-service
COPY --from=builder /workspace/target/release/runtime-migrate /usr/local/bin/runtime-migrate
COPY --from=builder /workspace/target/release/runtime-smoke /usr/local/bin/runtime-smoke

EXPOSE 4100

ENTRYPOINT ["/usr/local/bin/openagents-runtime-service"]
