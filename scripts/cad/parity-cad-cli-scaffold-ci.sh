#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

cd "$ROOT_DIR"

cargo run -p openagents-cad --bin parity-cad-cli-scaffold -- --check
cargo test -p openagents-cad --test parity_cad_cli_scaffold --quiet
cargo test -p openagents-cad --test cad_cli_scaffold --quiet
