#!/usr/bin/env bash
# Firm reason: 4 (bootstrap seed: regenerates a coax0 C seed with an already trusted compiler)
# Explicitly refresh the generated C seed using a previously trusted compiler.
# Normal builds never refresh a seed or silently accept a mismatch.
set -euo pipefail
cd "$(dirname "$0")"
compiler=${1:-bin/coax}
work=$(mktemp -d "${TMPDIR:-/tmp}/coax-checker-seed.XXXXXX")
"$compiler" test lib/compiler/checker.coax
# The bootstrap gate's killing checks (bootstrap-build.sh invoke_gate): each
# mutant tries its last killer first. The verdicts are the same without them.
killers="${COAX_GATE_CACHE:-$HOME/.cache/coax-gates}/killers"
if mkdir -p "$killers" 2> /dev/null; then
  "$compiler" mutate lib/compiler/checker.coax --killers "$killers/$(printf '%s\0' lib/compiler/checker.coax | { shasum -a 256 2> /dev/null || sha256sum; } | awk '{print $1}')"
else
  "$compiler" mutate lib/compiler/checker.coax
fi
"$compiler" c lib/compiler/checker.coax --library --header "$work/checker.h" > "$work/checker.c"
mv "$work/checker.c" bootstrap/checker.c
mv "$work/checker.h" bootstrap/checker.h
rmdir "$work"
