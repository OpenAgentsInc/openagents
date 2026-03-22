#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

export CARGO_INCREMENTAL=0

run_with_timeout() {
  local timeout_seconds="$1"
  shift
  python3 - "$timeout_seconds" "$@" <<'PY'
import os
import subprocess
import sys

timeout_seconds = int(sys.argv[1])
command = sys.argv[2:]
completed = subprocess.run(command, timeout=timeout_seconds)
raise SystemExit(completed.returncode)
PY
}

(
  cd "$ROOT"
  run_with_timeout 180 scripts/autopilot/headless-data-market-smoke.sh
)

(
  cd "$ROOT"
  OPENAGENTS_HEADLESS_DATA_MARKET_PRICE_SATS=0 \
    run_with_timeout 300 scripts/autopilot/headless-data-market-e2e.sh
)

(
  cd "$ROOT"
  cargo test -p autopilot-desktop --lib relay_catalog_projection_preserves_existing_link_hints_without_kernel_records -- --nocapture
)

(
  cd "$ROOT"
  cargo test -p autopilot-desktop --lib relay_delivery_synthesis_uses_ds_result_when_kernel_delivery_state_is_empty -- --nocapture
)
