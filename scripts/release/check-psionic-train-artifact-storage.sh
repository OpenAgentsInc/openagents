#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release/check-psionic-train-artifact-storage.sh

Runs the canonical Psionic train artifact-storage harness for #3590.
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

run_cmd cargo test -p psionic-train artifact_storage -- --nocapture
run_cmd scripts/lint/ownership-boundary-check.sh

echo
echo "Psionic train artifact storage passed."
