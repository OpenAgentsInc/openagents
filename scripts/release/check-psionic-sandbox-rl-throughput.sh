#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release/check-psionic-sandbox-rl-throughput.sh

Runs the canonical Psionic sandbox RL-throughput harness for #3579.
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

run_cmd cargo test -p psionic-sandbox pool::tests::sandbox_pool_ -- --nocapture
run_cmd scripts/lint/ownership-boundary-check.sh

echo
echo "Psionic sandbox RL-throughput checks passed."
