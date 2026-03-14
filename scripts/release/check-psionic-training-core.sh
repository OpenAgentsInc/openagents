#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release/check-psionic-training-core.sh

Runs the canonical Psionic fixed-budget training-core reference harness for #3564.
EOF
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${repo_root}"

run_cmd() {
  echo
  echo "==> $*"
  "$@"
}

run_cmd cargo test -p psionic-train --lib fixed_budget_training_loop_applies_updates_and_tracks_telemetry -- --nocapture
run_cmd cargo test -p psionic-train --lib fixed_budget_training_loop_can_restore_from_latest_durable_checkpoint -- --nocapture
run_cmd cargo test -p psionic-train --lib fixed_budget_training_loop_refuses_missing_gradients -- --nocapture
run_cmd cargo test -p psionic-train --lib -- --nocapture

echo
echo "Psionic training core reference loop passed."
