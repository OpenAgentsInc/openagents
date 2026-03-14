#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release/check-psionic-train-orchestrator.sh

Runs the canonical Psionic train orchestrator harness for #3573.
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

run_cmd cargo test -p psionic-train --lib orchestrator -- --nocapture
run_cmd scripts/lint/ownership-boundary-check.sh

echo
echo "Psionic train orchestrator passed."
