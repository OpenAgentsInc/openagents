#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export OPENAGENTS_AUTOPILOT_TAURI_SMOKE_TIMEOUT_MS="${OPENAGENTS_AUTOPILOT_TAURI_SMOKE_TIMEOUT_MS:-600000}"
exec "$SCRIPT_DIR/tauri-control-smoke.sh" --homework-matrix "$@"
