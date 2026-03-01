#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

cd "$ROOT_DIR"

cargo test -p autopilot-desktop cad_demo_20s_reliability_script_has_no_stalls_flicker_or_state_loss --quiet
