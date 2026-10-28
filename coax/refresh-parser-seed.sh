#!/usr/bin/env bash
# Explicitly refresh the generated C seed using a previously trusted compiler.
# Normal builds never refresh a seed or silently accept a mismatch.
set -euo pipefail
cd "$(dirname "$0")"
compiler=${1:-bin/coax}
work=$(mktemp -d "${TMPDIR:-/tmp}/coax-parser-seed.XXXXXX")
"$compiler" test lib/compiler/parser.coax
"$compiler" mutate lib/compiler/parser.coax
"$compiler" c lib/compiler/parser.coax --library --header "$work/parser.h" > "$work/parser.c"
mv "$work/parser.c" bootstrap/parser.c
mv "$work/parser.h" bootstrap/parser.h
rmdir "$work"
