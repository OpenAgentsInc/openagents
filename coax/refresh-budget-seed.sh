#!/usr/bin/env bash
# Explicit budget-policy seed refresh with a previously trusted compiler.
set -euo pipefail
cd "$(dirname "$0")"
compiler=${1:-bin/coax}
work=$(mktemp -d "${TMPDIR:-/tmp}/coax-budget-seed.XXXXXX")
trap 'find "$work" -depth -delete' EXIT
"$compiler" test lib/compiler/budget.coax
"$compiler" mutate lib/compiler/budget.coax
"$compiler" c lib/compiler/budget.coax --library --header "$work/budget.h" > "$work/budget.c"
mv "$work/budget.c" bootstrap/budget.c
mv "$work/budget.h" bootstrap/budget.h
