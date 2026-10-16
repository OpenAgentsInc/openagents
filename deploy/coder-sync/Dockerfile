ARG CACHE_IMAGE=rust:1.98.1-slim-trixie
FROM ${CACHE_IMAGE} AS cargo-cache
# Also works with an empty Rust image on the first build.
RUN mkdir -p /usr/local/cargo/registry /usr/local/cargo/git /app/deploy/coder-sync/target

FROM rust:1.98.1-slim-trixie AS build
WORKDIR /app/deploy/coder-sync
COPY --from=cargo-cache /usr/local/cargo/registry/ /usr/local/cargo/registry/
COPY --from=cargo-cache /usr/local/cargo/git/ /usr/local/cargo/git/
COPY --from=cargo-cache /app/deploy/coder-sync/target/ /app/deploy/coder-sync/target/
COPY crates/coder-sync/ /app/crates/coder-sync/
COPY deploy/coder-sync/ /app/deploy/coder-sync/
# The image compiles only the opt-in server, using its checked-in lockfile.
RUN cargo build --release --locked --features server

FROM debian:trixie-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /app/deploy/coder-sync/target/release/coder-sync /usr/local/bin/coder-sync
USER 65532:65532
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/coder-sync"]
