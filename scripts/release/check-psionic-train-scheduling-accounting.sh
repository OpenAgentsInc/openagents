#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release/check-psionic-train-scheduling-accounting.sh

Runs the canonical Psionic train scheduling/accounting harness for #3591.
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

run_cmd cargo test -p psionic-train scheduling_accounting -- --nocapture
run_cmd cargo test -p psionic-runtime runtime_dispatch_plan_batches_data_plane_work_and_reduces_cost -- --nocapture
run_cmd scripts/lint/ownership-boundary-check.sh

echo
echo "Psionic train scheduling and accounting passed."
