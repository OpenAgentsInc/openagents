#!/usr/bin/env bash
# Firm reason: 5. The nightly mutation sweep's entry (#13583): git, coax/build.sh and
# coax/web/tool.sh mutation-sweep, which holds every rule (coax/web/mutation-sweep.coax).
# It keeps its own checkout of origin/main (OA_SWEEP_DIR, default
# ~/.openagents/mutation-sweep/checkout), moves it to origin/main's tip, builds the compiler
# there and runs the sweep from it. The sweep's state (each part's killers file between
# nights, the report, the tracking issue's number) is that checkout's untracked
# mutation-sweep/ folder, which the move keeps. The parts run on the build pool through
# coax/remote/remote.sh, inside its spend cap (OA_REMOTE_DAILY_USD) and host count.
#   coax/remote/mutation-sweep.sh [ARGS...]   ARGS pass to the sweep (--dry-run, --only, --parts, --jobs)
# Scheduled nightly by a timer (docs/metaverse/remote-builds.md, "The nightly mutation sweep").
set -euo pipefail
here="$(cd "$(dirname "$0")/../.." && pwd -P)"
dir="${OA_SWEEP_DIR:-$HOME/.openagents/mutation-sweep/checkout}"
log="${dir%/*}/sweep.log"
mkdir -p "${dir%/*}"
if [[ ! -d "$dir/.git" && ! -f "$dir/.git" ]]; then
  git -C "$here" fetch -q origin main
  git -C "$here" worktree add -q --detach "$dir" origin/main
fi
cd "$dir"
git fetch -q origin main
git checkout -q --detach origin/main
{ echo "== $(date '+%Y-%m-%d %H:%M %Z') sweep at $(git rev-parse --short HEAD)"; } >> "$log"
coax/build.sh >> "$log" 2>&1
coax/web/tool.sh mutation-sweep "$@" 2>&1 | tee -a "$log"
