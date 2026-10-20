# Replace only the wallet assets in a previously verified website image.
ARG WEB_BASE
FROM rust:1.97.1-slim-trixie AS wallet
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y \
    --no-install-recommends build-essential pkg-config clang llvm curl ca-certificates git \
    && rm -rf /var/lib/apt/lists/*
COPY scripts/deploy/oa-kache.sh /usr/local/bin/oa-kache
RUN oa-kache install
WORKDIR /build
COPY Cargo.lock /build/Cargo.lock
RUN rustup target add wasm32-unknown-unknown \
    && version="$(awk '$0 == "name = \"wasm-bindgen\"" { getline; gsub(/"/, "", $3); print $3; exit }' Cargo.lock)" \
    && test -n "$version" \
    && cargo install --locked wasm-bindgen-cli --version "$version"
ENV CC_wasm32_unknown_unknown=/usr/bin/clang \
    AR_wasm32_unknown_unknown=/usr/bin/llvm-ar
COPY . .
RUN --network=host --mount=type=cache,target=/var/cache/kache,sharing=private \
    --mount=type=cache,target=/var/cache/openagents-wallet-cargo,sharing=locked \
    CARGO_TARGET_DIR=/var/cache/openagents-wallet-cargo oa-kache run scripts/build-spark-wallet-web.sh /build/wallet
FROM ${WEB_BASE}
COPY --from=wallet /build/wallet /srv/wallet
ENV OPENAGENTS_WALLET_WEB=/srv/wallet
