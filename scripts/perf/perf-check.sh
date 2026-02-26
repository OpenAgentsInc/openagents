#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

"$ROOT_DIR/scripts/perf/compile-budget-check.sh"
"$ROOT_DIR/scripts/perf/microbench-check.sh"
