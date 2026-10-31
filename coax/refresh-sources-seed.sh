#!/usr/bin/env bash
# Firm reason: 4 (bootstrap seed: regenerates a coax0 C seed with an already trusted compiler)
# Explicit source-unit kernel refresh with a previously trusted compiler.
set -euo pipefail
cd "$(dirname "$0")"
compiler=${1:-bin/coax}
work=$(mktemp -d "${TMPDIR:-/tmp}/coax-sources-seed.XXXXXX")
trap 'find "$work" -depth -delete' EXIT
"$compiler" test lib/compiler/sources.coax
"$compiler" mutate lib/compiler/sources.coax
"$compiler" c lib/compiler/sources.coax --library --header "$work/sources.h" > "$work/sources.c"
mv "$work/sources.c" bootstrap/sources.c
mv "$work/sources.h" bootstrap/sources.h
