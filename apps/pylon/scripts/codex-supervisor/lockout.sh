#!/usr/bin/env bash
#
# lockout.sh — dispatch lockout helpers for codex-supervisor (issue #6439 reopen)
#
# Before dispatching an issue to a slot, the supervisor checks GitHub for an
# existing OPEN PR that already references that issue. If one exists, the issue
# is "locked" and skipped, so the fleet never re-solves an issue that already
# has a PR. This is the supervisor-side half of the duplicate-PR fix (123 PRs
# across 49 issues): keep already-PR'd issues from ever being dispatched again.
#
# Design:
#   * Sourceable + test-friendly: the `gh` binary is indirected via $SUP_GH_BIN
#     (default "gh") so tests can stub it without a network.
#   * Short TTL cache ($SUP_LOCKOUT_TTL_SECS, default 90s) under the state dir
#     avoids hammering the API on every dispatch.
#   * FAIL-OPEN: if gh is missing or errors, issues are treated as UNLOCKED so a
#     transient GitHub problem never stalls the whole fleet.
#
# This file performs no work at source time; it only defines functions.

: "${SUP_GH_BIN:=gh}"
: "${SUP_REPO:=OpenAgentsInc/openagents}"
: "${SUP_LOCKOUT_TTL_SECS:=90}"
: "${SUP_STATE_DIR:=$HOME/.codex-supervisor}"

# Portable file mtime (BSD/macOS `stat -f`, GNU/Linux `stat -c`).
sup_file_mtime() {
  stat -f %m "$1" 2>/dev/null || stat -c %Y "$1" 2>/dev/null || echo 0
}

# issue_has_open_pr <issue>
#   rc 0  -> an OPEN PR references the issue (LOCKED; do not dispatch)
#   rc 1  -> unlocked / unknown / gh missing (safe to dispatch)
issue_has_open_pr() {
  local issue="$1"
  [ -n "$issue" ] || return 1
  command -v "$SUP_GH_BIN" >/dev/null 2>&1 || return 1

  local cache_dir="${SUP_LOCKOUT_CACHE_DIR:-$SUP_STATE_DIR}"
  mkdir -p "$cache_dir" 2>/dev/null || true
  local cache="$cache_dir/lockout.$issue"
  if [ -f "$cache" ]; then
    local now age
    now=$(date +%s)
    age=$(( now - $(sup_file_mtime "$cache") ))
    if [ "$age" -ge 0 ] && [ "$age" -lt "$SUP_LOCKOUT_TTL_SECS" ]; then
      [ "$(cat "$cache" 2>/dev/null)" = "locked" ] && return 0 || return 1
    fi
  fi

  local json
  json=$("$SUP_GH_BIN" pr list --repo "$SUP_REPO" --state open \
    --search "#$issue in:title in:body" \
    --json number,title,body --limit 50 2>/dev/null)

  local verdict
  verdict=$(printf '%s' "$json" | SUP_ISSUE="$issue" python3 -c "import sys,os,json,re
issue=os.environ.get('SUP_ISSUE','')
try:
    rows=json.load(sys.stdin)
except Exception:
    print('open'); sys.exit(0)
if not isinstance(rows,list):
    print('open'); sys.exit(0)
pat=re.compile(r'(?<!\d)#'+re.escape(issue)+r'(?!\d)')
for r in rows:
    if pat.search(r.get('title') or '') or pat.search(r.get('body') or ''):
        print('locked'); sys.exit(0)
print('open')" 2>/dev/null)
  [ -n "$verdict" ] || verdict="open"
  printf '%s' "$verdict" > "$cache" 2>/dev/null || true
  [ "$verdict" = "locked" ] && return 0 || return 1
}

# pick_unlocked_issue <start_index> <issue...>
#   Prints the first UNLOCKED issue, scanning the rotation from <start_index>.
#   rc 0 with the issue on stdout, or rc 1 when every issue is locked.
pick_unlocked_issue() {
  local start="$1"; shift
  local arr=("$@")
  local n="${#arr[@]}"
  [ "$n" -gt 0 ] || return 1
  local k i issue
  for (( k=0; k<n; k++ )); do
    i=$(( (start + k) % n ))
    issue="${arr[$i]}"
    if issue_has_open_pr "$issue"; then
      continue
    fi
    printf '%s' "$issue"
    return 0
  done
  return 1
}
