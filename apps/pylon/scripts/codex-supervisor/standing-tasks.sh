#!/usr/bin/env bash
#
# standing-tasks.sh — auto-recreate closed STANDING task(ops) issues (#6711)
#
# Fleet-saturation engine, part 2: the four standing task(ops) issues are the
# self-sustaining floor of the priority queue (one per prio tier 1-4). If any is
# closed (a slot finished its cycle of that standing work), the queue must
# re-grow it so the fleet never runs dry. A standing task is identified by the
# `standing-task` label; when a labelled issue is CLOSED and no OPEN issue with
# the SAME title exists, the supervisor opens a fresh clone (same title, body,
# and labels). The old closed issue stays closed as the audit trail.
#
# Idempotency (no duplicates): recreation is keyed on the issue TITLE. Once a
# fresh clone is open, the next sweep sees an OPEN sibling for that title and
# skips it. So running this repeatedly never produces duplicate standing tasks,
# and a title that already has an open standing issue is never recreated.
#
# Design:
#   * Sourceable + test-friendly: `gh` is indirected via $SUP_GH_BIN. The
#     selection logic (`sup_standing_tasks_to_recreate`) is a pure parse over the
#     gh JSON, so the no-dup behaviour is verifiable with a stubbed gh.
#   * Fails soft: when gh is missing/errors, nothing is recreated (rc 1) — a
#     transient GitHub problem never spams duplicate issues.
#
# This file performs no work at source time; it only defines functions.

: "${SUP_GH_BIN:=gh}"
: "${SUP_REPO:=OpenAgentsInc/openagents}"
: "${SUP_STANDING_TASK_LABEL:=standing-task}"
: "${SUP_STANDING_TASK_LIMIT:=200}"
: "${SUP_GH_TIMEOUT_SECS:=15}"

# Defensive fallback for standalone use; codex-supervisor.sh sources lockout.sh
# (which defines sup_run_timeout) before this file.
if ! command -v sup_run_timeout >/dev/null 2>&1; then
  sup_run_timeout() {
    local secs="$1"; shift
    [ "$#" -gt 0 ] || return 0
    local rc
    if command -v timeout >/dev/null 2>&1; then
      timeout "$secs" "$@"; rc=$?
    elif command -v gtimeout >/dev/null 2>&1; then
      gtimeout "$secs" "$@"; rc=$?
    else
      "$@" &
      local cmd_pid=$!
      ( sleep "$secs"; kill -TERM "$cmd_pid" 2>/dev/null; sleep 2; kill -KILL "$cmd_pid" 2>/dev/null ) >/dev/null 2>&1 &
      local watch_pid=$!
      wait "$cmd_pid" 2>/dev/null; rc=$?
      kill -TERM "$watch_pid" 2>/dev/null; wait "$watch_pid" 2>/dev/null
      [ "$rc" -ge 128 ] && rc=124
    fi
    return "$rc"
  }
fi

# Optional logger — used by the live supervisor; harmless no-op standalone.
if ! command -v sup_standing_log >/dev/null 2>&1; then
  sup_standing_log() {
    if [ -n "${SUP_LOG:-}" ]; then
      printf '%s standing-tasks %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*" >> "$SUP_LOG" 2>/dev/null || true
    fi
  }
fi

# sup_standing_task_rows_json
#   Prints the raw gh JSON array of ALL (open + closed) `standing-task` issues
#   with number, title, and state. rc 1 when gh is missing/errors.
sup_standing_task_rows_json() {
  command -v "$SUP_GH_BIN" >/dev/null 2>&1 || return 1
  local json
  json=$(sup_run_timeout "$SUP_GH_TIMEOUT_SECS" "$SUP_GH_BIN" issue list --repo "$SUP_REPO" \
    --label "$SUP_STANDING_TASK_LABEL" --state all \
    --limit "$SUP_STANDING_TASK_LIMIT" --json number,title,state 2>/dev/null) || return 1
  [ -n "$json" ] || return 1
  printf '%s' "$json"
}

# sup_standing_tasks_to_recreate
#   Prints the source issue NUMBER (most-recent closed clone) for each standing
#   task TITLE that is currently CLOSED with NO open sibling — i.e. the titles
#   that need a fresh clone. The no-dup guard lives here: any title with an OPEN
#   standing issue is omitted. rc 1 when gh is missing/errors (prints nothing).
sup_standing_tasks_to_recreate() {
  local json
  json="$(sup_standing_task_rows_json)" || return 1
  printf '%s' "$json" | python3 -c "import sys,json
try:
    rows=json.load(sys.stdin)
except Exception:
    sys.exit(1)
if not isinstance(rows,list):
    sys.exit(0)
open_titles=set()
closed={}  # title -> most-recent (highest) closed issue number
for r in rows:
    if not isinstance(r,dict):
        continue
    t=(r.get('title') or '').strip()
    st=(r.get('state') or '').upper()
    n=r.get('number')
    if not t or not isinstance(n,int):
        continue
    if st=='OPEN':
        open_titles.add(t)
    elif st=='CLOSED':
        if t not in closed or n>closed[t]:
            closed[t]=n
# Recreate only titles that are closed AND have no open sibling (idempotent).
for t,n in sorted(closed.items(), key=lambda kv: kv[1]):
    if t not in open_titles:
        print(n)" 2>/dev/null
}

# sup_recreate_closed_standing_tasks
#   For each closed-with-no-open-sibling standing task, opens a fresh clone with
#   the same title, body, and labels. Prints the number of issues created.
#   Idempotent across runs (keyed on title). rc 1 when gh is missing.
sup_recreate_closed_standing_tasks() {
  command -v "$SUP_GH_BIN" >/dev/null 2>&1 || return 1
  local created=0 src
  while IFS= read -r src; do
    [ -n "$src" ] || continue
    local meta
    meta=$(sup_run_timeout "$SUP_GH_TIMEOUT_SECS" "$SUP_GH_BIN" issue view "$src" --repo "$SUP_REPO" \
      --json title,body,labels 2>/dev/null) || continue
    [ -n "$meta" ] || continue
    local title body labels
    title=$(printf '%s' "$meta" | python3 -c "import sys,json
try: print((json.load(sys.stdin).get('title') or ''))
except Exception: pass" 2>/dev/null)
    body=$(printf '%s' "$meta" | python3 -c "import sys,json
try: print((json.load(sys.stdin).get('body') or ''))
except Exception: pass" 2>/dev/null)
    labels=$(printf '%s' "$meta" | python3 -c "import sys,json
try:
    d=json.load(sys.stdin)
    print(','.join([(l.get('name') or '') for l in (d.get('labels') or []) if isinstance(l,dict) and l.get('name')]))
except Exception: pass" 2>/dev/null)
    [ -n "$title" ] || continue
    local label_args=()
    if [ -n "$labels" ]; then
      local oldifs="$IFS"; IFS=','
      local l
      for l in $labels; do [ -n "$l" ] && label_args+=(--label "$l"); done
      IFS="$oldifs"
    fi
    if sup_run_timeout "$SUP_GH_TIMEOUT_SECS" "$SUP_GH_BIN" issue create --repo "$SUP_REPO" \
        --title "$title" --body "$body" "${label_args[@]}" >/dev/null 2>&1; then
      created=$(( created + 1 ))
      sup_standing_log "recreated standing task from closed #$src: $title"
    fi
  done < <(sup_standing_tasks_to_recreate)
  printf '%s' "$created"
  return 0
}
