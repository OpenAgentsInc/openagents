# Build the Rust adapter for SPACE's public intermediate-key protocol.
FROM rust:1.97.1-slim-trixie AS builder
RUN apt-get update && apt-get install -y --no-install-recommends protobuf-compiler && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY . .
RUN cargo build --release --locked -p space-key-broker --bin space-key-broker && strip target/release/space-key-broker

FROM debian:13-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/* && useradd --uid 10001 --user-group --no-create-home --shell /usr/sbin/nologin space-key-broker
COPY --from=builder /build/target/release/space-key-broker /usr/local/bin/space-key-broker
USER 10001:10001
ENTRYPOINT ["/usr/local/bin/space-key-broker"]
