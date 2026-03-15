#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release/check-psionic-decentralized-adapter-reference-program.sh

Runs the canonical decentralized adapter reference-program and chaos harness for
#3648.
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

run_cmd cargo test -p psionic-train adapter_reference_program -- --nocapture
run_cmd scripts/lint/ownership-boundary-check.sh

echo
echo "Psionic decentralized adapter reference program passed."
