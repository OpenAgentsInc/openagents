#!/usr/bin/env bash
# Firm reason: 4 (bootstrap seed: regenerates a coax0 C seed with an already trusted compiler)
# Explicitly refresh the generated C seed using a previously trusted compiler.
# Normal builds never refresh a seed or silently accept a mismatch.
set -euo pipefail
cd "$(dirname "$0")"
compiler=${1:-bin/coax}
work=$(mktemp -d "${TMPDIR:-/tmp}/coax-checker-seed.XXXXXX")
"$compiler" test lib/compiler/checker.coax
"$compiler" mutate lib/compiler/checker.coax
"$compiler" c lib/compiler/checker.coax --library --header "$work/checker.h" > "$work/checker.c"
mv "$work/checker.c" bootstrap/checker.c
mv "$work/checker.h" bootstrap/checker.h
rmdir "$work"
