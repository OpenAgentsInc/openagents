#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

cd "$ROOT_DIR"

"$ROOT_DIR/scripts/cad/vcad-capability-crawler-ci.sh"
"$ROOT_DIR/scripts/cad/openagents-capability-crawler-ci.sh"

cargo run -p openagents-cad --bin parity-gap-matrix -- --check
