#!/usr/bin/env bash
# Builds the Minecraft bridge helper, the nightly Rust executable that
# plays the game through azalea.
#
# The helper is a workspace of its own under `mc-bridge/` because azalea
# requires nightly Rust — `simdnbt` uses `portable_simd` — which the
# pinned stable toolchain cannot provide. Everything in `crates/voyager`
# builds and tests without it; the crate talks to the helper as a
# supervised child process, the `swift/lev-bridge` precedent.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
package="$root/mc-bridge"
# A separate target directory per worktree, same rule as the workspace.
target="${CARGO_TARGET_DIR:-$root/target-mc}"

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is not on PATH; install rustup" >&2
  exit 1
fi

# The helper pins its own toolchain in mc-bridge/rust-toolchain.toml.
toolchain="$(sed -n 's/^channel = "\(.*\)"/\1/p' "$package/rust-toolchain.toml")"
if ! rustup toolchain list 2>/dev/null | grep -qF "$toolchain"; then
  echo "rustup toolchain $toolchain is not installed" >&2
  echo "install it with: rustup toolchain install $toolchain" >&2
  exit 1
fi

(cd "$package" && CARGO_TARGET_DIR="$target" cargo build --release)

binary="$target/release/mc-bridge"
if [ ! -f "$binary" ]; then
  echo "expected the helper at $binary and did not find it" >&2
  exit 1
fi
echo "built $binary"
