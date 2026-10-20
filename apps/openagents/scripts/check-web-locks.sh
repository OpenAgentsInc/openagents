#!/usr/bin/env bash
# Checks the WebGPU-only web builds' own lock files (card, graph-card,
# pylon-card, lander-chat: each `web/*/webgpu/`), and with --write brings
# them up to date. Their build scripts build with --locked, so a change to
# a crate those builds link (chat-view, client, the SDK) that leaves one
# stale fails the website image. Run it after changing web dependencies.
#
# Usage: apps/openagents/scripts/check-web-locks.sh [--write]
set -euo pipefail

web="$(cd "$(dirname "$0")/../web" && pwd)"
write=false
[ "${1:-}" = "--write" ] && write=true

wgpu_version="$(awk '/^name = "wgpu"$/ { getline; gsub(/version = |"/, ""); print; exit }' "$web/Cargo.lock")"
cargo fetch --locked --manifest-path "$web/Cargo.toml" >/dev/null
registry="${CARGO_HOME:-$HOME/.cargo}/registry/src"
source_dir="$(find "$registry" -maxdepth 2 -type d -name "wgpu-$wgpu_version" | head -n 1)"
[ -n "$source_dir" ] || { echo "wgpu $wgpu_version is not in $registry" >&2; exit 69; }

stale=0
for manifest in "$web"/*/webgpu/Cargo.toml; do
  dir="$(dirname "$manifest")"
  patched="$dir/wgpu"
  [ -e "$patched" ] && { echo "$patched exists; remove it first" >&2; exit 70; }
  cp -R "$source_dir" "$patched"
  awk '
    /^default = \[$/ { print "default = [\"std\", \"parking_lot\", \"wgsl\", \"webgpu\"]"; skip = 1; next }
    skip && /^\]$/ { skip = 0; next }
    !skip { print }
  ' "$source_dir/Cargo.toml" > "$patched/Cargo.toml"
  if cargo metadata --locked --format-version 1 --manifest-path "$manifest" >/dev/null 2>&1; then
    echo "fresh: ${dir#"$web"/}"
  elif $write; then
    cargo metadata --format-version 1 --manifest-path "$manifest" >/dev/null
    echo "updated: ${dir#"$web"/}"
  else
    echo "stale: ${dir#"$web"/} (run with --write)"
    stale=1
  fi
  rm -rf "$patched"
done
exit "$stale"
