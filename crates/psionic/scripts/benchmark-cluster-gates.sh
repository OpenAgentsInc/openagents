#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  crates/psionic/scripts/benchmark-cluster-gates.sh [--json-out DIR]

Defaults:
  json summaries: disabled

Notes:
  - Runs the ignored `cluster_benchmark_gates` release tests for `psionic-cluster`.
  - Use env vars such as `PSIONIC_CLUSTER_BENCH_WHOLE_REQUEST_MAX_MS` to override budgets.
  - When `--json-out DIR` is set, each benchmark writes one summary JSON file into DIR.
EOF
}

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd -- "$SCRIPT_DIR/../../.." && pwd)
JSON_OUT=

while [[ $# -gt 0 ]]; do
  case "$1" in
    --json-out)
      JSON_OUT=${2:?missing value for --json-out}
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unrecognized argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ -n "$JSON_OUT" ]]; then
  mkdir -p "$JSON_OUT"
  export PSIONIC_CLUSTER_BENCH_JSON_OUT="$JSON_OUT"
fi

echo "repo_root=$REPO_ROOT"
echo "running=psionic-cluster release benchmark gates"
echo "whole_request_max_ms=${PSIONIC_CLUSTER_BENCH_WHOLE_REQUEST_MAX_MS:-2500}"
echo "recovery_max_ms=${PSIONIC_CLUSTER_BENCH_RECOVERY_MAX_MS:-2500}"
echo "replicated_max_ms=${PSIONIC_CLUSTER_BENCH_REPLICATED_MAX_MS:-4000}"
echo "layer_max_ms=${PSIONIC_CLUSTER_BENCH_LAYER_MAX_MS:-4000}"
echo "tensor_max_ms=${PSIONIC_CLUSTER_BENCH_TENSOR_MAX_MS:-4000}"

cd "$REPO_ROOT"
cargo test -p psionic-cluster --release --test cluster_benchmark_gates -- --ignored --nocapture --test-threads=1

if [[ -n "$JSON_OUT" ]]; then
  echo "benchmark_json_out=$JSON_OUT"
fi
