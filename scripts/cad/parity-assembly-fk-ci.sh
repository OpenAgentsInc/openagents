#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

cd "$ROOT_DIR"

cargo run -p openagents-cad --bin parity-assembly-fk -- --check
cargo test -p openagents-cad --test parity_assembly_fk --quiet
