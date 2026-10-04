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

# The Everglade web build (#10524) that /everglade serves (#10525): the
# wasm32 target, wasm-bindgen-cli at the wasm-bindgen version Cargo.lock
# pins (the CLI refuses a module built by another version), then
# scripts/build-everglade-web.sh, which writes the JS glue and the wasm into
# the directory it's given. The pinned pack goes under pack/.
FROM rust:1.97.1-slim-trixie AS everglade

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y \
        --no-install-recommends build-essential pkg-config \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY Cargo.lock /build/Cargo.lock
RUN rustup target add wasm32-unknown-unknown \
    && version="$(awk '$0 == "name = \"wasm-bindgen\"" { getline; gsub(/"/, "", $3); print $3; exit }' Cargo.lock)" \
    && test -n "$version" \
    && cargo install --locked wasm-bindgen-cli --version "$version"
COPY . .
RUN scripts/build-everglade-web.sh /build/everglade \
    && test -f /build/everglade/everglade_web.js \
    && test -f /build/everglade/everglade_web_bg.wasm \
    && mkdir -p /build/everglade/pack \
    && cp assets/verse/everglade/*.vtp /build/everglade/pack/

FROM debian:13-slim

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y \
        --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --uid 10001 --user-group --create-home \
        --home-dir /home/web --shell /usr/sbin/nologin web
COPY --from=builder /build/target/release/openagents-web /usr/local/bin/openagents-web
COPY --from=everglade /build/everglade /srv/everglade

USER 10001:10001
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/openagents-web"]
CMD ["--listen", "0.0.0.0:8080", "--public-host", "openagents.com", "--everglade", "/srv/everglade"]
