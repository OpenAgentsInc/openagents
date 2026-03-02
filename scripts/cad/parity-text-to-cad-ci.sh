#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

cd "$ROOT_DIR"

cargo run -p openagents-cad --bin parity-text-to-cad -- --check
cargo test -p openagents-cad --test parity_text_to_cad --quiet
cargo test -p openagents-cad text_to_cad --quiet
