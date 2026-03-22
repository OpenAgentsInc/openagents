#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

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
  cargo test -p autopilot-desktop data_seller_full_lifecycle_progresses_from_grant_to_revocation -- --nocapture
)

(
  cd "$ROOT"
  cargo test -p nexus-control data_market_flow_receipts_asset_grant_delivery_and_revocation -- --nocapture
)
