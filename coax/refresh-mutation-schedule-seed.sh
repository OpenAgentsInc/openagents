#!/usr/bin/env bash
# Refresh the checked mutation scheduling policy seed with a trusted compiler.
set -euo pipefail
cd "$(dirname "$0")"
compiler=${1:-bin/coax}
work=$(mktemp -d "${TMPDIR:-/tmp}/coax-mutation-schedule.XXXXXX")
trap 'find "$work" -depth -delete' EXIT
"$compiler" test lib/compiler/mutation-schedule.coax
"$compiler" mutate lib/compiler/mutation-schedule.coax
"$compiler" c lib/compiler/mutation-schedule.coax --library --header "$work/mutation-schedule.h" > "$work/mutation-schedule.c"
mv "$work/mutation-schedule.c" bootstrap/mutation-schedule.c
mv "$work/mutation-schedule.h" bootstrap/mutation-schedule.h
