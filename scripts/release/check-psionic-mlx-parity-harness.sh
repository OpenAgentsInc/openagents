#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release/check-psionic-mlx-parity-harness.sh [options]

Runs the canonical Psionic MLX parity harness.

Options:
  --only <family>   Include one named family (repeatable)
  --report <path>   Write the machine-readable JSON report
  --help            Show this help

Families:
  array_core
  ops_numeric
  device_eval_memory
  autograd
  vmap_custom_vjp
  compile
  export_import
  nn_optimizers_quantized
  distributed
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
  tests::mlx_parity_harness_report_tracks_seeded_pass_refusal_and_unsupported_families \
  -- --exact --nocapture
run_cmd cargo run --quiet -p psionic-compat --example mlx_parity_harness_report -- "$@"
run_cmd scripts/lint/ownership-boundary-check.sh

echo
echo "Psionic MLX parity harness passed."
