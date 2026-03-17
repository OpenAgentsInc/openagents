#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release/check-psionic-mlx-cpu-reference-coverage.sh [options]

Runs the canonical Psionic MLX CPU-reference coverage report.

Options:
  --only <family>   Include one named family (repeatable)
  --report <path>   Write the machine-readable JSON report
  --help            Show this help

Families:
  array_core
  ops_numeric
  device_eval_memory
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

run_cmd cargo test -p psionic-array \
  tests::mlx_cpu_reference_coverage_report_tracks_seeded_supported_and_refused_cases \
  -- --exact --nocapture
run_cmd cargo run --quiet -p psionic-array --example mlx_cpu_reference_coverage_report -- "$@"
run_cmd scripts/lint/ownership-boundary-check.sh

echo
echo "Psionic MLX CPU reference coverage passed."
