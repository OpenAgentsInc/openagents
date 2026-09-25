#!/usr/bin/env bash
# Replays the metric-target extraction from the recorded Jev answers
# (issue #9657). No live Jev request is made: `--jev recorded` answers only
# from the recorded file, and a request it doesn't hold comes back
# unanswered.
#
# It replays the second extraction (2026-09-25, later the same day): each
# run's answers are in records/jev-recorded-extraction-v2.json, and each
# run's summary is under records/extraction-v2-*/. The frozen wording is
# questions-v1.json; the current wording is the embedded question set.
#
# The first extraction's answers (records/jev-recorded.json, with
# records/summary-v1.json and records/summary.json) were asked with the
# numbers the first extraction code found, so they replay only at the
# commit that measured them, 68aa85cf63.
#
# Usage: replay.sh [ROOT]   (ROOT: the checkout, default: here)
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="${1:-$(cd "$here/../../../.." && pwd)}"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

status=0
replay() {
  local run="$1"
  shift
  cp "$here/records/jev-recorded-extraction-v2.json" "$scratch/jev-recorded.json"
  (cd "$root" && cargo run -q -p coder-one -- checks metric-target offline \
    --labels "$here/labels.json" --out "$scratch/$run" --jev recorded \
    --recorded "$scratch/jev-recorded.json" "$@" >/dev/null)
  if diff <(jq -S '.totals | del(.jev_usd_recorded)' "$here/records/$run/summary.json") \
          <(jq -S '.totals | del(.jev_usd_recorded)' "$scratch/$run/summary.json"); then
    echo "replay matches records/$run/summary.json"
  else
    echo "replay differs from records/$run/summary.json" >&2
    status=1
  fi
}

replay extraction-v2-frozen --questions "$here/questions-v1.json" --host-limit task
replay extraction-v2-frozen-no-limit --questions "$here/questions-v1.json" --host-limit none
replay extraction-v2-current --host-limit task
exit "$status"
