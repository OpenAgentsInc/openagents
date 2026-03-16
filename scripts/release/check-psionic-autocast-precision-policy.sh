#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release/check-psionic-autocast-precision-policy.sh

Runs the canonical Psionic autocast precision-policy harness.
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

run_cmd cargo test -p psionic-core autocast_policy_matrix_tracks_seeded_backend_rules_and_diagnostics -- --nocapture
run_cmd cargo test -p psionic-compat semantics_claim_report_marks_seeded_evidence_and_future_compatibility_targets -- --nocapture
run_cmd scripts/lint/ownership-boundary-check.sh

echo
echo "Psionic autocast precision policy passed."
