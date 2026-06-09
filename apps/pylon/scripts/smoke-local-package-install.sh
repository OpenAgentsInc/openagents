#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

cd "$repo_root"
pack_output="$(bun pm pack)"
tarball="$(printf '%s\n' "$pack_output" | awk '/openagentsinc-pylon-.*\.tgz$/ {print $1}' | tail -1)"

if [[ -z "$tarball" || ! -f "$repo_root/$tarball" ]]; then
  printf 'failed to locate packed Pylon tarball\n' >&2
  printf '%s\n' "$pack_output" >&2
  exit 1
fi

cd "$tmp_dir"
bun init -y >/dev/null
bun add "$repo_root/$tarball" >/dev/null
PYLON_HOME="$tmp_dir/pylon-home" bunx pylon bootstrap --json > bootstrap.json
bun -e 'const summary = await Bun.file("bootstrap.json").json(); if (summary.packageName !== "@openagentsinc/pylon" || summary.bin !== "pylon" || !summary.platform.supported) process.exit(1);'

printf 'local package install smoke passed\n'
