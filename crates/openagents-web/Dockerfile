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
# the directory it's given. The pinned pack goes under pack/, and the
# medieval kit pack, when cloudbuild.yaml's first step fetched it from the
# private bucket into crates/openagents-web/kit/, under kit/.
FROM rust:1.97.1-slim-trixie AS everglade

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y \
        --no-install-recommends build-essential pkg-config clang llvm binaryen \
    && rm -rf /var/lib/apt/lists/*
# secp256k1's C needs a clang with the WebAssembly target, which Debian's
# has; `llvm` supplies `llvm-ar` beside it and `binaryen` supplies `wasm-opt`.
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
    && gzip -9 -k /build/everglade/everglade_web.js /build/everglade/everglade_web_bg.wasm \
    && mkdir -p /build/everglade/pack /build/everglade/kit/bake \
    && cp assets/verse/everglade/*.vtp /build/everglade/pack/ \
    && if [ -d crates/openagents-web/kit ]; then \
        find crates/openagents-web/kit -maxdepth 1 -name '*.vtp' \
            -exec cp {} /build/everglade/kit/ \;; fi \
    && if [ -d crates/openagents-web/kit/bake ]; then \
        find crates/openagents-web/kit/bake -maxdepth 1 -name '*.vlay' \
            -exec cp {} /build/everglade/kit/bake/ \;; fi

# Build the shared component catalog, the Cloud privacy adapter, and the
# granted workbench terminal (both served from /cloud/assets/ out of the
# --cloud-build directory) directly in this isolated stage. The local build scripts use the machine's build lease.
RUN cargo build --locked --release --target wasm32-unknown-unknown \
        -p coder-components-web -p coder-cloud-web -p coder-browser-web \
        -p coder-chat-web \
    && mkdir -p /build/components /build/cloud /build/chat \
    && wasm-bindgen --target web --no-typescript --out-dir /build/components \
        target/wasm32-unknown-unknown/release/coder_components_web.wasm \
    && wasm-bindgen --target web --no-typescript --out-dir /build/cloud \
        target/wasm32-unknown-unknown/release/coder_cloud_web.wasm \
    && wasm-bindgen --target web --no-typescript --out-dir /build/cloud \
        target/wasm32-unknown-unknown/release/coder_browser_web.wasm \
    && wasm-bindgen --target web --no-typescript --out-dir /build/chat \
        target/wasm32-unknown-unknown/release/coder_chat_web.wasm \
    && test -s /build/chat/coder_chat_web.js \
    && test -s /build/chat/coder_chat_web_bg.wasm \
    && test -s /build/components/coder_components_web.js \
    && test -s /build/components/coder_components_web_bg.wasm \
    && test -s /build/cloud/coder_cloud_web.js \
    && test -s /build/cloud/coder_cloud_web_bg.wasm \
    && test -s /build/cloud/coder_browser_web.js \
    && test -s /build/cloud/coder_browser_web_bg.wasm

# Grow Little Bunny, which /games/grow-little-bunny serves: the glue, the
# module, the start script, and gzip copies of each.
RUN scripts/build-bunny-web.sh /build/bunny \
    && test -s /build/bunny/bunny_web.js \
    && test -s /build/bunny/bunny_web_bg.wasm \
    && test -s /build/bunny/start.js

# The sealed-inference demo, which /att serves (crates/att-web): the glue,
# the module, the start script, the stylesheet, and gzip copies of each.
RUN scripts/build-att-web.sh /build/att \
    && test -s /build/att/att_web.js \
    && test -s /build/att/att_web_bg.wasm \
    && test -s /build/att/start.js \
    && test -s /build/att/att.css

FROM debian:13-slim

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y \
        --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --uid 10001 --user-group --create-home \
        --home-dir /home/web --shell /usr/sbin/nologin web
COPY --from=builder /build/target/release/openagents-web /usr/local/bin/openagents-web
COPY --from=everglade /build/everglade /srv/everglade
COPY --from=everglade /build/components /srv/components
COPY --from=everglade /build/cloud /srv/cloud
COPY --from=everglade /build/chat /srv/chat
COPY --from=everglade /build/bunny /srv/bunny
COPY --from=everglade /build/att /srv/att

USER 10001:10001
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/openagents-web"]
CMD ["--listen", "0.0.0.0:8080", "--public-host", "openagents.com", "--everglade", "/srv/everglade", "--components-build", "/srv/components", "--cloud-build", "/srv/cloud", "--chat-build", "/srv/chat", "--bunny", "/srv/bunny"]
