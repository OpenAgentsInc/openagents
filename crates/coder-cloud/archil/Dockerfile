# The image every `chat work --on archil` sandbox starts from (#12136).
#
# Archil's managed network allowlist blocks static.rust-lang.org, so the
# toolchains the repository pins (rust-toolchain.toml and
# apps/openagents/rust-toolchain.toml), with rustfmt and clippy, are baked in
# here, with the GPUI system libraries apps/openagents' crates build against
# (apps/openagents/CONTRIBUTORS.md). Node carries Codex and Claude Code.
# Runs happen as `coder`.
#
# Build and push with Cloud Build (docs/cloud/archil.md),
# then the first `chat work --on archil` imports it into Archil once.
FROM node:22-bookworm-slim AS node

FROM rust:1.97.1-bookworm
ENV RUSTUP_HOME=/usr/local/rustup CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
RUN rustup component add rustfmt clippy \
 && rustup toolchain install 1.98.1 --profile minimal -c rustfmt -c clippy \
 && rustup default 1.97.1
RUN apt-get update \
 && DEBIAN_FRONTEND=noninteractive apt-get install -y -q --no-install-recommends \
      jq python3 python3-numpy bubblewrap ca-certificates curl git xz-utils \
      clang lld cmake protobuf-compiler libprotobuf-dev pkg-config libssl-dev \
      libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libx11-dev libxcb1-dev \
      libx11-xcb-dev libfontconfig1-dev libfreetype-dev libasound2-dev libvulkan-dev \
      libwebkit2gtk-4.1-dev libjson-glib-dev \
 && rm -rf /var/lib/apt/lists/*
COPY --from=node /usr/local/bin/node /usr/local/bin/node
COPY --from=node /usr/local/lib/node_modules /usr/local/lib/node_modules
RUN ln -s ../lib/node_modules/npm/bin/npm-cli.js /usr/local/bin/npm \
 && ln -s ../lib/node_modules/npm/bin/npx-cli.js /usr/local/bin/npx \
 && npm install -g @openai/codex @anthropic-ai/claude-code \
 && npm cache clean --force
RUN useradd -m -s /bin/bash coder \
 && chown -R coder:coder /usr/local/cargo /usr/local/rustup
