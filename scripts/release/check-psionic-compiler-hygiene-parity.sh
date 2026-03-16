#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release/check-psionic-compiler-hygiene-parity.sh

Runs the seeded Psionic symbolic-shape, fake-tensor, and compiler-hygiene
parity matrix harness.
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

run_cmd cargo test -p psionic-compiler compiler_hygiene_parity_matrix_tracks_seeded_supported_and_refusal_cases -- --nocapture
run_cmd cargo test -p psionic-compiler --test process_replay -- --nocapture
run_cmd scripts/lint/ownership-boundary-check.sh

echo
echo "Psionic compiler-hygiene parity matrix passed."
