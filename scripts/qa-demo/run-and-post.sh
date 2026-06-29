#!/usr/bin/env bash
# QA flow: run a Khala-driven QA session, compose a polished video, and post it to a PR.
#
# This is the "actual flow" Rhys asked for, end to end:
#   Khala drives real Chrome  ->  records a video  ->  distills a committed e2e test
#   ->  compose a polished video (apps/qa-runner compose, ffmpeg)  ->  attach it to the PR
#   via gh-attach (the GitHub web-upload path the REST API does not expose).
#
# Usage:  scripts/qa-demo/run-and-post.sh <PR_NUMBER> [EXISTING_RUN_DIR]
#   OPENAGENTS_API_KEY  a Khala agent bearer token (own-infra/exempt lane -> $0)
#   GH_ATTACH           path to the gh-attach binary (default: gh-attach on PATH)
set -euo pipefail
PR="${1:?usage: run-and-post.sh <PR_NUMBER> [EXISTING_RUN_DIR]}"
RUN_DIR="${2:-}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

if [ -z "$RUN_DIR" ]; then
  : "${OPENAGENTS_API_KEY:?set OPENAGENTS_API_KEY to a Khala agent token}"
  RUN_DIR="apps/qa-runner/runs/pr-${PR}"
  echo "[1/3] Khala-driven QA run -> $RUN_DIR"
  bun run --cwd apps/qa-runner demo:khala -- --out "$ROOT/$RUN_DIR"
else
  echo "[1/3] reusing run dir $RUN_DIR"
fi

echo "[2/3] compose polished video"
OUT="$RUN_DIR/compose.mp4"
bun run --cwd apps/qa-runner compose -- --run "$ROOT/$RUN_DIR" --out "$ROOT/$OUT"

echo "[3/3] attach to PR #$PR via gh-attach"
GH_ATTACH="${GH_ATTACH:-gh-attach}"
MD="$("$GH_ATTACH" "$ROOT/$OUT" --repo OpenAgentsInc/openagents --md)"
gh pr comment "$PR" --body "$(printf '### 🤖 Khala autonomous-QA run\n\n%s\n\nDriven by Khala on own-infra at \$0 (operator-credit exemption). Verdict + steps in `result.json`; a committed e2e test was distilled from this session.\n' "$MD")"
echo "done -> posted composed video to PR #$PR"
