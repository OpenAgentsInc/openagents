#!/usr/bin/env bash
# Build the portable headless Coder artifact for Boat and other Linux hosts.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
target_triple=x86_64-unknown-linux-musl
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/work/openagents-target-agent0}"
export CODER_BUILD_COMMIT="$(git -C "$root" rev-parse HEAD)"
if [[ -n "$(git -C "$root" status --porcelain)" ]]; then
  echo "Build the cloud runtime from a clean commit." >&2
  exit 1
fi
export CODER_BUILD_DIRTY=0
build() {
  cd "$root"
  openagents lease build --keep-target-dir -- cargo build --locked --release -p coder-new --bin coder-cloud-runtime --target "$target_triple"
}
export -f build
export root target_triple
if [[ -n "${CC_x86_64_unknown_linux_musl:-}" ]]; then
  build
elif command -v x86_64-linux-musl-gcc >/dev/null 2>&1; then
  export CC_x86_64_unknown_linux_musl=x86_64-linux-musl-gcc
  export CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=x86_64-linux-musl-gcc
  build
else
  nix shell nixpkgs#pkgsCross.musl64.stdenv.cc -c bash -c '
    export CC_x86_64_unknown_linux_musl=x86_64-unknown-linux-musl-gcc
    export AR_x86_64_unknown_linux_musl=x86_64-unknown-linux-musl-ar
    export CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=x86_64-unknown-linux-musl-gcc
    build'
fi
binary="$CARGO_TARGET_DIR/$target_triple/release/coder-cloud-runtime"
"$binary" --runtime-manifest
sha256sum "$binary"
