#!/usr/bin/env bash
# Build the pinned headless WoW helper outside the repository.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/work/openagents-target-agent1}"
(cd "$root/wow-bridge" && cargo build --locked --release)
echo "$CARGO_TARGET_DIR/release/wow-bridge"
