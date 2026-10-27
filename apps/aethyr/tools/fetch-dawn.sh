#!/usr/bin/env bash
# Prepares a Dawn install for af-gpu's `native` feature from Dawn's own pinned
# prebuilt release (github.com/google/dawn/releases):
#
#   <out>/include/webgpu/webgpu.h   Dawn's webgpu.h (and the dawn/ headers it includes)
#   <out>/lib/libwebgpu_dawn.a      the static library af-gpu's build.rs links
#   <out>/lib/dawn-version          release tag and Dawn commit
#
# The archive is checked against the SHA-256 recorded below before anything is
# extracted; an unknown release or platform is refused rather than downloaded
# unpinned. Prebuilt releases exist for macOS arm64 and x86_64 and Linux x86_64.
#
# Usage:
#   tools/fetch-dawn.sh [--out DIR]
#
# Then build from crates/aethyr with:
#   AF_WEBGPU_DIR=DIR AF_WEBGPU_IMPL=dawn cargo build -p af-render-lab --features native --release
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
monorepo="$(cd "$repo/../.." && pwd -P)"
# Pinned 2026-10-26 (#12600). Digests are the ones GitHub publishes for the
# release assets, re-checked by downloading each archive.
release="v20261023.194543"
commit="1716e88cc9e58398fab5d4473f7f2e858667b677"
out="$(dirname "$monorepo")/webgpu-dawn"

while [ $# -gt 0 ]; do
  case "$1" in
    --out) out="$2"; shift 2 ;;
    -h|--help) sed -n '2,18p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64)
    platform="macos-latest"
    expected="8f013482cad9e634995538089c8354c2a70b7a669a3952cd03ab60d297a55f9a" ;;
  Darwin-x86_64)
    platform="macos-15-intel"
    expected="a22c3faa2777e01dee29aa5f969da80cd608d31fdaa57e95b532495838cb8048" ;;
  Linux-x86_64)
    platform="ubuntu-latest"
    expected="bef250fdfbd078ffc22ffc9a756b66b190e2b04121c0f6df216e7905395bc45b" ;;
  *) echo "no pinned Dawn release for $(uname -s)-$(uname -m)" >&2; exit 1 ;;
esac

name="Dawn-$commit-$platform-Release"
url="https://github.com/google/dawn/releases/download/$release/$name.tar.gz"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
echo "Downloading $url" >&2
curl -fsSL -o "$tmp/dawn.tar.gz" "$url"
if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "$tmp/dawn.tar.gz" | cut -d' ' -f1)"
else
  actual="$(shasum -a 256 "$tmp/dawn.tar.gz" | cut -d' ' -f1)"
fi
if [ "$actual" != "$expected" ]; then
  echo "checksum mismatch for $name.tar.gz: $actual (expected $expected)" >&2
  exit 1
fi
mkdir -p "$out/lib"
rm -rf "$out/include"
# The library sits in lib/ on macOS and lib64/ on Linux.
library="$(tar -tzf "$tmp/dawn.tar.gz" | grep -E "^$name/lib(64)?/libwebgpu_dawn\.a$" | head -1)"
[ -n "$library" ] || { echo "no libwebgpu_dawn.a in $name.tar.gz" >&2; exit 1; }
tar -xzf "$tmp/dawn.tar.gz" -C "$tmp" "$name/include" "$library"
mv "$tmp/$name/include" "$out/include"
mv "$tmp/$library" "$out/lib/"
printf '%s %s %s\n' "$release" "$commit" "$platform" > "$out/lib/dawn-version"
[ -f "$out/include/webgpu/webgpu.h" ] || { echo "missing $out/include/webgpu/webgpu.h" >&2; exit 1; }
echo "Dawn $release ($commit, $platform) in $out" >&2
echo "export AF_WEBGPU_DIR=$out AF_WEBGPU_IMPL=dawn"
