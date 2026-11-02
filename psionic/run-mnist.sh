#!/usr/bin/env bash
# Explicit original-data run; no download, Python runtime or silent training.
set -euo pipefail
cd "$(dirname "$0")/.."
[[ $# == 3 && ( $1 == --oracle || $1 == --train ) ]] || {
    echo 'usage: psionic/run-mnist.sh --oracle|--train DATA_DIR NEW_OUTPUT_DIR' >&2; exit 2;
}
mode=$1
data_dir=$2
output_dir=$3
[[ ! -e $output_dir ]] || { echo 'MNIST: preserve prior results; choose a new output directory' >&2; exit 2; }
mkdir -p "$output_dir"
output_dir=$(cd "$output_dir" && pwd)
psionic/platform/mnist-build.sh "$output_dir/build" release 0
"$output_dir/build/mnist" "$mode" "$data_dir" psionic/fixtures/mnist/experiment.json "$output_dir/run"
