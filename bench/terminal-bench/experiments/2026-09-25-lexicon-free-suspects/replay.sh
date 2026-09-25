#!/usr/bin/env bash
# Replay the lexicon-free suspects measurement (issue #9652) from the
# recorded Jev answers, with no live call.
#
# 1. build.py rebuilds the untouched and fixed workspaces from the
#    Terminal-Bench task cache, writes one fixture per task and arm, copies
#    each fixture's recorded answers from recorded/, and recomputes
#    labels.json.
# 2. The evidence.departures suite runs every fixture with --jev recorded.
#    A recorded answer is keyed by the digest of the state and the question
#    set, so a changed candidate list or question misses instead of
#    replaying a stale answer.
# 3. analyze.py scores both arms and writes records/results.json.
#
# Usage: replay.sh [OUT_DIR]   (default ~/.openagents/coder-one/lexicon-free-suspects)
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../../../.." && pwd)"
out="${1:-$HOME/.openagents/coder-one/lexicon-free-suspects}"

python3 "$here/build.py" --out "$out"
cargo run -q --manifest-path "$root/Cargo.toml" -p coder-one -- \
  component suite evidence.departures \
  --fixtures "$out/fixtures" --jev recorded --no-record \
  --export "$out/recorded-export.json" > /dev/null
python3 "$here/analyze.py" "$out/recorded-export.json" --workspaces "$out/workspaces"
