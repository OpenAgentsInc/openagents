#!/usr/bin/env bash
# Regenerate the checked-in RPC types from vendored release protocols.
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/work/openagents-target-agent2}"
LND_SDK_REGENERATE=1 cargo test -p lnd-sdk --test generate
