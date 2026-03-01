#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

cd "$ROOT_DIR"

cargo test -p autopilot-desktop cad_headless_script_harness_ --quiet
cargo test -p autopilot-desktop cad_release_gate_reliability_reuses_canonical_script_fixture --quiet
cargo test -p autopilot-desktop cad_chat_build_e2e_harness --quiet
