#!/usr/bin/env bash
#
# replenishment.sh — bounded real-work refill for codex-supervisor LOCKOUT (#6822)
#
# When every backlog issue is locked (closed, already has an open PR, or claimed
# by another slot), the supervisor must not sit in a long idle backoff. This
# helper creates or reuses a small fixed set of public, valuable replenishment
# issues: desktop-fleet readiness, broad codebase audit/review, and
# test/lint/typecheck sweep work. Dedupe is by exact open issue title under a
# dedicated label, so repeated LOCKOUT cycles do not spam duplicate issues.
#
# This file performs no work at source time; it only defines functions.

: "${SUP_GH_BIN:=gh}"
: "${SUP_REPO:=OpenAgentsInc/openagents}"
: "${SUP_STATE_DIR:=$HOME/.codex-supervisor}"
: "${SUP_GH_TIMEOUT_SECS:=15}"
: "${SUP_REPLENISHMENT_LABEL:=supervisor-replenishment}"
: "${SUP_REPLENISHMENT_LOCKOUTS:=3}"
: "${SUP_REPLENISHMENT_MAX_CREATE:=3}"
: "${SUP_REPLENISHMENT_LOCK_TTL_SECS:=120}"

if ! command -v sup_file_mtime >/dev/null 2>&1; then
  sup_file_mtime() {
    stat -f %m "$1" 2>/dev/null || stat -c %Y "$1" 2>/dev/null || echo 0
  }
fi

if ! command -v sup_run_timeout >/dev/null 2>&1; then
  sup_run_timeout() {
    local secs="$1"; shift
    [ "$#" -gt 0 ] || return 0
    if command -v timeout >/dev/null 2>&1; then
      timeout "$secs" "$@"; return $?
    fi
    "$@" &
    local cmd_pid=$!
    ( sleep "$secs"; kill -TERM "$cmd_pid" 2>/dev/null; sleep 2; kill -KILL "$cmd_pid" 2>/dev/null ) >/dev/null 2>&1 &
    local watch_pid=$!
    wait "$cmd_pid" 2>/dev/null
    local rc=$?
    kill -TERM "$watch_pid" 2>/dev/null; wait "$watch_pid" 2>/dev/null
    [ "$rc" -ge 128 ] && rc=124
    return "$rc"
  }
fi

sup_replenishment_label_args() {
  printf '%s\n' \
    "$SUP_REPLENISHMENT_LABEL" \
    "standing-task" \
    "prio:4-backstop-burn"
}

sup_replenishment_templates() {
  cat <<'TEMPLATES'
SG-2 replenish: desktop fleet readiness audit|desktop-fleet|Audit the current desktop-fleet dispatch path end to end: Khala Code fleet tools, Pylon heartbeat/capacity, closeout proof, and exact token accounting. Fix one concrete desktop-fleet blocker if found; otherwise add a focused regression test or public-safe audit note. Run the named verification before closeout.
SG-2 replenish: audit Pylon clients and Worker API for dispatch waste|audit|Perform a big-context codebase audit over apps/pylon, clients, and apps/openagents.com/workers/api. Look for real dispatch waste, duplicate-work, lockout, stale-base, timeout, and verification gaps. Implement a bounded fix when clear; otherwise add a focused regression test or public-safe audit note. Run the named verification before closeout.
SG-2 replenish: test lint typecheck sweep for owner-capacity lane|sweep|Run a focused test/lint/typecheck sweep on the owner-capacity Pylon and Worker API lane. Fix real failures in touched code without broad refactors. Do not use apps/pylon/scripts/multi-session-campaign.ts. Run the named verification before closeout.
TEMPLATES
}

sup_replenishment_template_titles() {
  sup_replenishment_templates | awk -F'|' 'NF { print $1 }'
}

sup_replenishment_open_issues_json() {
  command -v "$SUP_GH_BIN" >/dev/null 2>&1 || return 1
  sup_run_timeout "$SUP_GH_TIMEOUT_SECS" "$SUP_GH_BIN" issue list \
    --repo "$SUP_REPO" \
    --state open \
    --label "$SUP_REPLENISHMENT_LABEL" \
    --limit 50 \
    --json number,title 2>/dev/null
}

sup_replenishment_issue_for_title() {
  local title="$1"
  [ -n "$title" ] || return 1
  local json
  json="$(sup_replenishment_open_issues_json)" || return 1
  printf '%s' "$json" | SUP_REPLENISHMENT_TITLE="$title" python3 -c "import json,os,sys
title=os.environ.get('SUP_REPLENISHMENT_TITLE','')
try:
    rows=json.load(sys.stdin)
except Exception:
    sys.exit(1)
for row in rows if isinstance(rows,list) else []:
    if row.get('title') == title and isinstance(row.get('number'), int):
        print(row['number'])
        sys.exit(0)
sys.exit(1)" 2>/dev/null
}

sup_replenishment_template_issue_numbers_from_json() {
  local titles
  titles="$(sup_replenishment_template_titles)"
  SUP_REPLENISHMENT_TEMPLATE_TITLES="$titles" python3 -c "import json,os,sys
titles=set(filter(None, os.environ.get('SUP_REPLENISHMENT_TEMPLATE_TITLES','').splitlines()))
try:
    rows=json.load(sys.stdin)
except Exception:
    sys.exit(0)
seen=set()
for row in rows if isinstance(rows,list) else []:
    n=row.get('number')
    title=row.get('title')
    if isinstance(n,int) and title in titles and n not in seen:
        seen.add(n)
        print(n)" 2>/dev/null
}

sup_replenishment_created_issue_number() {
  python3 -c "import re,sys
text=sys.stdin.read()
matches=re.findall(r'/issues/([1-9][0-9]{0,8})(?:\\b|$)|#([1-9][0-9]{0,8})(?:\\b|$)', text)
for a,b in matches:
    print(a or b)
    sys.exit(0)
sys.exit(1)" 2>/dev/null
}

sup_replenishment_create_issue() {
  local title="$1" kind="$2" body="$3"
  [ -n "$title" ] && [ -n "$body" ] || return 1
  command -v "$SUP_GH_BIN" >/dev/null 2>&1 || return 1

  local labels=() label
  while IFS= read -r label; do
    [ -n "$label" ] && labels+=(--label "$label")
  done < <(sup_replenishment_label_args)

  local full_body
  full_body="$(cat <<EOF
Auto-replenishment task for supervisor LOCKOUT (#6822).

Kind: $kind

$body

Boundaries:
- Public repository work only.
- Keep the patch small and verification-backed.
- Do not create duplicate PRs; if an equivalent open PR already exists, close out with that evidence.
- Do not use apps/pylon/scripts/multi-session-campaign.ts.
EOF
)"

  local out
  out="$(sup_run_timeout "$SUP_GH_TIMEOUT_SECS" "$SUP_GH_BIN" issue create \
    --repo "$SUP_REPO" \
    --title "$title" \
    --body "$full_body" \
    "${labels[@]}" 2>/dev/null)" || return 1
  printf '%s' "$out" | sup_replenishment_created_issue_number
}

sup_replenishment_lock_dir() {
  local d="${SUP_LOCKOUT_CACHE_DIR:-$SUP_STATE_DIR}/replenishment.lock"
  printf '%s' "$d"
}

sup_replenishment_with_lock() {
  local lock; lock="$(sup_replenishment_lock_dir)"
  local parent; parent="$(dirname "$lock")"
  mkdir -p "$parent" 2>/dev/null || true
  if mkdir "$lock" 2>/dev/null; then
    printf '%s\n' "$(date +%s)" > "$lock/meta" 2>/dev/null || true
    return 0
  fi
  if [ -e "$lock" ]; then
    local now age
    now=$(date +%s)
    age=$(( now - $(sup_file_mtime "$lock") ))
    if [ "$age" -lt 0 ] || [ "$age" -ge "$SUP_REPLENISHMENT_LOCK_TTL_SECS" ]; then
      rm -rf "$lock" 2>/dev/null || true
      mkdir "$lock" 2>/dev/null && return 0
    fi
  fi
  return 1
}

sup_replenishment_unlock() {
  rm -rf "$(sup_replenishment_lock_dir)" 2>/dev/null || true
}

# sup_ensure_replenishment_issues
#   Prints open issue numbers for replenishment work. Creates at most
#   SUP_REPLENISHMENT_MAX_CREATE missing template issues while holding a local
#   lock. Existing open issues with exact matching titles are always reused.
sup_ensure_replenishment_issues() {
  command -v "$SUP_GH_BIN" >/dev/null 2>&1 || return 1

  if ! sup_replenishment_with_lock; then
    sup_replenishment_open_issues_json | sup_replenishment_template_issue_numbers_from_json
    return 0
  fi

  local made=0
  local title kind body existing created
  while IFS='|' read -r title kind body; do
    [ -n "$title" ] || continue
    existing="$(sup_replenishment_issue_for_title "$title" 2>/dev/null || true)"
    if [ -n "$existing" ]; then
      printf '%s\n' "$existing"
      continue
    fi
    if [ "$made" -ge "$SUP_REPLENISHMENT_MAX_CREATE" ] 2>/dev/null; then
      continue
    fi
    created="$(sup_replenishment_create_issue "$title" "$kind" "$body" 2>/dev/null || true)"
    if [ -n "$created" ]; then
      made=$(( made + 1 ))
      printf '%s\n' "$created"
    fi
  done < <(sup_replenishment_templates)
  sup_replenishment_unlock
}
