#!/usr/bin/env bash
# Fail fast when a folder under crates/ has no Cargo.toml (#11593).
#
# The root workspace takes every crates/* folder as a member, so a new crate
# joins without anyone editing a list. The cost: one folder without a
# Cargo.toml (fixtures copied ahead of the crate, a half-made scaffold) stops
# every cargo command with "failed to read .../Cargo.toml". Cargo has no
# setting to skip such a folder in a glob, and its exclude list only takes
# names known in advance, so this check names the folder and the fix instead.
#
# Usage: scripts/dev/check-crate-dirs.sh [WORKSPACE_ROOT]
# Exits 0 when every crates/* folder is a package or an excluded nested
# workspace, 1 when one is not.
set -euo pipefail
root=${1:-"$(cd "$(dirname "$0")/../.." && pwd)"}
cd "$root"

stray=()
for dir in crates/*/; do
  dir=${dir%/}
  [[ -f $dir/Cargo.toml ]] && continue
  stray+=("$dir")
done

if (( ${#stray[@]} == 0 )); then
  exit 0
fi

for dir in "${stray[@]}"; do
  name=${dir#crates/}
  echo "FAIL: $dir has no Cargo.toml, so every cargo command in this checkout fails." >&2
  echo "      It may be someone's work in progress; do not delete it." >&2
  echo "      Move it out of the workspace:  mv $dir ../$name.parked" >&2
  echo "      (or give it a Cargo.toml if it is meant to be a crate)" >&2
done
exit 1
