#!/usr/bin/env bash
# Focused residual route; never downloads data or starts an optimizer run.
set -euo pipefail
cd "$(dirname "$0")/../.."
compiler=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-${CC:-cc}}
jobs=2;mutate=0;out=;data_dir=;native_only=0
while [[ $# -gt 0 ]];do
  case "$1" in
    --jobs) jobs=${2:?};[[ $jobs =~ ^[1-9][0-9]*$ ]] || exit 2;shift 2;;
    --mutate) mutate=1;shift;;
    --native-only) native_only=1;shift;;
    --out) out=${2:?};shift 2;;
    --data-dir) data_dir=${2:?};shift 2;;
    *) echo 'usage: check_residual.sh [--jobs N] [--mutate] [--native-only] [--out NEW_DIR] [--data-dir PINNED_CACHE]' >&2;exit 2;;
  esac
done
if [[ -z $out ]];then out=$(mktemp -d "${TMPDIR:-/tmp}/residual-checks.XXXXXXXX");else mkdir "$out";fi
out=$(cd "$out" && pwd)
if [[ $native_only == 0 ]];then
for mode in debug release;do
  flags=();if [[ $mode == release ]];then flags+=(--release);fi
  "$compiler" test psionic/lib/module-residual.coax --library --only 'residual modules:' "${flags[@]}"
  "$compiler" test psionic/lib/residual-classifier.coax --library --only 'residual CNN classifier' "${flags[@]}"
  "$compiler" test psionic/lib/residual-mlp-classifier.coax --library --only 'residual MLP-head CNN classifier' "${flags[@]}"
  for unit in residual-policy residual-mlp-policy;do
    "$compiler" test "psionic/lib/$unit.coax" --library "${flags[@]}"
  done
done
if [[ $mutate == 1 ]];then
  for unit in module-residual residual-policy residual-mlp-policy;do
    "$compiler" mutate "psionic/lib/$unit.coax" --library --root-only --jobs "$jobs"
  done
fi
fi
COAX="$compiler" bash psionic/tests/check_residual_native.sh "$out/native"
for mode in debug release sanitized;do
  emission=$mode;sanitize=0;if [[ $mode == sanitized ]];then emission=release;sanitize=1;fi
  COAX="$compiler" bash psionic/platform/residual-mlp-build.sh "$out/$mode" "$emission" "$sanitize"
  ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/$mode/residual" --synthetic
  if [[ -n $data_dir ]];then
    ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/$mode/residual" --oracle "$data_dir" psionic/fixtures/residual-mlp/experiment.json "$out/$mode/capture"
    python3 psionic/tests/residual_mlp_reference.py verify-capture "$out/$mode/capture" --data-dir "$data_dir" --out "$out/$mode/comparison.json"
  fi
done
"$cc" -std=c11 -Wall -Wextra -Werror -fsyntax-only -I"$out/release" psionic/tests/residual-mlp-preflight.c
python3 psionic/tests/residual_launch_test.py
python3 psionic/tests/residual_gate_routing.py
if [[ $native_only == 0 ]];then python3 psionic/tests/book.py;fi
echo "Residual checks retained: $out (no dataset or learning run)"
