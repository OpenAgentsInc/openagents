#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

cd "$ROOT_DIR"

"$ROOT_DIR/scripts/cad/parity-scorecard-ci.sh"

cargo run -p openagents-cad --bin parity-fixture-corpus -- --check
