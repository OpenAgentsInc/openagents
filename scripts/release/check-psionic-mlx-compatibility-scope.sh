#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release/check-psionic-mlx-compatibility-scope.sh

Runs the canonical Psionic MLX compatibility-scope check.
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

run_cmd cargo test -p psionic-compat \
  tests::mlx_compatibility_scope_report_freezes_upstream_version_window_and_claim_vocabulary \
  -- --exact --nocapture
run_cmd scripts/lint/ownership-boundary-check.sh

echo
echo "Psionic MLX compatibility scope passed."
