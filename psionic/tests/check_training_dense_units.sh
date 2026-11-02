#!/usr/bin/env bash
# Only the dense primitive: no data acquisition, model training or GPU launch.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
jobs=2;mutate=0;out=
while [[ $# -gt 0 ]];do
    case "$1" in
        --jobs) jobs=${2:?};[[ $jobs =~ ^[1-9][0-9]*$ ]] || exit 2;shift 2;;
        --mutate) mutate=1;shift;;
        --out) out=${2:?};shift 2;;
        *) echo 'usage: check_training_dense_units.sh [--jobs N] [--mutate] [--out NEW_DIR]' >&2;exit 2;;
    esac
done
if [[ -z $out ]];then out=$(mktemp -d "${TMPDIR:-/tmp}/training-dense-checks.XXXXXXXX");else mkdir "$out";fi
out=$(cd "$out" && pwd)
for unit in device/training-dense device/training-dense-kernels;do
    "$coax" test "psionic/lib/$unit.coax" --library
    "$coax" test "psionic/lib/$unit.coax" --library --release
    if [[ $mutate == 1 ]];then "$coax" mutate "psionic/lib/$unit.coax" --library --root-only --jobs "$jobs";fi
done
COAX="$coax" bash psionic/tests/check_training_dense.sh "$out/native"
