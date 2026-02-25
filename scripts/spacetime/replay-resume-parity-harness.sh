#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

echo "==> runtime sync parity checks"
cargo test -p openagents-runtime-service spacetime_sync_metrics_expose_stream_delivery_totals -- --nocapture
cargo test -p openagents-runtime-service retired_spacetime_routes_return_not_found -- --nocapture

echo "==> shared client replay/resume checks"
cargo test -p autopilot-spacetime client::tests -- --nocapture

echo "==> desktop replay/resume checks"
cargo test -p autopilot-desktop sync_checkpoint_store -- --nocapture
cargo test -p autopilot-desktop sync_apply_engine -- --nocapture
cargo test -p autopilot-desktop sync_lifecycle -- --nocapture

echo "Spacetime replay/resume parity harness passed."
