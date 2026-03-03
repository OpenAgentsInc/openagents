#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

cd "$ROOT_DIR"

cargo run -p openagents-cad --bin parity-final-parity-certification-report -- --check
cargo test -p openagents-cad --test parity_final_parity_certification_report --quiet
