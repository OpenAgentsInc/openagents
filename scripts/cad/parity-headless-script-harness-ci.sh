#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

cd "$ROOT_DIR"

cargo run -p openagents-cad --bin parity-headless-script-harness -- --check
cargo test -p openagents-cad --test parity_headless_script_harness --quiet
cargo test -p openagents-cad headless_script_harness --quiet
