#!/usr/bin/env bash
# Verify only the excluded LayerTree lab, on a contributor or Cloud Build worker.
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/work/openagents-target-agent2}"
exec cargo kani --manifest-path crates/layertree-tee-lab/Cargo.toml \
  --lib -Z stubbing --output-format terse "$@"
