#!/usr/bin/env bash
# The Coder harness acceptance gate (#11549): a real chat on every harness
# installed here, on the exact Coder build under test, before that build
# reaches a release or the owner's `coder`/`coderrc`. docs/coder/harness-acceptance.md
# is the runbook.
#
#   scripts/coder/harness-acceptance.sh [--coder PATH] [--out DIR] [--only a,b]
#                                       [--jobs N] [--keep] [--list]
#
# --coder PATH   the Coder binary to test (default ~/.openagents/bin/coder)
# --out DIR      reports go to DIR/<version>-<commit>/report.{md,json}
#                (default ~/work/harness-acceptance)
# --only KEYS    run only these harnesses (--list prints them)
# --jobs N       harnesses run at once (default 3)
# --keep         keep the isolated homes under /tmp/hacc.*
#
# Exit 0 when nothing failed (a harness that isn't installed, or the Coder
# loop without a gate sign-in key, is skipped, not passed), 1 on a failing
# check, 2 on a setup error.
set -uo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
[ "$(uname -s)" = Darwin ] || { echo "harness-acceptance: runs on a Mac" >&2; exit 2; }
command -v tmux >/dev/null || { echo "harness-acceptance: tmux is required (brew install tmux)" >&2; exit 2; }
exec python3 -I "$here/harness_acceptance.py" "$@"
