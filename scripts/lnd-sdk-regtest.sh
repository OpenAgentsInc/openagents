#!/usr/bin/env bash
# Run the SDK against isolated, unfunded Bitcoin/LND/Taproot Assets fixtures.
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/work/openagents-target-agent2}"
scratch=$(openagents scratch)
export LND_SDK_REGTEST_ROOT="$scratch/lnd-sdk-regtest-$(date +%s)-$$"
cargo test -p lnd-sdk --features supervisor --test regtest --test recovery --test liquidity -- --ignored --nocapture --test-threads=1
