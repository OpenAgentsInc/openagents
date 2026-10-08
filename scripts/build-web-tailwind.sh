#!/usr/bin/env bash
# Regenerates the website's Tailwind utilities.
#
#   ./scripts/build-web-tailwind.sh
#
# Reads `crates/openagents-web/static/tailwind.input.css`, scans the crate's
# Rust sources and scripts for class names, and writes the minified result
# to `crates/openagents-web/static/tailwind.css`, which is checked in and
# served at `/static/tailwind.css`.
#
# Uses the Tailwind CSS standalone CLI, a single binary, so no Node or npm
# is involved. The pinned release is downloaded once into
# `~/.openagents/tools/tailwindcss/<version>/` (`OPENAGENTS_TAILWIND_DIR`
# overrides the root) and verified against the SHA-256 recorded below; a
# binary that fails the digest is deleted, not run.
set -euo pipefail

version="v4.3.3"

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64)
    asset="tailwindcss-macos-arm64"
    sha256="cdf646702987a743464dff4d9c60fd4480d1c1e73dd819a9a67f1078815dce9d"
    ;;
  Linux-x86_64)
    asset="tailwindcss-linux-x64"
    sha256="dc61b3ac6b8c9ca874c0cc4c57b2409791a64c5540404ca5f5367360babc313a"
    ;;
  *)
    echo "no pinned Tailwind CLI for $(uname -s)-$(uname -m)" >&2
    exit 1
    ;;
esac

root="$(cd "$(dirname "$0")/.." && pwd)"
crate="$root/crates/openagents-web"
cache="${OPENAGENTS_TAILWIND_DIR:-$HOME/.openagents/tools/tailwindcss}/$version"
bin="$cache/$asset"

digest() {
  shasum -a 256 "$1" | awk '{print $1}'
}

if [ ! -x "$bin" ] || [ "$(digest "$bin")" != "$sha256" ]; then
  mkdir -p "$cache"
  echo "fetching Tailwind CSS $version ($asset)"
  curl -fsSL -o "$bin.part" \
    "https://github.com/tailwindlabs/tailwindcss/releases/download/$version/$asset"
  actual="$(digest "$bin.part")"
  if [ "$actual" != "$sha256" ]; then
    echo "sha256 mismatch: expected $sha256, got $actual" >&2
    rm -f "$bin.part"
    exit 1
  fi
  chmod +x "$bin.part"
  mv "$bin.part" "$bin"
fi

cd "$crate"
"$bin" --input static/tailwind.input.css --output static/tailwind.css --minify
echo "wrote $crate/static/tailwind.css"
