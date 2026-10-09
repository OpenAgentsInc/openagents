#!/usr/bin/env bash
# Resolve the exact packages coderdev builds; no Verse crate may be reachable.
set -euo pipefail
cd "$(dirname "$0")/.."
tree="$(cargo tree --offline --locked -p coder-new -p microcoder --prefix none)"
if grep -E '^verse(-| )' <<<"$tree"; then
  echo 'coderdev: unexpected Verse dependency' >&2
  exit 1
fi
echo 'test-coderdev-no-verse: ok'
