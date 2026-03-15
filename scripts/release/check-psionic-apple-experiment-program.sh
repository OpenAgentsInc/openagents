#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release/check-psionic-apple-experiment-program.sh

Runs the canonical experiment-program validation for the first real Apple
`Psionic architecture explainer` iteration tracked in issue #3656.
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

run_cmd cargo test -p psionic-train apple_adapter_experiment -- --nocapture
run_cmd cargo test -p psionic-eval apple_adapter_benchmark -- --nocapture
run_cmd scripts/lint/ownership-boundary-check.sh

echo
echo "Psionic Apple experiment program gate passed."
