#!/usr/bin/env bash
# Standing re-study cadence for the OpenAgents repo study packet (SA-4, EPIC #5337).
#
# Runs the SA-4 freshness verdict over the live tree and, when the studied
# content has actually drifted (`stale`), refreshes the small committed digest
# index in place. This is the cheap-incremental re-study lane:
#
#   - It does NOT regenerate or commit the multi-MB packet/graph blobs (those are
#     regenerate-on-demand; only the small index is committed — the #5334 lesson).
#   - It does NOT churn the index on every commit. Pure commit drift (HEAD moved,
#     studied content identical) reports `fresh`, so nothing is written or
#     committed. The index is rewritten only on real content drift.
#   - A red verification correctness gate (`gate_failed`) fails the cadence
#     without writing, so a broken tree never silently refreshes the substrate.
#
# Intended drivers: CI-on-merge (push to main) or a scheduled job (see
# .github/workflows/restudy-openagents.yml). Safe to run locally too.
#
# Modes (first arg):
#   verdict   Print the freshness verdict JSON and exit with its code
#             (0 fresh, 2 stale, 1 gate_failed). Read-only. Default.
#   refresh   Refresh the committed index in place when stale (no git commit).
#             Exit 0 on fresh-or-refreshed, 1 on gate_failed.
#   ci        Refresh, and if the committed index changed, commit + push it on the
#             current branch with neutral metadata. Exit 0 on success, 1 on
#             gate_failed. No-op commit when already fresh.
#
# Env:
#   STUDY_INDEX_PATH   Override the committed index path (defaults to the SA-1 path).
#   GIT_PUSH=0         In `ci` mode, commit but do not push (for dry runs).

set -euo pipefail

MODE="${1:-verdict}"

RUNTIME_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="$(git -C "$RUNTIME_DIR" rev-parse --show-toplevel)"
CLI="$RUNTIME_DIR/scripts/generate-openagents-study-packet.ts"
INDEX_PATH="${STUDY_INDEX_PATH:-$REPO_ROOT/docs/research/machine-studying/openagents-studybench/study-packets/openagents.study-artifact-index.json}"

cd "$RUNTIME_DIR"

case "$MODE" in
  verdict)
    exec bun "$CLI" --freshness --root "$REPO_ROOT"
    ;;
  refresh)
    exec bun "$CLI" --refresh-if-stale --root "$REPO_ROOT"
    ;;
  ci)
    bun "$CLI" --refresh-if-stale --root "$REPO_ROOT"

    if git -C "$REPO_ROOT" diff --quiet -- "$INDEX_PATH"; then
      echo "study-packet index already fresh; no re-study commit needed."
      exit 0
    fi

    echo "study-packet index drifted; committing refreshed index."
    git -C "$REPO_ROOT" add "$INDEX_PATH"
    git -C "$REPO_ROOT" commit -m "chore(studybench): SA-4 re-study refresh of openagents study-packet index

Standing-freshness cadence detected content drift and refreshed the committed
digest index over the live tree. Only the small index is rewritten; the
packet/graph blobs stay regenerate-on-demand (EPIC #5337, SA-4)."

    if [ "${GIT_PUSH:-1}" = "1" ]; then
      git -C "$REPO_ROOT" push
    fi
    exit 0
    ;;
  *)
    echo "unknown mode: $MODE (expected verdict | refresh | ci)" >&2
    exit 64
    ;;
esac
