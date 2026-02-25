#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

echo "==> runtime shadow parity checks"
cargo test -p openagents-runtime-service shadow_control_khala -- --nocapture
cargo test -p openagents-runtime-service shadow::tests -- --nocapture

echo "==> shared client replay/resume checks"
cargo test -p autopilot-spacetime client::tests -- --nocapture

echo "==> desktop replay/resume checks"
cargo test -p autopilot-desktop sync_checkpoint_store -- --nocapture
cargo test -p autopilot-desktop sync_apply_engine -- --nocapture
cargo test -p autopilot-desktop sync_lifecycle -- --nocapture

echo "Spacetime replay/resume parity harness passed."
