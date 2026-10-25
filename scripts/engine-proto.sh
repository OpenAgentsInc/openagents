#!/usr/bin/env bash
# Generate or verify the checked-in engine message bindings.
set -euo pipefail
cd "$(dirname "$0")/.."
mode="${1:-check}"
if [[ "$mode" == export ]]; then
  exec "${ENGINE_PROTO_PYTHON:-python3}" crates/engine-proto/tools/export.py "${2:?Specify an export directory}"
fi
if [[ "$mode" != generate && "$mode" != check ]]; then
  echo 'Usage: scripts/engine-proto.sh [generate|check|export DIRECTORY]' >&2
  exit 2
fi
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/work/openagents-target-1}"
. scripts/dev/cargo-build-env.sh
"${ENGINE_PROTO_PYTHON:-python3}" crates/engine-proto/tools/generate.py "$mode"
"${ENGINE_PROTO_PYTHON:-python3}" -m unittest discover -s crates/engine-proto/tools
if [[ "$mode" == generate ]]; then
  ENGINE_PROTO_REGENERATE=1 cargo test -p engine-proto --test generate
else
  cargo test -p engine-proto --test generate
fi
