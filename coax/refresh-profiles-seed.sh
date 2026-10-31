#!/usr/bin/env bash
# Firm reason: 4 (bootstrap seed: regenerates a coax0 C seed with an already trusted compiler)
# Explicit semantic-profile policy seed refresh with a previously trusted compiler.
set -euo pipefail
cd "$(dirname "$0")"
compiler=${1:-bin/coax}
work=$(mktemp -d "${TMPDIR:-/tmp}/coax-profiles-seed.XXXXXX")
trap 'find "$work" -depth -delete' EXIT
"$compiler" test lib/compiler/profiles.coax
"$compiler" mutate lib/compiler/profiles.coax
"$compiler" c lib/compiler/profiles.coax --library --header "$work/profiles.h" > "$work/profiles.c"
mv "$work/profiles.c" bootstrap/profiles.c
mv "$work/profiles.h" bootstrap/profiles.h
