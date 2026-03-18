#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

(
  cd "$ROOT"
  timeout 180s scripts/autopilot/headless-data-market-smoke.sh
)

(
  cd "$ROOT"
  cargo test -p autopilot-desktop data_seller_full_lifecycle_progresses_from_grant_to_revocation -- --nocapture
)

(
  cd "$ROOT"
  cargo test -p nexus-control data_market_flow_receipts_asset_grant_delivery_and_revocation -- --nocapture
)
