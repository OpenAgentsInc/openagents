#!/usr/bin/env bash
# Builds the Coder One artifact the Terminal-Bench harness installs: a
# static x86_64 Linux binary, stamped with the commit it was built from,
# and prints the path and sha256 to pin with --agent-kwarg.
#
# Task environments are Linux containers with arbitrary base images, so
# the binary links musl statically and needs nothing from the image. The
# `ring` crate compiles C, which needs a musl C compiler: set
# CC_x86_64_unknown_linux_musl, put x86_64-linux-musl-gcc on PATH, or run
# on a machine with nix, where this script fetches one.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target_triple="x86_64-unknown-linux-musl"
toolchain="$(sed -n 's/^channel = "\(.*\)"/\1/p' "$root/rust-toolchain.toml")"
# A separate target directory from the workspace's, since the target differs.
target="${CODER_ONE_TARGET_DIR:-$root/target-coder-one}"

rustup target add --toolchain "$toolchain" "$target_triple" >/dev/null

commit="$(git -C "$root" rev-parse --short=12 HEAD)"
if [ -n "$(git -C "$root" status --porcelain -- crates/coder-one crates/jev crates/atif crates/supervise)" ]; then
  commit="$commit-dirty"
fi

build() {
  CODER_ONE_COMMIT="$commit" \
    CARGO_TARGET_DIR="$target" \
    cargo "+$toolchain" build --release --locked -p coder-one --target "$target_triple" \
    --manifest-path "$root/Cargo.toml"
}

if [ -n "${CC_x86_64_unknown_linux_musl:-}" ]; then
  build
elif command -v x86_64-linux-musl-gcc >/dev/null 2>&1; then
  CC_x86_64_unknown_linux_musl=x86_64-linux-musl-gcc \
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=x86_64-linux-musl-gcc build
elif command -v nix >/dev/null 2>&1; then
  export -f build
  export commit target target_triple toolchain root
  nix shell nixpkgs#pkgsCross.musl64.stdenv.cc -c bash -c '
    CC_x86_64_unknown_linux_musl=x86_64-unknown-linux-musl-gcc \
    AR_x86_64_unknown_linux_musl=x86_64-unknown-linux-musl-ar \
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=x86_64-unknown-linux-musl-gcc build'
else
  echo "no musl C compiler: set CC_x86_64_unknown_linux_musl or install musl-tools" >&2
  exit 1
fi

binary="$target/$target_triple/release/coder-one"
echo "artifact_path=$binary"
echo "artifact_sha256=$(sha256sum "$binary" | cut -d' ' -f1)"
echo "version: $("$binary" --version)"
