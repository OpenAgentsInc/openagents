#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  crates/psionic/scripts/benchmark-cluster-gates.sh [--json-out DIR]

Defaults:
  benchmark receipt JSON: disabled

Notes:
  - Runs the ignored `cluster_benchmark_gates` release tests for `psionic-cluster`.
  - Use env vars such as `PSIONIC_CLUSTER_BENCH_WHOLE_REQUEST_MAX_MS` to override budgets.
  - When `--json-out DIR` is set, each benchmark writes one typed benchmark receipt JSON file into DIR.
  - Stable receipt filenames are:
      whole_request_scheduler.json
      recovery_catchup.json
      replicated_serving.json
      pipeline_sharded_planner.json
      layer_sharded_planner.json
      tensor_sharded_planner.json
EOF
}

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd -- "$SCRIPT_DIR/../../.." && pwd)
JSON_OUT=
RECEIPT_IDS=(
  whole_request_scheduler
  recovery_catchup
  replicated_serving
  pipeline_sharded_planner
  layer_sharded_planner
  tensor_sharded_planner
)

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
echo "pipeline_max_ms=${PSIONIC_CLUSTER_BENCH_PIPELINE_MAX_MS:-5000}"
echo "layer_max_ms=${PSIONIC_CLUSTER_BENCH_LAYER_MAX_MS:-4000}"
echo "tensor_max_ms=${PSIONIC_CLUSTER_BENCH_TENSOR_MAX_MS:-4000}"

cd "$REPO_ROOT"
cargo test -p psionic-cluster --release --test cluster_benchmark_gates -- --ignored --nocapture --test-threads=1

if [[ -n "$JSON_OUT" ]]; then
  for benchmark_id in "${RECEIPT_IDS[@]}"; do
    receipt_path="$JSON_OUT/$benchmark_id.json"
    if [[ ! -f "$receipt_path" ]]; then
      echo "missing benchmark receipt: $receipt_path" >&2
      exit 1
    fi
    if ! grep -q "\"schema_version\"" "$receipt_path"; then
      echo "benchmark receipt missing schema_version: $receipt_path" >&2
      exit 1
    fi
    if ! grep -q "\"benchmark_id\": \"$benchmark_id\"" "$receipt_path"; then
      echo "benchmark receipt has unexpected benchmark_id: $receipt_path" >&2
      exit 1
    fi
    if ! grep -q "\"outcome\"" "$receipt_path"; then
      echo "benchmark receipt missing outcome: $receipt_path" >&2
      exit 1
    fi
  done
  echo "benchmark_receipt_json_out=$JSON_OUT"
  for benchmark_id in "${RECEIPT_IDS[@]}"; do
    echo "benchmark_receipt=$JSON_OUT/$benchmark_id.json"
  done
fi
