#!/usr/bin/env bash
# Native MNIST consumer checks. No training or network access in this gate.
# Unit checks/mutation belong to psionic/check.sh; the data adapter's retained
# qualification is psionic/fixtures/mnist/data-qualification.json.
# Usage: check_mnist.sh [--data-dir DIR] [--manifest FILE] [--out DIR]
# With --data-dir, also compare the actual-data preflight with its frozen
# manifest. Otherwise run only synthetic guards, in debug/release/sanitized C.
set -euo pipefail
cd "$(dirname "$0")/../.."
data_dir=
manifest=$PWD/psionic/fixtures/mnist/experiment.json
output_dir=
unit_checks=0
jobs=2
while [[ $# -gt 0 ]]; do
    case "$1" in
        --data-dir) data_dir=${2:?missing data directory}; shift 2 ;;
        --manifest) manifest=${2:?missing manifest}; shift 2 ;;
        --out) output_dir=${2:?missing output directory}; shift 2 ;;
        --unit-checks) unit_checks=1; shift ;;
        --jobs) jobs=${2:?missing worker count}; [[ $jobs =~ ^[1-9][0-9]*$ ]] || exit 2; shift 2 ;;
        *) echo "usage: $0 [--data-dir DIR] [--manifest FILE] [--out DIR] [--unit-checks] [--jobs N]" >&2; exit 2 ;;
    esac
done
compiler=${COAX:-$PWD/coax/bin/coax}
if [[ $unit_checks == 1 ]]; then
    for unit in module-train data/mnist mnist-policy; do
        "$compiler" test "psionic/lib/$unit.coax" --library --steps 4000000000
        "$compiler" test "psionic/lib/$unit.coax" --library --release --steps 4000000000
        "$compiler" mutate "psionic/lib/$unit.coax" --library --root-only --jobs "$jobs"
    done
    for name in 'MNIST classifier' 'MNIST zero network'; do
        "$compiler" test psionic/lib/mnist-classifier.coax --library --only "$name" --steps 4000000000
        "$compiler" test psionic/lib/mnist-classifier.coax --library --release --only "$name" --steps 4000000000
    done
fi
if [[ -n $data_dir ]]; then
    cmp -s "$manifest" psionic/fixtures/mnist/experiment.json || { echo "MNIST gate requires the frozen experiment identity" >&2; exit 2; }
fi
temporary=0
if [[ -z $output_dir ]]; then
    output_dir=$(mktemp -d "${TMPDIR:-/tmp}/psionic-mnist-check.XXXXXX")
    temporary=1
fi
cleanup() { if [[ $temporary == 1 ]]; then rm -rf "$output_dir"; fi; }
trap cleanup EXIT
mkdir -p "$output_dir"
output_dir=$(cd "$output_dir" && pwd)
for mode in debug release sanitized; do
    build_mode=$mode
    sanitize=0
    if [[ $mode == sanitized ]]; then build_mode=debug; sanitize=1; fi
    work=$output_dir/$mode
    psionic/platform/mnist-build.sh "$work" "$build_mode" "$sanitize"
    ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 \
        "$work/mnist" --synthetic
    if [[ -n $data_dir ]]; then
        ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 \
            "$work/mnist" --oracle "$data_dir" "$manifest" "$work/oracle"
        python3 psionic/tests/mnist_reference.py verify-capture "$work/oracle" --data-dir "$data_dir" --out "$work/oracle-comparison.json"
    fi
done
python3 psionic/tests/book.py
echo "MNIST native consumer checks pass; no training was requested"
