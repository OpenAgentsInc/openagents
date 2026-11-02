#!/usr/bin/env bash
# Focused CNN integration gate; never downloads data or starts training.
set -euo pipefail
cd "$(dirname "$0")/../.."
compiler=${COAX:-$PWD/coax/bin/coax}
jobs=2
mutate=0
native_only=0
data_dir=
while [[ $# -gt 0 ]]; do
    case "$1" in
        --jobs) jobs=${2:?missing worker count}; [[ $jobs =~ ^[1-9][0-9]*$ ]] || exit 2; shift 2 ;;
        --mutate) mutate=1; shift ;;
        --native-only) native_only=1; shift ;;
        --data-dir) data_dir=${2:?missing data directory}; shift 2 ;;
        *) echo "usage: $0 [--jobs N] [--mutate] [--native-only] [--data-dir DIR]" >&2; exit 2 ;;
    esac
done
if [[ $native_only == 0 ]]; then
    for mode in debug release; do
        flags=(); if [[ $mode == release ]]; then flags+=(--release); fi
        "$compiler" test psionic/tests/cnn-core-checks.coax --library --only 'cnn core:' "${flags[@]}" --steps 4000000000
        "$compiler" test psionic/lib/function-layout.coax --library --only 'cnn function eligibility' "${flags[@]}" --run-steps 250000000
        "$compiler" test psionic/lib/module-cnn.coax --library --only CNN "${flags[@]}" --steps 4000000000
        "$compiler" test psionic/lib/cnn-classifier.coax --library --only 'CNN classifier' "${flags[@]}" --steps 4000000000
        "$compiler" test psionic/lib/cnn-classifier.coax --library --only 'CNN zero network' "${flags[@]}" --steps 4000000000
        "$compiler" test psionic/lib/cnn-policy.coax --library "${flags[@]}"
    done
    if [[ $mutate == 1 ]]; then
        # Entry-source mutation for the new public admission roots only.
        # Shared IR/autodiff changes have focused numerical/refusal checks;
        # full shared-engine mutation is omitted under the owner's fast gate.
        for unit in module-cnn cnn-policy; do
            "$compiler" mutate "psionic/lib/$unit.coax" --library --root-only --jobs "$jobs"
        done
    fi
fi
COAX="$compiler" psionic/tests/check_cnn_native.sh
if [[ -n $data_dir ]]; then
    output_dir=$(mktemp -d "${TMPDIR:-/tmp}/psionic-cnn-preflight.XXXXXX")
    # Retain even a failed preflight. No learning or network effects.
    COAX="$compiler" psionic/platform/cnn-build.sh "$output_dir/build" release 0
    "$output_dir/build/cnn" --oracle "$data_dir" psionic/fixtures/cnn/experiment.json "$output_dir/capture"
    python3 psionic/tests/cnn_reference.py verify-capture "$output_dir/capture" --data-dir "$data_dir" --out "$output_dir/comparison.json"
    echo "CNN original-data preflight retained: $output_dir"
fi
python3 psionic/tests/book.py
echo "CNN focused checks pass; no training requested"
