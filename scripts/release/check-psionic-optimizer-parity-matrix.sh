#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release/check-psionic-optimizer-parity-matrix.sh

Runs the seeded Psionic optimizer parity matrix harness.
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

run_cmd cargo test -p psionic-train optimizer_parity_matrix_report_tracks_seeded_supported_and_refusal_cases -- --nocapture
run_cmd scripts/lint/ownership-boundary-check.sh

echo
echo "Psionic optimizer parity matrix passed."
