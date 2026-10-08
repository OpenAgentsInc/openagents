#!/usr/bin/env bash
# Idempotent Cloud Agent install for the OpenAgents workspace.
# System packages match scripts/cloud/coder-host-setup.sh, plus the
# headers native crates ask pkg-config for (OpenSSL, SQLite, libclang).
set -euo pipefail

export DEBIAN_FRONTEND=noninteractive

sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  build-essential \
  pkg-config \
  clang \
  cmake \
  protobuf-compiler \
  libprotobuf-dev \
  libssl-dev \
  libsqlite3-dev \
  libclang-dev \
  ca-certificates \
  curl \
  git \
  ripgrep \
  jq \
  xz-utils \
  unzip \
  zstd \
  procps \
  bubblewrap \
  python3

if ! command -v rustup >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs \
    | sh -s -- -y --no-modify-path --profile minimal --default-toolchain none
  sudo ln -sfn "${HOME}/.cargo/bin/rustup" /usr/local/bin/rustup
  sudo ln -sfn "${HOME}/.cargo/bin/cargo" /usr/local/bin/cargo
  sudo ln -sfn "${HOME}/.cargo/bin/rustc" /usr/local/bin/rustc
  sudo ln -sfn "${HOME}/.cargo/bin/rustfmt" /usr/local/bin/rustfmt
  sudo ln -sfn "${HOME}/.cargo/bin/cargo-fmt" /usr/local/bin/cargo-fmt
  sudo ln -sfn "${HOME}/.cargo/bin/cargo-clippy" /usr/local/bin/cargo-clippy
fi

rustup toolchain install 1.97.1 --profile minimal --component rustfmt,clippy
rustup default 1.97.1

mkdir -p "${HOME}/work"

# Registry and git crates for the root workspace. The phone crate is its own
# workspace; its checked-in lock file does not resolve with --locked on this
# toolchain, so this install leaves that fetch to the person editing it.
cargo fetch --locked --manifest-path Cargo.toml
