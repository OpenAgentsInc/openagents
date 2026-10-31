#!/usr/bin/env bash
# coax/lib/bounded.coax: its checks, zero mutation survivors, and the two
# consumers' before/after fixtures. Each fixture's .expected is the output
# recorded from the consumer before it moved to Bounded (r4a-generic-values);
# a consumer that still prints the same is unchanged in what callers see.
#   coax/tests/bounded.sh [COMPILER]
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${1:-${COAX:-coax/bin/coax}}
"$coax" test coax/lib/bounded.coax --library
"$coax" mutate coax/lib/bounded.coax --library
out=$(mktemp -d "${TMPDIR:-/tmp}/coax-bounded.XXXXXX")
trap 'rm -rf "$out"' EXIT
fixture() {
  local source=$1 expected=$2
  "$coax" run "$source" > "$out/actual"
  if cmp -s "$out/actual" "$expected"; then echo "ok   $source matches $expected"; else
    echo "FAIL $source differs from $expected" >&2; diff "$expected" "$out/actual" | head -20 >&2; return 1; fi
}
fixture coax/tests/bounded/shadow.coax coax/tests/bounded/shadow.expected
fixture psionic/examples/shadow.coax coax/tests/bounded/shadow-example.expected
fixture coax/tests/bounded/forest.coax coax/tests/bounded/forest.expected
