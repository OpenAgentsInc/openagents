#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release/check-psionic-semantics-claim-report.sh

Runs the canonical Psionic semantics claim report harness.
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

run_cmd cargo test -p psionic-compat -- --nocapture
run_cmd scripts/lint/ownership-boundary-check.sh

echo
echo "Psionic semantics claim report passed."
