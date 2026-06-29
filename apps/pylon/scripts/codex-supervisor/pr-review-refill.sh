#!/usr/bin/env bash
#
# pr-review-refill.sh — lock-aware PR-review refill helpers for Codex fanout.
#
# The issue supervisor correctly avoids open-PR issues; once the backlog becomes
# PR-heavy, a separate review/refill lane is needed. This helper keeps that lane
# from re-launching the same PR in parallel by using an atomic on-disk lock keyed
# by PR number. Accepted assignments keep their lock while the matching local
# assignment marker is active; finished assignments become short-lived done
# markers so the refiller moves through the PR backlog instead of rereviewing
# one hot PR.
#
# Source this file for helpers, or run:
#   bash apps/pylon/scripts/codex-supervisor/pr-review-refill.sh refill-once --account codex-3

: "${SUP_PR_REVIEW_STATE_DIR:=${SUP_STATE_DIR:-$HOME/.codex-supervisor}/pr-review}"
: "${SUP_PR_REVIEW_LOCK_TTL_SECS:=7200}"
: "${SUP_PR_REVIEW_DONE_TTL_SECS:=86400}"
: "${SUP_PR_REVIEW_GH_LIMIT:=300}"
: "${SUP_PR_REVIEW_LOG_DIR:=$HOME/.pylon-fable/pr-topup-logs}"
: "${SUP_PR_REVIEW_ACTIVE_ASSIGNMENT_DIR:=$HOME/.pylon-fable/active-assignment-runs}"
: "${SUP_PR_REVIEW_REPO:=OpenAgentsInc/openagents}"
: "${SUP_PR_REVIEW_REPO_ROOT:=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)}"
: "${SUP_PR_REVIEW_PYLON_HOME:=$HOME/.pylon-fable}"
: "${SUP_PR_REVIEW_BASE_URL:=https://openagents.com}"
: "${SUP_PR_REVIEW_WORKFLOW:=codex_agent_task}"
: "${SUP_PR_REVIEW_VERIFY:=bun scripts/check-conflict-markers.mjs}"
: "${SUP_PR_REVIEW_BRANCH:=main}"
: "${SUP_GH_BIN:=gh}"

sup_pr_review_file_mtime() {
  stat -f %m "$1" 2>/dev/null || stat -c %Y "$1" 2>/dev/null || echo 0
}

sup_pr_review_locks_dir() {
  local d="$SUP_PR_REVIEW_STATE_DIR/locks"
  mkdir -p "$d" 2>/dev/null || true
  printf '%s' "$d"
}

sup_pr_review_done_dir() {
  local d="$SUP_PR_REVIEW_STATE_DIR/done"
  mkdir -p "$d" 2>/dev/null || true
  printf '%s' "$d"
}

sup_pr_review_lock_path() {
  local pr="$1"
  printf '%s/pr.%s' "$(sup_pr_review_locks_dir)" "$pr"
}

sup_pr_review_done_path() {
  local pr="$1"
  printf '%s/pr.%s' "$(sup_pr_review_done_dir)" "$pr"
}

sup_pr_review_valid_pr() {
  case "${1:-}" in
    ''|*[!0-9]*) return 1 ;;
    *) return 0 ;;
  esac
}

sup_pr_review_lock_assignment_ref() {
  local pr="$1"
  local f
  f="$(sup_pr_review_lock_path "$pr")/assignment_ref"
  [ -f "$f" ] || return 1
  sed -n '1p' "$f"
}

sup_pr_review_active_assignment_exists() {
  local ref="$1"
  [ -n "$ref" ] || return 1
  local dir="$SUP_PR_REVIEW_ACTIVE_ASSIGNMENT_DIR"
  [ -d "$dir" ] || return 1
  python3 - "$dir" "$ref" <<'PY' >/dev/null 2>&1
import json
import os
import sys

root, wanted = sys.argv[1], sys.argv[2]
try:
    names = os.listdir(root)
except OSError:
    sys.exit(1)
for name in names:
    path = os.path.join(root, name)
    if name in (wanted, f"{wanted}.json"):
        sys.exit(0)
    if not os.path.isfile(path):
        continue
    try:
        with open(path, encoding="utf-8") as fh:
            data = json.load(fh)
    except Exception:
        continue
    if data.get("assignmentRef") == wanted:
        sys.exit(0)
sys.exit(1)
PY
}

sup_pr_review_done_is_fresh() {
  local pr="$1"
  local done
  done="$(sup_pr_review_done_path "$pr")"
  [ -e "$done" ] || return 1
  local now age
  now=$(date +%s)
  age=$(( now - $(sup_pr_review_file_mtime "$done") ))
  [ "$age" -ge 0 ] && [ "$age" -lt "$SUP_PR_REVIEW_DONE_TTL_SECS" ]
}

sup_pr_review_mark_done() {
  local pr="$1" ref="${2:-}"
  sup_pr_review_valid_pr "$pr" || return 1
  local done
  done="$(sup_pr_review_done_path "$pr")"
  mkdir -p "$done" 2>/dev/null || true
  {
    printf 'pr=%s\n' "$pr"
    [ -n "$ref" ] && printf 'assignment_ref=%s\n' "$ref"
    printf 'marked_at=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  } > "$done/meta" 2>/dev/null || true
  touch "$done" 2>/dev/null || true
}

sup_pr_review_release() {
  local pr="$1"
  sup_pr_review_valid_pr "$pr" || return 0
  rm -rf "$(sup_pr_review_lock_path "$pr")" 2>/dev/null || true
}

sup_pr_review_gc_locks() {
  local now lock pr ref age done
  now=$(date +%s)

  for done in "$(sup_pr_review_done_dir)"/pr.*; do
    [ -e "$done" ] || continue
    age=$(( now - $(sup_pr_review_file_mtime "$done") ))
    if [ "$age" -lt 0 ] || [ "$age" -ge "$SUP_PR_REVIEW_DONE_TTL_SECS" ]; then
      rm -rf "$done" 2>/dev/null || true
    fi
  done

  for lock in "$(sup_pr_review_locks_dir)"/pr.*; do
    [ -e "$lock" ] || continue
    pr="${lock##*.}"
    ref="$(sed -n '1p' "$lock/assignment_ref" 2>/dev/null || true)"
    if [ -n "$ref" ] && sup_pr_review_active_assignment_exists "$ref"; then
      continue
    fi
    age=$(( now - $(sup_pr_review_file_mtime "$lock") ))
    if [ "$age" -lt 0 ] || [ "$age" -ge "$SUP_PR_REVIEW_LOCK_TTL_SECS" ]; then
      if [ -n "$ref" ]; then
        sup_pr_review_mark_done "$pr" "$ref"
      fi
      rm -rf "$lock" 2>/dev/null || true
    fi
  done
}

sup_pr_review_is_reserved() {
  local pr="$1"
  sup_pr_review_valid_pr "$pr" || return 1
  if sup_pr_review_done_is_fresh "$pr"; then
    return 0
  fi

  local lock ref now age
  lock="$(sup_pr_review_lock_path "$pr")"
  [ -e "$lock" ] || return 1
  ref="$(sed -n '1p' "$lock/assignment_ref" 2>/dev/null || true)"
  if [ -n "$ref" ] && sup_pr_review_active_assignment_exists "$ref"; then
    return 0
  fi
  now=$(date +%s)
  age=$(( now - $(sup_pr_review_file_mtime "$lock") ))
  [ "$age" -ge 0 ] && [ "$age" -lt "$SUP_PR_REVIEW_LOCK_TTL_SECS" ]
}

sup_pr_review_try_claim() {
  local pr="$1" owner="${2:-$$}"
  sup_pr_review_valid_pr "$pr" || return 1
  sup_pr_review_gc_locks
  if sup_pr_review_is_reserved "$pr"; then
    return 1
  fi

  local lock
  lock="$(sup_pr_review_lock_path "$pr")"
  if mkdir "$lock" 2>/dev/null; then
    {
      printf 'pr=%s\n' "$pr"
      printf 'owner=%s\n' "$owner"
      printf 'claimed_at=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    } > "$lock/meta" 2>/dev/null || true
    return 0
  fi
  return 1
}

sup_pr_review_mark_accepted() {
  local pr="$1" ref="$2"
  sup_pr_review_valid_pr "$pr" || return 1
  [ -n "$ref" ] || return 1
  local lock
  lock="$(sup_pr_review_lock_path "$pr")"
  mkdir -p "$lock" 2>/dev/null || true
  printf '%s\n' "$ref" > "$lock/assignment_ref" 2>/dev/null || true
  {
    printf 'pr=%s\n' "$pr"
    printf 'assignment_ref=%s\n' "$ref"
    printf 'accepted_at=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  } > "$lock/meta" 2>/dev/null || true
  touch "$lock" 2>/dev/null || true
}

sup_pr_review_pick_and_claim() {
  local pr
  sup_pr_review_gc_locks
  for pr in "$@"; do
    sup_pr_review_valid_pr "$pr" || continue
    if sup_pr_review_is_reserved "$pr"; then
      continue
    fi
    if sup_pr_review_try_claim "$pr"; then
      printf '%s' "$pr"
      return 0
    fi
  done
  return 1
}

sup_pr_review_pr_from_log_path() {
  python3 - "$1" <<'PY' 2>/dev/null
import os
import re
import sys
m = re.search(r"-([0-9]+)-[0-9]+\.log$", os.path.basename(sys.argv[1]))
if m:
    print(m.group(1))
PY
}

sup_pr_review_assignment_ref_from_log() {
  python3 - "$1" <<'PY' 2>/dev/null
import re
import sys
try:
    text = open(sys.argv[1], encoding="utf-8", errors="replace").read()
except OSError:
    sys.exit(1)
m = re.search(r'"event":"assignment_run\.accepted".*?"assignmentRef":"([^"]+)"', text)
if m:
    print(m.group(1))
PY
}

sup_pr_review_sync_recent_logs() {
  local limit="${1:-200}"
  local logs=()
  [ -d "$SUP_PR_REVIEW_LOG_DIR" ] || return 0
  while IFS= read -r log_path; do
    [ -n "$log_path" ] && logs+=("$log_path")
  done < <(python3 - "$SUP_PR_REVIEW_LOG_DIR" "$limit" <<'PY' 2>/dev/null
import glob
import os
import sys

root = sys.argv[1]
limit = int(sys.argv[2])
paths = glob.glob(os.path.join(root, "pr-review-*.log"))
paths.sort(key=lambda p: os.path.getmtime(p), reverse=True)
for path in paths[:limit]:
    print(path)
PY
  )

  local log_path pr ref
  for log_path in "${logs[@]}"; do
    pr="$(sup_pr_review_pr_from_log_path "$log_path")"
    [ -n "$pr" ] || continue
    ref="$(sup_pr_review_assignment_ref_from_log "$log_path")"
    if [ -n "$ref" ]; then
      sup_pr_review_mark_accepted "$pr" "$ref"
    fi
  done
  sup_pr_review_gc_locks
}

sup_pr_review_open_pr_numbers() {
  local json
  json="$("$SUP_GH_BIN" pr list \
    --repo "$SUP_PR_REVIEW_REPO" \
    --state open \
    --limit "$SUP_PR_REVIEW_GH_LIMIT" \
    --json number,isDraft,mergeable,updatedAt \
    2>/dev/null)" || return 1
  printf '%s' "$json" | python3 -c '
import json
import sys
try:
    rows = json.load(sys.stdin)
except Exception:
    sys.exit(0)
def key(row):
    mergeable = row.get("mergeable")
    rank = 0 if mergeable == "MERGEABLE" else 1 if mergeable == "UNKNOWN" else 2
    return (rank, row.get("updatedAt") or "", row.get("number") or 0)
for row in sorted((r for r in rows if not r.get("isDraft")), key=key):
    number = row.get("number")
    if isinstance(number, int):
        print(number)
'
}

sup_pr_review_fresh_origin_main() {
  git ls-remote "https://github.com/${SUP_PR_REVIEW_REPO}.git" "refs/heads/$SUP_PR_REVIEW_BRANCH" 2>/dev/null \
    | awk '{print $1; exit}'
}

sup_pr_review_prompt() {
  local pr="$1"
  cat <<EOF
Resolve OpenAgents public PR #$pr as part of the PR queue burndown. Use gh pr view #$pr, inspect the patch, and classify it as merge-ready, duplicate/obsolete, or blocked. If it is obviously duplicate or superseded by a newer/better PR for the same issue, leave a concise public comment explaining the superseding PR and close #$pr. If it is merge-ready and the narrow verification passes, merge it using the safest normal gh merge path for this repo. If it has a small clear blocker, check out the PR branch, fix only that blocker, run the narrow verification, and push to the same branch. Do not open a duplicate PR. Do not touch unrelated files. Finish with the concrete action taken: merged, closed, fixed, or still-blocked with reason.
EOF
}

sup_pr_review_launch() {
  local account="$1" pr="$2" commit="$3" log_path="$4"
  [ -n "$account" ] || return 1
  sup_pr_review_valid_pr "$pr" || return 1
  [ -n "$commit" ] || return 1
  mkdir -p "$(dirname "$log_path")" 2>/dev/null || true
  local prompt
  prompt="$(sup_pr_review_prompt "$pr")"
  nohup bash -c 'cd "$1" || exit 1; shift; exec "$@"' sh "$SUP_PR_REVIEW_REPO_ROOT" \
      env \
        PYLON_HOME="$SUP_PR_REVIEW_PYLON_HOME" \
        PYLON_OPENAGENTS_BASE_URL="$SUP_PR_REVIEW_BASE_URL" \
      bun apps/pylon/src/index.ts khala request \
        --account "$account" \
        --prompt "$prompt" \
        --workflow "$SUP_PR_REVIEW_WORKFLOW" \
        --repo "$SUP_PR_REVIEW_REPO" \
        --branch "$SUP_PR_REVIEW_BRANCH" \
        --commit "$commit" \
        --verify "$SUP_PR_REVIEW_VERIFY" \
        --json \
    >> "$log_path" 2>&1 &
  printf '%s' "$!"
}

sup_pr_review_refill_once() {
  local account="$1"
  [ -n "$account" ] || return 64
  sup_pr_review_sync_recent_logs 300

  local candidates=()
  while IFS= read -r pr; do
    [ -n "$pr" ] && candidates+=("$pr")
  done < <(sup_pr_review_open_pr_numbers)
  [ "${#candidates[@]}" -gt 0 ] || return 2

  local pr
  pr="$(sup_pr_review_pick_and_claim "${candidates[@]}")" || return 3
  [ -n "$pr" ] || return 3

  local commit
  commit="$(sup_pr_review_fresh_origin_main)"
  if [ -z "$commit" ]; then
    sup_pr_review_release "$pr"
    return 4
  fi

  local stamp log_path pid
  stamp="$(date -u +%Y%m%dT%H%M%SZ)"
  mkdir -p "$SUP_PR_REVIEW_LOG_DIR" 2>/dev/null || true
  log_path="$SUP_PR_REVIEW_LOG_DIR/pr-review-${stamp}-${account}-${pr}-$$.log"
  pid="$(sup_pr_review_launch "$account" "$pr" "$commit" "$log_path")"
  if [ -z "$pid" ]; then
    sup_pr_review_release "$pr"
    return 5
  fi
  printf 'spawned account=%s pr=#%s pid=%s log=%s\n' "$account" "$pr" "$pid" "$log_path"
}

sup_pr_review_usage() {
  cat <<'EOF'
usage:
  pr-review-refill.sh gc
  pr-review-refill.sh pick <pr>...
  pr-review-refill.sh mark-accepted <pr> <assignment-ref>
  pr-review-refill.sh release <pr>
  pr-review-refill.sh sync-logs [limit]
  pr-review-refill.sh refill-once --account <codex-ref>
EOF
}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then
  set -uo pipefail
  cmd="${1:-help}"
  shift || true
  case "$cmd" in
    gc)
      sup_pr_review_gc_locks
      ;;
    pick)
      sup_pr_review_pick_and_claim "$@"
      ;;
    mark-accepted)
      sup_pr_review_mark_accepted "${1:-}" "${2:-}"
      ;;
    release)
      sup_pr_review_release "${1:-}"
      ;;
    sync-logs)
      sup_pr_review_sync_recent_logs "${1:-200}"
      ;;
    refill-once)
      account=""
      while [ "$#" -gt 0 ]; do
        case "$1" in
          --account)
            shift
            account="${1:-}"
            ;;
          *)
            sup_pr_review_usage >&2
            exit 64
            ;;
        esac
        shift || true
      done
      sup_pr_review_refill_once "$account"
      ;;
    help|-h|--help)
      sup_pr_review_usage
      ;;
    *)
      sup_pr_review_usage >&2
      exit 64
      ;;
  esac
fi
