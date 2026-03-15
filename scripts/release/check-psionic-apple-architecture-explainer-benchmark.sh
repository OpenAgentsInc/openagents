#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release/check-psionic-apple-architecture-explainer-benchmark.sh

Runs the canonical base-vs-adapter benchmark gate for the first real Apple
`Psionic architecture explainer` run from issue #3653.
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

run_cmd cargo test -p psionic-eval apple_adapter_benchmark -- --nocapture
run_cmd cargo test -p psionic-data apple_adapter_curation -- --nocapture
run_cmd scripts/lint/ownership-boundary-check.sh

echo
echo "Psionic Apple architecture explainer benchmark gate passed."
