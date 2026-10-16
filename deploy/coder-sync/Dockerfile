FROM rust:1.98.1-slim-trixie AS build
WORKDIR /app/deploy/coder-sync
COPY crates/coder-sync/ /app/crates/coder-sync/
COPY deploy/coder-sync/Cargo.toml deploy/coder-sync/Cargo.lock deploy/coder-sync/lib.rs /app/deploy/coder-sync/
# The image compiles only the opt-in server, using its checked-in lockfile.
RUN cargo build --release --locked --features server

FROM debian:trixie-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /app/deploy/coder-sync/target/release/coder-sync /usr/local/bin/coder-sync
USER 65532:65532
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/coder-sync"]
