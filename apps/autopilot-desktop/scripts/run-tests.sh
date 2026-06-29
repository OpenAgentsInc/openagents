#!/usr/bin/env bash
# #5026: `bun test` over the full Autopilot Desktop suite wedges at module-graph
# load when all test files load into one process at once. Both halves of the
# suite pass on their own (108 + 77 = 185 tests), and every file passes
# individually — but the full set hangs before any test runs (a bun load-time
# deadlock; the offending file pair spans the halves, so naive alphabetical
# chunking can re-trigger it). Running each file in its own `bun test` process
# sidesteps the hang while still covering the whole suite, and auto-includes new
# test files. Root cause of the bun load-deadlock is tracked in #5026 as a
# follow-up; this runner is the practical fix so `bun run test` is reliable.
set -uo pipefail
cd "$(dirname "$0")/.."

fail=0
failed=()
for f in tests/*.test.ts; do
  echo "== $f =="
  if ! bun test "$f"; then
    fail=1
    failed+=("$f")
  fi
done

echo
if [ "$fail" -ne 0 ]; then
  echo "FAILED files:"
  printf '  %s\n' "${failed[@]}"
else
  echo "All desktop test files passed (per-file runner — avoids the #5026 bun load-hang)."
fi
exit "$fail"
