#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

cd "$ROOT_DIR"

cargo run -p openagents-cad --bin parity-compact-ir -- --check
cargo test -p openagents-cad --test parity_compact_ir --quiet
cargo test -p openagents-cad --test compact_ir --quiet
