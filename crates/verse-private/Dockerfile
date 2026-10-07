# The verse-assets broker (crates/verse-private, docs/verse/private-assets.md).
# Built from the repository root by crates/verse-private/cloudbuild.yaml.
# It holds no key: the service account comes from the metadata server.
FROM rust:1.97.1-slim-trixie AS builder

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y \
        --no-install-recommends build-essential pkg-config \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY . .
RUN cargo build --locked --release -p verse-private --features broker --bin verse-assets \
    && strip target/release/verse-assets

FROM debian:13-slim

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y \
        --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --uid 10001 --user-group --no-create-home \
        --shell /usr/sbin/nologin broker
COPY --from=builder /build/target/release/verse-assets /usr/local/bin/verse-assets

USER 10001:10001
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/verse-assets"]
