#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release/check-psionic-apple-runtime-validation.sh

Runs the canonical Apple bridge-backed runtime validation and drift-check gate
for the first real `Psionic architecture explainer` adapter path from issue
#3657.
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

run_cmd cargo test -p psionic-eval apple_adapter_runtime -- --nocapture
run_cmd cargo check -p autopilot-desktop
run_cmd scripts/lint/ownership-boundary-check.sh

echo
echo "Psionic Apple runtime validation gate passed."
