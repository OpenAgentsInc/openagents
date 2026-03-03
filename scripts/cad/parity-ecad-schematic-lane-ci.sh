#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

cd "$ROOT_DIR"

cargo run -p openagents-cad --bin parity-ecad-schematic-lane -- --check
cargo test -p openagents-cad --test parity_ecad_schematic_lane --quiet
