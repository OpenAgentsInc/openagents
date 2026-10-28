#!/usr/bin/env bash
# Explicitly refresh the generated C seed using a previously trusted compiler.
# Normal builds never refresh a seed or silently accept a mismatch.
set -euo pipefail
cd "$(dirname "$0")"
compiler=${1:-bin/coax}
work=$(mktemp -d "${TMPDIR:-/tmp}/coax-lexer-seed.XXXXXX")
"$compiler" test lib/compiler/lexer.coax
"$compiler" mutate lib/compiler/lexer.coax
"$compiler" c lib/compiler/lexer.coax --library --header "$work/lexer.h" > "$work/lexer.c"
mv "$work/lexer.c" bootstrap/lexer.c
mv "$work/lexer.h" bootstrap/lexer.h
rmdir "$work"
