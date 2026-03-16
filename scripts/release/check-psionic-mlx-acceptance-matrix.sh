#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release/check-psionic-mlx-acceptance-matrix.sh [options]

Runs the canonical Psionic MLX acceptance matrix.

Options:
  --only <category>   Include one named category (repeatable)
  --report <path>     Write the machine-readable JSON report
  --help              Show this help

Categories:
  array-runtime-surface
  transform-compile
  nn-optimizer
  export-serialization-tooling
  distributed-semantics
  backend-closure
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
  tests::mlx_acceptance_matrix_report_declares_all_named_closure_categories_and_filtering \
  -- --exact --nocapture
run_cmd cargo run --quiet -p psionic-compat --example mlx_acceptance_matrix_report -- "$@"
run_cmd scripts/lint/ownership-boundary-check.sh

echo
echo "Psionic MLX acceptance matrix passed."
