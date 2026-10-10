#!/usr/bin/env bash
# Retrain the filefind ranker on every fix up to now (issue #11210).
#
#   OPENROUTER_API_KEY=... scripts/filefind/retrain.sh [WORK_DIR]
#
# 1. refreshes the indexes (incremental) and the run feedback,
# 2. rebuilds the issue -> fix dataset from main's history (closed issues),
# 3. embeds/tokenizes every replayed parent tree (cached per blob),
# 4. computes candidate features (cached per issue in WORK_DIR),
# 5. trains the two-stage ranker on all of them and writes scripts/filefind/model.json.
# Then `file-finding-bench.py eval` on the same WORK_DIR reports the held-out numbers.
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
python3 "$bench" train --all --reuse-stage2 --repo "$root" --dataset "$work/dataset.json" --work "$work" \
  --model "$root/scripts/filefind/model.json"
echo "model: $root/scripts/filefind/model.json (held-out model: $work/model.json)"
