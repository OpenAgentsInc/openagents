#!/usr/bin/env bash
# Train a candidate filefind ranker on every fix up to now (issues #11210, #11231).
#
#   scripts/filefind/retrain.sh [WORK_DIR]
#
# 1. refreshes the indexes (incremental) and the run feedback,
# 2. rebuilds the issue -> fix dataset from main's history (closed issues),
# 3. embeds/tokenizes every replayed parent tree (cached per blob),
# 4. computes candidate features (cached per issue in WORK_DIR),
# 5. trains the two-stage ranker on the train cases only (never the eval cases)
#    and copies it to an immutable WORK_DIR/candidates/model-<digest>.json,
# 6. compares the active scripts/filefind/model.json with the candidate on the
#    held-out eval cases under the gate's frozen plan (ranker_gate.py) and writes
#    WORK_DIR/compare-<digest>.json.
#
# It never writes scripts/filefind/model.json. The active model changes only on an
# explicit promote, which refuses unless the comparison passed against the model
# that is active now:
#   python3 scripts/filefind/ranker_gate.py promote --candidate C --receipt R
# A candidate trained on any eval case is refused by compare and by eval.
# Takes about an hour on a laptop the first time; later runs reuse WORK_DIR's features.
set -euo pipefail
root="$(git rev-parse --show-toplevel)"
work="${1:-${HOME}/.cache/openagents/filefind/train}"
mkdir -p "$work"
ff="$root/scripts/filefind/filefind.py"
bench="$root/scripts/bench/file-finding-bench.py"
python3 "$ff" index --repo "$root" --rev HEAD
python3 "$ff" feedback --repo "$root"
gh issue list --state closed --limit 20000 --json number,title,body,closedAt,createdAt > "$work/closed-issues.json"
python3 -I "$root/scripts/bench/file-finding-dataset.py" --repo "$root" --rev HEAD \
  --issues "$work/closed-issues.json" --multi --max-hand 40 --out "$work/dataset.json"
python3 "$bench" prepare --repo "$root" --dataset "$work/dataset.json" --work "$work"
python3 "$bench" features --repo "$root" --dataset "$work/dataset.json" --work "$work"
python3 "$bench" train --repo "$root" --dataset "$work/dataset.json" --work "$work" --model "$work/model.json"
candidate="$(cd "$root/scripts/filefind" && python3 -c 'import sys, ranker_gate as g; print(g.immutable_copy(sys.argv[1], sys.argv[2]))' \
  "$work/model.json" "$work/candidates")"
set +e
python3 "$bench" compare --repo "$root" --dataset "$work/dataset.json" --work "$work" \
  --baseline "$root/scripts/filefind/model.json" --candidate "$candidate"
status=$?
set -e
digest="$(basename "$candidate" .json)"
echo "candidate: $candidate"
echo "receipt:   $work/compare-${digest#model-}.json"
if [ "$status" -eq 0 ]; then
  echo "the active model is unchanged; to activate a passing candidate:"
  echo "  python3 $root/scripts/filefind/ranker_gate.py promote --candidate $candidate --receipt $work/compare-${digest#model-}.json"
fi
exit "$status"
