#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release/check-psionic-mlx-compatibility-matrix.sh [options]

Runs the canonical Psionic MLX compatibility matrix.

Options:
  --only <surface>   Include one named surface (repeatable)
  --report <path>    Write the machine-readable JSON report
  --help             Show this help

Surfaces:
  governance_contracts
  seeded_transform_compile_export_parity_anchors
  graph_first_function_export_bridge
  portable_model_io_bridge
  module_state_tree_bridge
  public_mlx_array_api
  public_mlx_transform_api
  public_mlx_nn_optimizer_api
  mlxfn_interop
  mlx_naming_facade_and_bindings
  public_mlx_distributed_api
  mlx_package_ecosystem
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
  tests::mlx_compatibility_matrix_report_tracks_supported_convertible_and_unsupported_rows \
  -- --exact --nocapture
run_cmd cargo run --quiet -p psionic-compat --example mlx_compatibility_matrix_report -- "$@"
run_cmd scripts/lint/ownership-boundary-check.sh

echo
echo "Psionic MLX compatibility matrix passed."
