#!/usr/bin/env bash
# Explicit original-data run; no download, Python runtime or silent training.
set -euo pipefail
cd "$(dirname "$0")/.."
[[ $# == 3 && ( $1 == --oracle || $1 == --train ) ]] || {
    echo 'usage: psionic/run-cnn.sh --oracle|--train DATA_DIR NEW_OUTPUT_DIR' >&2; exit 2;
}
mode=$1
data_dir=$2
output_dir=$3
[[ ! -e $output_dir ]] || { echo 'CNN: preserve prior results; choose a new output directory' >&2; exit 2; }
mkdir -p "$output_dir"
output_dir=$(cd "$output_dir" && pwd)
psionic/platform/cnn-build.sh "$output_dir/build" release 0
if [[ $mode == --train ]]; then
    "$output_dir/build/cnn" --oracle "$data_dir" psionic/fixtures/cnn/experiment.json "$output_dir/preflight"
    python3 psionic/tests/cnn_reference.py verify-capture "$output_dir/preflight" --data-dir "$data_dir" --out "$output_dir/preflight.json"
fi
"$output_dir/build/cnn" "$mode" "$data_dir" psionic/fixtures/cnn/experiment.json "$output_dir/run"
if [[ $mode == --oracle ]]; then
    python3 psionic/tests/cnn_reference.py verify-capture "$output_dir/run" --data-dir "$data_dir" --out "$output_dir/preflight.json"
fi
