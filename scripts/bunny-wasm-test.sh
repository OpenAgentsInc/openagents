#!/usr/bin/env bash
# Run Grow Little Bunny's rules tests as wasm32 under Node, to check that the
# same inputs end a run in the same state as on native (the
# `a_fixed_run_ends_in_the_same_state_everywhere` test pins that state).
#
# Prerequisites: `rustup target add wasm32-wasip1` and Node 20 or newer.
#
# Usage: ./scripts/bunny-wasm-test.sh [TEST FILTER...]
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
export CARGO_TARGET_WASM32_WASIP1_RUNNER="node $root/scripts/bunny-wasm-test.mjs"
# The bot playtests are slow as wasm; the determinism and receipt tests are
# what this run is for.
cargo test -p bunny-rules --lib --target wasm32-wasip1 -- --test-threads=1 "${@:-receipt::}"
