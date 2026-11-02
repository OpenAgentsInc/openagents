#!/usr/bin/env bash
# Chapter 101: Maya's recipe for V4.1 Flash as a plan. The recipe's checks
# against the tensor-by-tensor byte count and the position marking, both
# transcripts in debug and release, and (with --mutate) the authority
# file's mutation check.
# Firm reason: 5 (a thin wrapper: runs coax test, coax run and coax mutate on chapter 101's files).
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
mutate=0;jobs=${COAX_MUTATE_JOBS:-8}
while [[ $# -gt 0 ]];do
    case "$1" in
        --mutate) mutate=1;shift ;;
        --jobs) jobs=$2;shift 2 ;;
        *) echo 'usage: check_deepseek_recipe.sh [--mutate] [--jobs N]' >&2;exit 2 ;;
    esac
done
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-recipe.XXXXXX");trap 'rm -rf "$out"' EXIT
for mode in debug release;do
    flags=()
    if [[ $mode == release ]];then flags+=(--release);fi
    "$coax" test psionic/lib/deepseek/recipe.coax "${flags[@]+"${flags[@]}"}"
    for example in deepseek-recipe deepseek-recipe-refuse;do
        "$coax" run "psionic/examples/$example.coax" "${flags[@]+"${flags[@]}"}" > "$out/$example.out"
        cmp "psionic/fixtures/deepseek-recipe/$example.out" "$out/$example.out"
    done
done
if [[ $mutate == 1 ]];then "$coax" mutate psionic/lib/deepseek/recipe.coax --root-only --jobs "$jobs";fi
echo "Maya's recipe: plan checks and both transcripts pass"
