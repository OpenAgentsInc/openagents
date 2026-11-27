#!/bin/bash
# Firm reason: 5 (thin wrapper around coax test; holds no logic)
# The page's checks under 85% of the compiler's value slots (#13767). Each check file under
# page/checks/, and evals/openagents-desktop/tier1/replay_test.coax, runs in the evaluator
# (COAX_NATIVE_CHECKS=0), as a run of one check does, with the slot limit lowered to 85% of the
# checks' 33,554,432: a check that needs more fails with E0400 and names what holds its slots. A
# file above 75% of that also prints the compiler's near-limit note.
# Only slots fail it: a check past its steps in the evaluator is listed as a note.
#   apps/coder-desktop/page-slots.sh              every check file
#   apps/coder-desktop/page-slots.sh FILE...      these
set -u
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
coax="${COAX:-$repo/coax/bin/coax}"
limit=$((33554432 * 85 / 100))
# Tier 1 replay_test.coax loads page.coax, which already loads and runs the page's check files;
# add only page/checks/*-checks.coax files not loaded by page.coax so shared library checks do not
# re-run 45 times in the evaluator while keeping 100% check coverage under the 85% slot ceiling.
if [ $# -eq 0 ]; then
  files=("$repo/evals/openagents-desktop/tier1/replay_test.coax")
  for f in "$here"/page/checks/*-checks.coax; do
    grep -qF "page/checks/$(basename "$f")" "$here/page.coax" || files+=("$f")
  done
  set -- "${files[@]}"
fi
# Files run side by side, PAGE_SLOTS_JOBS at once (default: half the cores, at most 8); each is one
# evaluator process. The report comes in the files' order.
cores="$(sysctl -n hw.ncpu 2>/dev/null || nproc 2>/dev/null || echo 2)"
jobs="${PAGE_SLOTS_JOBS:-$(( cores / 2 > 8 ? 8 : (cores / 2 < 1 ? 1 : cores / 2) ))}"
tmp="$(mktemp -d "${TMPDIR:-/tmp}/page-slots.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT
one() {   # one check file: its report into $tmp/N.txt, and $tmp/N.failed when it needs too many slots
  local n="$1" f="$2" out near
  out="$(COAX_NATIVE_CHECKS=0 "$coax" test "$f" --library --slots "$limit" 2>&1)"
  near="$(printf '%s\n' "$out" | grep -o 'value slots: [0-9]* of [0-9]* used ([0-9]*%)' | head -1)"
  {
    if printf '%s\n' "$out" | grep -q 'value slots needed'; then
      touch "$tmp/$n.failed"
      echo "FAIL $(basename "$f"): a check needs more than $limit value slots"
      printf '%s\n' "$out" | grep -B1 -A9 'value slots needed' | head -40
    else
      echo "ok   $(basename "$f")${near:+ ($near)}"
      # a check past its steps in the evaluator (a heavy one runs as native code) is not this script's
      printf '%s\n' "$out" | grep '^FAIL' | sed 's/^/     note: /'
    fi
  } > "$tmp/$n.txt"
}
n=0
for f in "$@"; do
  while [ "$(jobs -rp | wc -l)" -ge "$jobs" ]; do sleep 0.2; done
  one "$n" "$f" &
  n=$((n + 1))
done
wait
failed=0
for i in $(seq 0 $((n - 1))); do
  cat "$tmp/$i.txt"
  [ -e "$tmp/$i.failed" ] && failed=1
done
exit $failed
