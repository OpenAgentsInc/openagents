#!/usr/bin/env bash
# Replays the metric-target extraction from the recorded Jev answers
# (issue #9657). No live Jev request is made: `--jev recorded` answers only
# from records/jev-recorded.json, and a request it doesn't hold comes back
# unanswered.
#
# The current question set is the second version, so the replay reproduces
# records/summary.json. The first version's answers are in the same file;
# to replay them, check out questions/metric-target.json from the commit
# that froze the protocol and compare with records/summary-v1.json.
#
# Usage: replay.sh [ROOT]   (ROOT: the checkout, default: here)
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="${1:-$(cd "$here/../../../.." && pwd)}"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

cp "$here/records/jev-recorded.json" "$scratch/jev-recorded.json"
(cd "$root" && cargo run -q -p coder-one -- checks metric-target offline \
  --labels "$here/labels.json" --out "$scratch" --jev recorded \
  --recorded "$scratch/jev-recorded.json")
if diff <(jq -S '.totals | del(.jev_usd_recorded)' "$here/records/summary.json") \
        <(jq -S '.totals | del(.jev_usd_recorded)' "$scratch/summary.json"); then
  echo "replay matches records/summary.json"
else
  echo "replay differs from records/summary.json" >&2
  exit 1
fi
