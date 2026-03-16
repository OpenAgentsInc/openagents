#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release/check-psionic-advanced-operator-programs.sh

Runs the canonical Psionic advanced-operator harness.
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

run_cmd cargo test -p psionic-ir advanced_operator_program_matrix_tracks_supported_and_refused_families -- --nocapture
run_cmd cargo test -p psionic-compat semantics_claim_report_marks_seeded_evidence_and_future_compatibility_targets -- --nocapture
run_cmd scripts/lint/ownership-boundary-check.sh

echo
echo "Psionic advanced operator programs passed."
