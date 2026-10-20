# The openagents.com website (crates/openagents-web). Built from the
# repository root by crates/openagents-web/cloudbuild.yaml; the context is
# what crates/openagents-web/web.gcloudignore lets through.
#
# Each Rust stage first builds its exact `cargo build` commands against the
# dependency-only skeleton (scripts/deploy/cargo-skeleton.sh) in a `*-deps`
# stage, so the compiled dependencies are an image layer keyed by the
# manifests and Cargo.lock alone (#12177). cloudbuild.yaml keeps those
# layers in a registry cache between builds; a source-only change pulls
# them and compiles only the workspace's own crates. Keep each `*-deps`
# command identical to the build it stands in for (packages, target,
# profile, features, and environment), or the dependencies compile twice.
# Every Cargo command also runs under the shared remote compile cache
# (#12176) through scripts/deploy/oa-kache.sh, so a workspace crate whose
# inputs didn't change is downloaded rather than compiled.

# The skeleton, read from the context through a bind mount so the context
# itself never becomes a layer.
FROM rust:1.97.1-slim-trixie AS skeleton
RUN --mount=type=bind,target=/context \
    sh /context/scripts/deploy/cargo-skeleton.sh /context /skeleton

FROM rust:1.97.1-slim-trixie AS builder-deps

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y \
        --no-install-recommends build-essential pkg-config curl \
    && rm -rf /var/lib/apt/lists/*
COPY scripts/deploy/oa-kache.sh /usr/local/bin/oa-kache
RUN oa-kache install
WORKDIR /build
COPY --from=skeleton /skeleton/ ./
RUN --network=host --mount=type=cache,target=/var/cache/kache,sharing=private oa-kache run \
    cargo build --locked --release -p openagents-web --bin openagents-web --bin lab-cad-copy \
        -p cad-lab --bin cad-lab

FROM builder-deps AS builder
COPY . .
# The commit the image is built from, compiled in for /att/release.json and
# /vault/release.json (option_env!("OPENAGENTS_COMMIT")); "unknown" when the
# build names none.
ARG OPENAGENTS_COMMIT=unknown
ENV OPENAGENTS_COMMIT=${OPENAGENTS_COMMIT}
# The CAD engine (crates/cad-lab) beside it: the site runs CAD Lab's tools
# for signed-in people in a child process (crates/openagents-web/src/cad_web.rs,
# #11653). Every source file is touched first, so no stand-in the deps
# stage compiled can look newer than the real crate.
RUN --network=host --mount=type=cache,target=/var/cache/kache,sharing=private find . -path ./target -prune -o -exec touch -c {} + \
    && oa-kache run cargo build --locked --release -p openagents-web --bin openagents-web --bin lab-cad-copy \
        -p cad-lab --bin cad-lab \
    && strip target/release/openagents-web target/release/cad-lab target/release/lab-cad-copy

# The WebAssembly toolchain both browser stages share: clang with the
# WebAssembly target (secp256k1's C needs it), `llvm-ar`, `wasm-opt`
# (binaryen), brotli, the wasm32 target, and wasm-bindgen-cli at the version
# Cargo.lock pins (the CLI refuses a module built by another version).
FROM rust:1.97.1-slim-trixie AS wasm-base

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y \
        --no-install-recommends build-essential pkg-config clang llvm binaryen brotli curl \
    && rm -rf /var/lib/apt/lists/*
COPY scripts/deploy/oa-kache.sh /usr/local/bin/oa-kache
RUN oa-kache install
WORKDIR /build
COPY Cargo.lock /build/Cargo.lock
RUN rustup target add wasm32-unknown-unknown \
    && version="$(awk '$0 == "name = \"wasm-bindgen\"" { getline; gsub(/"/, "", $3); print $3; exit }' Cargo.lock)" \
    && test -n "$version" \
    && cargo install --locked wasm-bindgen-cli --version "$version"
# The C compiler and archiver for wasm32, named for every build in both
# stages. They are the ones the build scripts and the cc crate pick on their
# own; naming them keeps the dependencies' build scripts from rerunning
# between a script that sets them and a command that doesn't.
ENV CC_wasm32_unknown_unknown=/usr/bin/clang \
    AR_wasm32_unknown_unknown=/usr/bin/llvm-ar
RUN test -x "$CC_wasm32_unknown_unknown" && test -x "$AR_wasm32_unknown_unknown"

# The dependencies of every browser build in the everglade stage, one
# command each.
FROM wasm-base AS everglade-deps
COPY --from=skeleton /skeleton/ ./
RUN --network=host --mount=type=cache,target=/var/cache/kache,sharing=private oa-kache run sh -c '\
    w=wasm32-unknown-unknown \
    && cargo build --profile verse-web --locked --target $w -p everglade-web \
    && cargo build --locked --release --target $w \
        -p coder-components-web -p coder-cloud-web -p coder-browser-web \
        -p coder-chat-web -p oa-vault-web \
    && cargo build --release --locked --target $w -p bunny-web \
    && cargo build --release --locked --target $w -p icecream-web \
    && cargo build --locked -p robot-web --target $w --profile guest \
    && cargo build --release --locked --target $w -p snap-web \
    && cargo build --release --locked --target $w -p att-web \
    && cargo build --locked --release --target $w -p sand-garden \
    && cargo build --release --locked --target $w -p layertree-web \
    && cargo build --locked --profile wasm-tiny --target $w -p lander5-web -p pylons-web'

# The Everglade web build (#10524) that /everglade serves (#10525):
# scripts/build-everglade-web.sh writes the JS glue and the wasm into the
# directory it's given. The pinned pack goes under pack/, and the medieval
# kit pack, when cloudbuild.yaml's first step fetched it from the private
# bucket into crates/openagents-web/kit/, under kit/.
FROM everglade-deps AS everglade
COPY . .
RUN --network=host --mount=type=cache,target=/var/cache/kache,sharing=private find . -path ./target -prune -o -exec touch -c {} + \
    && oa-kache run scripts/build-everglade-web.sh /build/everglade \
    && test -f /build/everglade/everglade_web.js \
    && test -f /build/everglade/everglade_web_bg.wasm \
    && gzip -9 -k /build/everglade/everglade_web.js /build/everglade/everglade_web_bg.wasm \
    && brotli -q 11 -k /build/everglade/everglade_web.js /build/everglade/everglade_web_bg.wasm \
    && test -s /build/everglade/everglade_web_bg.wasm.br \
    && mkdir -p /build/everglade/pack /build/everglade/kit/bake /build/everglade/space \
    && cp assets/verse/everglade/*.vtp /build/everglade/pack/ \
    && cp crates/verse/assets/lagrange/*.png crates/verse/assets/lagrange/stars.bin /build/everglade/space/ \
    && if [ -d crates/openagents-web/kit ]; then \
        find crates/openagents-web/kit -maxdepth 1 -name '*.vtp' \
            -exec cp {} /build/everglade/kit/ \;; fi \
    && if [ -d crates/openagents-web/kit/bake ]; then \
        find crates/openagents-web/kit/bake -maxdepth 1 -name '*.vlay' \
            -exec cp {} /build/everglade/kit/bake/ \;; fi

# Build the shared component catalog, the Cloud privacy adapter, and the
# granted workbench terminal (both served from /cloud/assets/ out of the
# --cloud-build directory) directly in this isolated stage. The local build scripts use the machine's build lease.
RUN --network=host --mount=type=cache,target=/var/cache/kache,sharing=private oa-kache run cargo build --locked --release --target wasm32-unknown-unknown \
        -p coder-components-web -p coder-cloud-web -p coder-browser-web \
        -p coder-chat-web -p oa-vault-web \
    && mkdir -p /build/components /build/cloud /build/chat \
    && wasm-bindgen --target web --no-typescript --out-dir /build/components \
        target/wasm32-unknown-unknown/release/coder_components_web.wasm \
    && wasm-bindgen --target web --no-typescript --out-dir /build/cloud \
        target/wasm32-unknown-unknown/release/coder_cloud_web.wasm \
    && wasm-bindgen --target web --no-typescript --out-dir /build/cloud \
        target/wasm32-unknown-unknown/release/coder_browser_web.wasm \
    && wasm-bindgen --target web --no-typescript --out-dir /build/chat \
        target/wasm32-unknown-unknown/release/coder_chat_web.wasm \
    && wasm-bindgen --target web --no-typescript --out-dir /build/chat \
        target/wasm32-unknown-unknown/release/oa_vault_web.wasm \
    && test -s /build/chat/oa_vault_web.js \
    && test -s /build/chat/oa_vault_web_bg.wasm \
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
RUN --network=host --mount=type=cache,target=/var/cache/kache,sharing=private oa-kache run scripts/build-bunny-web.sh /build/bunny \
    && test -s /build/bunny/bunny_web.js \
    && test -s /build/bunny/bunny_web_bg.wasm \
    && test -s /build/bunny/start.js

# The Ice Cream Man, which /games/the-ice-cream-man serves the same way. The
# site finds it at /srv/icecream without a flag (production's arguments are
# copied from the live revision).
RUN --network=host --mount=type=cache,target=/var/cache/kache,sharing=private oa-kache run scripts/build-icecream-web.sh /build/icecream \
    && test -s /build/icecream/icecream_web.js \
    && test -s /build/icecream/icecream_web_bg.wasm \
    && test -s /build/icecream/start.js

# Robot Lab's Rust player for account-scoped CAD imports.
RUN --network=host --mount=type=cache,target=/var/cache/kache,sharing=private oa-kache run env CARGO_TARGET_DIR=/build/target scripts/build-robot-web.sh /build/robot \
    && test -s /build/robot/robot_web.js \
    && test -s /build/robot/robot_web_bg.wasm

# Snap Station, which /games/snap-station serves the same way, found at
# /srv/snap without a flag.
RUN --network=host --mount=type=cache,target=/var/cache/kache,sharing=private oa-kache run scripts/build-snap-web.sh /build/snap \
    && test -s /build/snap/snap_web.js \
    && test -s /build/snap/snap_web_bg.wasm \
    && test -s /build/snap/start.js \
    && test -s /build/snap/style.css

# The sealed-inference demo, which /att serves (crates/att-web): the glue,
# the module, the start script, the stylesheet, and gzip copies of each.
RUN --network=host --mount=type=cache,target=/var/cache/kache,sharing=private oa-kache run scripts/build-att-web.sh /build/att \
    && test -s /build/att/att_web.js \
    && test -s /build/att/att_web_bg.wasm \
    && test -s /build/att/start.js \
    && test -s /build/att/att.css

RUN --network=host --mount=type=cache,target=/var/cache/kache,sharing=private oa-kache run scripts/build-sand-garden.sh /build/sand

# The LayerTree demo (#11987), which /layertree serves the same way, found at
# /srv/layertree without a flag.
RUN --network=host --mount=type=cache,target=/var/cache/kache,sharing=private oa-kache run scripts/build-layertree-web.sh /build/layertree \
    && test -s /build/layertree/layertree_web.js \
    && test -s /build/layertree/layertree_web_bg.wasm \
    && test -s /build/layertree/start.js

# /lander5's WebGL2 runner, which the page inlines, found at /srv/lander5
# without a flag.
RUN --network=host --mount=type=cache,target=/var/cache/kache,sharing=private oa-kache run scripts/build-lander5-web.sh /build/lander5 \
    && test -s /build/lander5/lander5_web.js \
    && test -s /build/lander5/lander5_web_bg.wasm

# /pylons' Grid layer, found at /srv/pylons/layer without a flag.
RUN --network=host --mount=type=cache,target=/var/cache/kache,sharing=private oa-kache run scripts/build-pylons-web.sh /build/pylons-layer \
    && test -s /build/pylons-layer/pylons_web.js \
    && test -s /build/pylons-layer/pylons_web_bg.wasm.br

# The CAD engine for the browser (crates/cad-wasm), which /cad loads
# (#11649): its own stage, so it builds beside the others. wasm-opt
# (binaryen) shrinks it; brotli and gzip copies ride beside each file.
FROM wasm-base AS cadweb-deps
COPY --from=skeleton /skeleton/ ./
RUN --network=host --mount=type=cache,target=/var/cache/kache,sharing=private oa-kache run \
    cargo build --locked --profile cad-web --target wasm32-unknown-unknown -p cad-wasm

FROM cadweb-deps AS cadweb
COPY . .
RUN --network=host --mount=type=cache,target=/var/cache/kache,sharing=private find . -path ./target -prune -o -exec touch -c {} + \
    && oa-kache run scripts/build-cad-web.sh /build/cad \
    && test -s /build/cad/cad_wasm.js \
    && test -s /build/cad/cad_wasm_bg.wasm.br \
    && test -s /build/cad/worker.js \
    && test -s /build/cad/start.js \
    && test -s /build/cad/cad.css

# The desktop chat UI for the browser, which /demo/gpui serves,
# AgentGraph, which /agent/foreman draws with (and its first frame: the
# paint-only painter for wasm32, and Foreman's frames, captured by the
# native `first-frame-capture` on GPUI's headless platform, no GPU), and
# the GPUI web chat at / and /chat/{id}
# (apps/openagents/web, its own Cargo workspace and lock). GPUI needs the
# app's pinned compiler, 1.98.1, so this stage has its own base: the wasm32
# target, binaryen, brotli, and wasm-bindgen-cli at the version
# apps/openagents/web/Cargo.lock pins. The target directory is named so the
# deps stage and the build script share it.
FROM rust:1.98.1-slim-trixie AS gpuiweb-skeleton
RUN --mount=type=bind,target=/context \
    sh /context/scripts/deploy/cargo-skeleton.sh /context /skeleton apps/openagents/web

FROM rust:1.98.1-slim-trixie AS gpuiweb-base
RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y \
        --no-install-recommends build-essential pkg-config clang llvm brotli ca-certificates curl python3 \
    && rm -rf /var/lib/apt/lists/*
# wasm-opt from Binaryen's own release, the version the budget is measured
# with on our machines: trixie's binaryen (120) leaves the chat module about
# 2.5 KB larger with brotli, over package-web-build.py's budget.
ARG BINARYEN_VERSION=132
ARG BINARYEN_SHA256=195ddc94f9bc89f45abdabb0b9eea86023d727ba90eac8b35b80f2544fc30572
RUN curl -fsSLo /tmp/binaryen.tar.gz \
        "https://github.com/WebAssembly/binaryen/releases/download/version_${BINARYEN_VERSION}/binaryen-version_${BINARYEN_VERSION}-x86_64-linux.tar.gz" \
    && echo "${BINARYEN_SHA256}  /tmp/binaryen.tar.gz" | sha256sum -c - \
    && tar -xzf /tmp/binaryen.tar.gz -C /tmp \
    && install -m 0755 "/tmp/binaryen-version_${BINARYEN_VERSION}/bin/wasm-opt" /usr/local/bin/wasm-opt \
    && rm -rf /tmp/binaryen.tar.gz "/tmp/binaryen-version_${BINARYEN_VERSION}" \
    && wasm-opt --version
COPY scripts/deploy/oa-kache.sh /usr/local/bin/oa-kache
RUN oa-kache install
WORKDIR /build
COPY apps/openagents/web/Cargo.lock /build/gpuiweb.lock
RUN rustup target add wasm32-unknown-unknown \
    && version="$(awk '$0 == "name = \"wasm-bindgen\"" { getline; gsub(/"/, "", $3); print $3; exit }' gpuiweb.lock)" \
    && test -n "$version" \
    && cargo install --locked wasm-bindgen-cli --version "$version"
ENV CC_wasm32_unknown_unknown=/usr/bin/clang \
    AR_wasm32_unknown_unknown=/usr/bin/llvm-ar \
    CARGO_TARGET_DIR=/build/target-gpuiweb

FROM gpuiweb-base AS gpuiweb-deps
COPY --from=gpuiweb-skeleton /skeleton/ ./
RUN --network=host --mount=type=cache,target=/var/cache/kache,sharing=private oa-kache run \
    cargo build --locked --manifest-path /build/apps/openagents/web/Cargo.toml \
        -p openagents-web-demo -p openagents-web-agent-graph -p openagents-web-chat --release --target wasm32-unknown-unknown \
    && oa-kache run cargo build --locked --manifest-path /build/apps/openagents/web/Cargo.toml \
        -p openagents-first-frame-paint --release --target wasm32-unknown-unknown \
    && oa-kache run cargo build --locked --manifest-path /build/apps/openagents/web/Cargo.toml \
        -p openagents-first-frame-capture --bin first-frame-capture --profile capture

FROM gpuiweb-deps AS gpuiweb
COPY . .
RUN --network=host --mount=type=cache,target=/var/cache/kache,sharing=private find . -path ./target -prune -o -path ./target-gpuiweb -prune -o -exec touch -c {} + \
    && apps/openagents/scripts/build-web-fonts.sh /build/gpui-fonts \
    && test -s /build/gpui-fonts/fonts.json \
    && oa-kache run apps/openagents/scripts/build-web-demo.sh /build/gpui-demo \
    && test -s /build/gpui-demo/openagents_web_demo.js \
    && test -s /build/gpui-demo/openagents_web_demo_bg.wasm \
    && test -s /build/gpui-demo/start.js \
    && oa-kache run apps/openagents/scripts/build-web-demo.sh --app agent-graph /build/agent-graph \
    && test -s /build/agent-graph/openagents_web_agent_graph.js \
    && test -s /build/agent-graph/openagents_web_agent_graph_bg.wasm \
    && test -s /build/agent-graph/start.js \
    && test -s /build/agent-graph/first-frame/manifest.json \
    && test -s /build/agent-graph/first-frame/painter.wasm.gz \
    && test -s /build/agent-graph/first-frame/foreman-desktop.ffs.gz \
    && test -x /build/agent-graph/first-frame/first-frame-capture \
    && oa-kache run apps/openagents/scripts/build-web-chat.sh /build/gpui-chat \
    && test -s /build/gpui-chat/openagents_web_chat.js \
    && test -s /build/gpui-chat/openagents_web_chat_bg.wasm \
    && test -s /build/gpui-chat/start.js \
    && test -s /build/gpui-chat/fonts/Geist.ttf \
    && oa-kache run apps/openagents/scripts/build-web-pylon-card.sh /build/pylons-card \
    && test -s /build/pylons-card/webgpu/openagents_web_pylon_card_bg.wasm.br \
    && test -s /build/pylons-card/webgl/openagents_web_pylon_card_bg.wasm.br \
    && oa-kache run apps/openagents/scripts/build-web-lander-chat.sh /build/lander8 \
    && test -s /build/lander8/webgpu/openagents_web_lander_chat_bg.wasm.br \
    && test -s /build/lander8/webgl/openagents_web_lander_chat_bg.wasm.br \
    && scripts/fetch-hand-landmarker.sh /build/lander9/hands \
    && test -s /build/lander9/hands/hand_landmarker.task.br \
    && test -s /build/lander9/hands/wasm/vision_wasm_internal.wasm.br

# Every dependency layer, and nothing else: cloudbuild.yaml builds this
# target first and exports it to the registry cache.
FROM scratch AS deps
COPY --from=builder-deps /build/Cargo.lock /builder-deps.lock
COPY --from=everglade-deps /build/Cargo.lock /everglade-deps.lock
COPY --from=cadweb-deps /build/Cargo.lock /cadweb-deps.lock
COPY --from=gpuiweb-deps /build/gpuiweb.lock /gpuiweb-deps.lock

FROM debian:13-slim

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y \
        --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --uid 10001 --user-group --create-home \
        --home-dir /home/web --shell /usr/sbin/nologin web
COPY --from=builder /build/target/release/openagents-web /usr/local/bin/openagents-web
COPY --from=builder /build/target/release/cad-lab /usr/local/bin/cad-lab
COPY --from=builder /build/target/release/lab-cad-copy /usr/local/bin/lab-cad-copy
COPY --from=everglade /build/everglade /srv/everglade
COPY --from=everglade /build/components /srv/components
COPY --from=everglade /build/cloud /srv/cloud
COPY --from=everglade /build/chat /srv/chat
COPY --from=everglade /build/bunny /srv/bunny
COPY --from=everglade /build/icecream /srv/icecream
COPY --from=everglade /build/snap /srv/snap
COPY --from=everglade /build/robot /srv/robot
ENV OPENAGENTS_ROBOT_BUILD=/srv/robot
COPY --from=everglade /build/att /srv/att
COPY --from=cadweb /build/cad /srv/cad
COPY --from=everglade /build/sand /srv/sand
COPY --from=everglade /build/layertree /srv/layertree
COPY --from=everglade /build/lander5 /srv/lander5
COPY --from=gpuiweb /build/gpui-demo /srv/gpui-demo
COPY --from=gpuiweb /build/agent-graph /srv/agent-graph
COPY --from=gpuiweb /build/gpui-fonts /srv/gpui-fonts
COPY --from=gpuiweb /build/gpui-chat /srv/gpui-chat
COPY --from=everglade /build/pylons-layer /srv/pylons/layer
COPY --from=gpuiweb /build/pylons-card /srv/pylons/card
COPY --from=gpuiweb /build/lander8 /srv/lander8
COPY --from=gpuiweb /build/lander9 /srv/lander9

USER 10001:10001
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/openagents-web"]
CMD ["--listen", "0.0.0.0:8080", "--public-host", "openagents.com", "--everglade", "/srv/everglade", "--components-build", "/srv/components", "--cloud-build", "/srv/cloud", "--chat-build", "/srv/chat", "--bunny", "/srv/bunny"]
