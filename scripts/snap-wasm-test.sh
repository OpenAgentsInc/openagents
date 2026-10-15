#!/usr/bin/env bash
# Run Snap Station's rules tests as wasm32 under Node, to check that the
# same inputs end a run in the same state as on native (the
# `a_fixed_run_ends_in_the_same_state_everywhere` test pins that state), and
# that the bot's builds do every job there too.
#
# Prerequisites: `rustup target add wasm32-wasip1` and Node 20 or newer.
#
# Usage: ./scripts/snap-wasm-test.sh [TEST FILTER...]
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
export CARGO_TARGET_WASM32_WASIP1_RUNNER="node $root/scripts/snap-wasm-test.mjs"
# The whole suite is quick as wasm, the bot's job runs included.
cargo test -p snap-rules --lib --target wasm32-wasip1 -- --test-threads=1 "$@"
