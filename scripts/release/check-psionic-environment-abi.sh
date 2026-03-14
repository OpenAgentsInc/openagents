#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release/check-psionic-environment-abi.sh

Runs the canonical Psionic environment ABI and runtime harness for #3566.
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

run_cmd cargo test -p psionic-environments --lib -- --nocapture
run_cmd cargo test -p psionic-train --lib rollout_artifacts_and_trainer_batch_are_machine_legible -- --nocapture

echo
echo "Psionic environment ABI passed."
