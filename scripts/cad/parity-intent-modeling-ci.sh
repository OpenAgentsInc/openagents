#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

cd "$ROOT_DIR"

cargo run -p openagents-cad --bin parity-intent-modeling -- --check
cargo test -p openagents-cad --test parity_intent_modeling --quiet
cargo test -p openagents-cad intent_execution --quiet
