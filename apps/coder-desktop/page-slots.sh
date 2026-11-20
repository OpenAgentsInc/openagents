#!/bin/bash
# Firm reason: 5 (thin wrapper around coax test; holds no logic)
# The page's checks under 85% of the compiler's value slots (#13767). Each check file under
# page/checks/ runs in the evaluator (COAX_NATIVE_CHECKS=0), as a run of one check does, with the
# slot limit lowered to 85% of the checks' 33,554,432: a check that needs more fails with E0400 and
# names what holds its slots. A file above 75% of that also prints the compiler's near-limit note.
# Only slots fail it: a check past its steps in the evaluator is listed as a note.
#   apps/coder-desktop/page-slots.sh              every check file
#   apps/coder-desktop/page-slots.sh FILE...      these
set -u
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
coax="${COAX:-$repo/coax/bin/coax}"
limit=$((33554432 * 85 / 100))
if [ $# -eq 0 ]; then set -- "$here"/page/checks/*-checks.coax; fi
failed=0
for f in "$@"; do
  out="$(COAX_NATIVE_CHECKS=0 "$coax" test "$f" --library --slots "$limit" 2>&1)"
  near="$(printf '%s\n' "$out" | grep -o 'value slots: [0-9]* of [0-9]* used ([0-9]*%)' | head -1)"
  if printf '%s\n' "$out" | grep -q 'value slots needed'; then
    failed=1
    echo "FAIL $(basename "$f"): a check needs more than $limit value slots"
    printf '%s\n' "$out" | grep -B1 -A9 'value slots needed' | head -40
  else
    echo "ok   $(basename "$f")${near:+ ($near)}"
    # a check past its steps in the evaluator (a heavy one runs as native code) is not this script's
    printf '%s\n' "$out" | grep '^FAIL' | sed 's/^/     note: /'
  fi
done
exit $failed
