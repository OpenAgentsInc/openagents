# The openagents.com website (crates/openagents-web). Built from the
# repository root by crates/openagents-web/cloudbuild.yaml; the context is
# what crates/openagents-web/web.gcloudignore lets through.
FROM rust:1.97.1-slim-trixie AS builder

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y \
        --no-install-recommends build-essential pkg-config \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY . .
RUN cargo build --locked --release -p openagents-web --bin openagents-web \
    && strip target/release/openagents-web

FROM debian:13-slim

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y \
        --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --uid 10001 --user-group --create-home \
        --home-dir /home/web --shell /usr/sbin/nologin web
COPY --from=builder /build/target/release/openagents-web /usr/local/bin/openagents-web

USER 10001:10001
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/openagents-web"]
CMD ["--listen", "0.0.0.0:8080", "--public-host", "openagents.com"]
