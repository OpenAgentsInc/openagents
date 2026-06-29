#!/usr/bin/env bash
#
# lockout.sh — dispatch lockout helpers for codex-supervisor (issue #6439 reopen)
#
# Before dispatching an issue to a slot, the supervisor must confirm BOTH:
#   1. the issue is STILL OPEN (it was not closed/merged during this run), and
#   2. no existing OPEN PR already references it.
# If either check fails the issue is "locked" and skipped, so the fleet never
# re-solves work that is already resolved or already has a PR. Closed-issue
# redispatch (a PR opened against an already-closed issue, e.g. PR #6602 vs the
# closed #6423) and duplicate PRs (123 PRs across 49 issues) both come from the
# supervisor trusting a STALE startup issue snapshot and dispatching issues that
# should be locked.
#
# Design:
#   * Sourceable + test-friendly: the `gh` binary is indirected via $SUP_GH_BIN
#     (default "gh") so tests can stub it without a network.
#   * Short TTL cache ($SUP_LOCKOUT_TTL_SECS, default 90s) under the state dir
#     avoids hammering the API on every dispatch.
#   * Fail policy is intentionally split by which mistake is worse:
#       - issue_has_open_pr FAILS OPEN: if gh is missing/errors, treat the issue
#         as UNLOCKED so a transient GitHub problem never stalls the whole fleet.
#       - issue_is_open FAILS CLOSED: if gh is missing/errors (state cannot be
#         confirmed OPEN), treat the issue as NOT dispatchable. Re-doing already
#         resolved work is the exact waste this guard exists to stop, so when in
#         doubt we skip rather than risk dispatching a closed issue.
#
# This file performs no work at source time; it only defines functions.

: "${SUP_GH_BIN:=gh}"
: "${SUP_REPO:=OpenAgentsInc/openagents}"
: "${SUP_LOCKOUT_TTL_SECS:=90}"
: "${SUP_STATE_DIR:=$HOME/.codex-supervisor}"
# Strict timeout (s) for every external `gh` CLI call (issue #6646 wedge fix).
: "${SUP_GH_TIMEOUT_SECS:=15}"
: "${SUP_NON_DISPATCH_ISSUES:=6376 6637 6654 6707 6708 6709}"
: "${SUP_NON_DISPATCH_LABELS:=epic standing-task}"
: "${SUP_NON_DISPATCH_TITLE_RE:=^(EPIC:|task\\(ops\\):)}"

# Portable file mtime (BSD/macOS `stat -f`, GNU/Linux `stat -c`).
sup_file_mtime() {
  stat -f %m "$1" 2>/dev/null || stat -c %Y "$1" 2>/dev/null || echo 0
}

# sup_run_timeout <secs> <cmd...>
#   #6646 wedge guard. The supervisor's #1 token-burn failure mode is a single
#   external `gh`/network call in the dispatch loop hanging with NO timeout: the
#   async dispatch loop silently stalls (alive + heartbeating, but never
#   dispatching) while the independent heartbeat keeps firing. This wraps any
#   external call with a hard wall-clock bound so it can NEVER hang unbounded.
#   On timeout it returns 124 (mirroring coreutils `timeout`), emits a TIMEOUT
#   line on stderr, and the caller treats it as a failed/empty result and
#   CONTINUES the loop. Portable: prefers `timeout`/`gtimeout`, else a pure-bash
#   background-watchdog fallback (stock macOS ships neither binary).
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
    # Normalize a watchdog-killed command to coreutils' 124 timeout code.
    [ "$rc" -ge 128 ] && rc=124
  fi
  if [ "$rc" -eq 124 ]; then
    printf 'sup_run_timeout: TIMEOUT after %ss: %s\n' "$secs" "$*" >&2
  fi
  return "$rc"
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
  json=$(sup_run_timeout "$SUP_GH_TIMEOUT_SECS" "$SUP_GH_BIN" pr list --repo "$SUP_REPO" --state open \
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

# issue_is_open <issue>
#   rc 0  -> the issue is confirmed OPEN (safe to dispatch w.r.t. state)
#   rc 1  -> the issue is CLOSED, OR its state cannot be confirmed (gh missing /
#            error / unexpected output). FAILS CLOSED on purpose: never dispatch
#            an issue we cannot prove is still open, because re-solving already
#            resolved work (a PR against a CLOSED issue) is the exact waste this
#            guard exists to stop.
issue_is_open() {
  local issue="$1"
  [ -n "$issue" ] || return 1
  command -v "$SUP_GH_BIN" >/dev/null 2>&1 || return 1

  local cache_dir="${SUP_LOCKOUT_CACHE_DIR:-$SUP_STATE_DIR}"
  mkdir -p "$cache_dir" 2>/dev/null || true
  local cache="$cache_dir/issuestate.$issue"
  if [ -f "$cache" ]; then
    local now age
    now=$(date +%s)
    age=$(( now - $(sup_file_mtime "$cache") ))
    if [ "$age" -ge 0 ] && [ "$age" -lt "$SUP_LOCKOUT_TTL_SECS" ]; then
      [ "$(cat "$cache" 2>/dev/null)" = "open" ] && return 0 || return 1
    fi
  fi

  local state
  state=$(sup_run_timeout "$SUP_GH_TIMEOUT_SECS" "$SUP_GH_BIN" issue view "$issue" --repo "$SUP_REPO" \
    --json state -q .state 2>/dev/null | tr '[:lower:]' '[:upper:]' | tr -d '[:space:]')

  case "$state" in
    OPEN)
      printf 'open' > "$cache" 2>/dev/null || true
      return 0
      ;;
    CLOSED)
      # A closed issue stays closed; cache the verdict so the whole rotation
      # locks it for the TTL instead of re-querying it every slot.
      printf 'closed' > "$cache" 2>/dev/null || true
      return 1
      ;;
    *)
      # Empty / unexpected -> gh error. Do NOT cache an undeterminable verdict
      # (so a transient error is retried next cycle) and fail CLOSED for now.
      return 1
      ;;
  esac
}

# issue_has_non_dispatch_metadata <issue>
#   rc 0 when issue metadata identifies an epic / standing task that the coding
#   supervisor must never claim for one-shot dispatch. Fails open when metadata
#   cannot be read so transient GitHub errors do not stall ordinary work.
issue_has_non_dispatch_metadata() {
  local issue="$1"
  [ -n "$issue" ] || return 1

  local configured
  for configured in $SUP_NON_DISPATCH_ISSUES; do
    [ "$configured" = "$issue" ] && return 0
  done

  command -v "$SUP_GH_BIN" >/dev/null 2>&1 || return 1

  local json verdict
  json=$(sup_run_timeout "$SUP_GH_TIMEOUT_SECS" "$SUP_GH_BIN" issue view "$issue" --repo "$SUP_REPO" \
    --json title,labels 2>/dev/null) || return 1
  [ -n "$json" ] || return 1

  verdict=$(printf '%s' "$json" | \
    SUP_NON_DISPATCH_LABELS="$SUP_NON_DISPATCH_LABELS" \
    SUP_NON_DISPATCH_TITLE_RE="$SUP_NON_DISPATCH_TITLE_RE" \
    python3 -c "import json, os, re, sys
try:
    row=json.load(sys.stdin)
except Exception:
    sys.exit(1)
title=row.get('title') or ''
title_re=os.environ.get('SUP_NON_DISPATCH_TITLE_RE') or ''
if title_re and re.search(title_re, title, re.I):
    print('skip'); sys.exit(0)
skip_labels={s.lower() for s in (os.environ.get('SUP_NON_DISPATCH_LABELS') or '').split() if s}
labels=row.get('labels') or []
for label in labels:
    name=(label.get('name') if isinstance(label, dict) else str(label)).lower()
    if name in skip_labels:
        print('skip'); sys.exit(0)
print('dispatch')" 2>/dev/null) || return 1
  [ "$verdict" = "skip" ]
}

# sup_open_issue_numbers
#   Prints the repo's currently-OPEN issue numbers (one per line) with a short
#   TTL cache, so the supervisor can dynamically refetch the open-issue set each
#   dispatch cycle instead of trusting a stale startup snapshot.
#   rc 0 with numbers on stdout when the open set was fetched (or served from a
#   fresh cache); rc 1 (no output) when gh is missing/errors so the caller can
#   fall back to the per-issue issue_is_open backstop.
sup_open_issue_numbers() {
  command -v "$SUP_GH_BIN" >/dev/null 2>&1 || return 1

  local cache_dir="${SUP_LOCKOUT_CACHE_DIR:-$SUP_STATE_DIR}"
  mkdir -p "$cache_dir" 2>/dev/null || true
  local cache="$cache_dir/open-issues.set"
  local ttl="${SUP_OPEN_SET_TTL_SECS:-$SUP_LOCKOUT_TTL_SECS}"
  if [ -f "$cache" ] && [ -s "$cache" ]; then
    local now age
    now=$(date +%s)
    age=$(( now - $(sup_file_mtime "$cache") ))
    if [ "$age" -ge 0 ] && [ "$age" -lt "$ttl" ]; then
      cat "$cache" 2>/dev/null
      return 0
    fi
  fi

  local json
  json=$(sup_run_timeout "$SUP_GH_TIMEOUT_SECS" "$SUP_GH_BIN" issue list --repo "$SUP_REPO" --state open \
    --limit "${SUP_OPEN_SET_LIMIT:-300}" --json number 2>/dev/null) || return 1
  [ -n "$json" ] || return 1

  local nums
  nums=$(printf '%s' "$json" | python3 -c "import sys,json
try:
    rows=json.load(sys.stdin)
except Exception:
    sys.exit(1)
if not isinstance(rows,list):
    sys.exit(1)
for r in rows:
    n=r.get('number')
    if isinstance(n,int):
        print(n)" 2>/dev/null) || return 1
  [ -n "$nums" ] || return 1
  printf '%s\n' "$nums" > "$cache" 2>/dev/null || true
  printf '%s\n' "$nums"
  return 0
}

# pick_unlocked_issue <start_index> <issue...>
#   Prints the first DISPATCHABLE issue, scanning the rotation from
#   <start_index>. An issue is dispatchable only when it is STILL OPEN and has
#   NO existing open PR. rc 0 with the issue on stdout, or rc 1 when every issue
#   is locked (closed and/or already has an open PR).
pick_unlocked_issue() {
  local start="$1"; shift
  local arr=("$@")
  local n="${#arr[@]}"
  [ "$n" -gt 0 ] || return 1
  local k i issue
  for (( k=0; k<n; k++ )); do
    i=$(( (start + k) % n ))
    issue="${arr[$i]}"
    # Epics and standing ops tasks are not one-shot coding dispatch work.
    if issue_has_non_dispatch_metadata "$issue"; then
      continue
    fi
    # Skip issues that were closed/merged during the run (stale-snapshot redo).
    if ! issue_is_open "$issue"; then
      continue
    fi
    # Skip issues that already have an open PR (duplicate-PR guard).
    if issue_has_open_pr "$issue"; then
      continue
    fi
    printf '%s' "$issue"
    return 0
  done
  return 1
}

# --- In-flight claim registry (concurrent-slot distinct-issue dispatch) ------
#
# ROOT CAUSE this fixes: the supervisor runs N worker slots as independent
# background loops. Per cycle each slot independently ordered the SAME pool by
# priority and called `pick_unlocked_issue 0 ...`, which returns the FIRST
# dispatchable issue. The only cross-slot spread was a start-index rotation that
# the priority bucket-sort then collapsed: whenever the highest non-empty tier
# held only a few issues, EVERY concurrent slot picked the SAME top issue. The
# dispatch gate admits ONE assignment per issue and refused the duplicates
# (409 / duplicate_active_assignment / target_pylon_unavailable), so N-1 slots
# backed off and the fleet under-filled (e.g. ~10 turns instead of 12+, with the
# same 1-2 issues hammered dozens of times).
#
# FIX: a shared on-disk claim registry under the state dir. Before a slot
# dispatches an issue it must ATOMICALLY acquire that issue's claim; another slot
# that already holds a fresh claim is skipped. So N concurrent slots resolve to
# up to N DISTINCT unlocked issues, restoring same-account parallelism across
# different issues. A claim is held for SUP_CLAIM_TTL_SECS (defaults to the
# dispatch timeout) so it covers the whole in-flight assignment; a refused/busy
# issue stays parked for the TTL (the fleet moves on to other work instead of
# re-hammering the stuck #6661/#6662-style issue), while a transiently
# rate-limited/errored issue is released immediately so other accounts can retry
# it. Stale claims (TTL elapsed) are garbage-collected so finished/abandoned work
# frees the issue for a later retry.
#
# Atomicity: `mkdir` is the POSIX atomic create primitive — exactly one slot wins
# the create. Staleness is handled by a separate GC sweep (sup_gc_stale_claims)
# rather than an in-acquire delete+create, so there is never a window where two
# slots both believe they own the same issue.

# Claim TTL: cover the full in-flight assignment so concurrent slots never grab
# an issue another slot is actively working. Defaults to the dispatch timeout
# when set, else 30m.
: "${SUP_CLAIM_TTL_SECS:=${SUP_DISPATCH_TIMEOUT_SECS:-1800}}"

sup_claims_dir() {
  local d="${SUP_LOCKOUT_CACHE_DIR:-$SUP_STATE_DIR}/claims"
  mkdir -p "$d" 2>/dev/null || true
  printf '%s' "$d"
}

sup_claim_owner_pid() {
  local claim="$1"
  [ -n "$claim" ] || return 1
  [ -f "$claim/meta" ] || return 1
  awk '{print $1}' "$claim/meta" 2>/dev/null
}

sup_pid_is_alive() {
  local pid="$1"
  [ "$pid" -ge 1 ] 2>/dev/null || return 1
  kill -0 "$pid" 2>/dev/null
}

# sup_gc_stale_claims
#   Remove claim entries whose age has reached SUP_CLAIM_TTL_SECS. A claim that
#   old means its dispatch (bounded by SUP_DISPATCH_TIMEOUT_SECS <= TTL) has
#   ended, so freeing it cannot race a live dispatch. Idempotent; safe to run
#   concurrently from every slot (rm is a no-op on an already-removed entry).
sup_gc_stale_claims() {
  local dir; dir="$(sup_claims_dir)"
  local now claim age owner_pid
  now=$(date +%s)
  for claim in "$dir"/claim.*; do
    [ -e "$claim" ] || continue
    owner_pid="$(sup_claim_owner_pid "$claim" 2>/dev/null || true)"
    if [ -z "$owner_pid" ] || ! sup_pid_is_alive "$owner_pid"; then
      rm -rf "$claim" 2>/dev/null || true
      continue
    fi
    age=$(( now - $(sup_file_mtime "$claim") ))
    if [ "$age" -lt 0 ] || [ "$age" -ge "$SUP_CLAIM_TTL_SECS" ]; then
      rm -rf "$claim" 2>/dev/null || true
    fi
  done
}

sup_gc_orphan_claims() {
  sup_gc_stale_claims
}

sup_gc_orphaned_claims() {
  sup_gc_orphan_claims
}

# sup_claim_is_active <issue>
#   rc 0 if a FRESH claim is held for the issue (reserved by some slot), rc 1
#   otherwise. Does not mutate; GC of stale entries is sup_gc_stale_claims's job.
sup_claim_is_active() {
  local issue="$1"; [ -n "$issue" ] || return 1
  local f; f="$(sup_claims_dir)/claim.$issue"
  [ -e "$f" ] || return 1
  local owner_pid
  owner_pid="$(sup_claim_owner_pid "$f" 2>/dev/null || true)"
  [ -n "$owner_pid" ] && sup_pid_is_alive "$owner_pid" || return 1
  local now age
  now=$(date +%s)
  age=$(( now - $(sup_file_mtime "$f") ))
  [ "$age" -ge 0 ] && [ "$age" -lt "$SUP_CLAIM_TTL_SECS" ]
}

# sup_try_claim_issue <issue> [owner]
#   ATOMICALLY acquire the in-flight claim for <issue>. rc 0 (and the claim is
#   now held by this caller) only for the single slot that wins the create;
#   rc 1 if another slot already holds a fresh claim. Relies on the caller (or a
#   preceding sup_gc_stale_claims) to have cleared expired claims first.
sup_try_claim_issue() {
  local issue="$1"; local owner="${2:-$$}"
  [ -n "$issue" ] || return 1
  if issue_has_non_dispatch_metadata "$issue"; then
    return 1
  fi
  local dir; dir="$(sup_claims_dir)"
  local claim="$dir/claim.$issue"
  if mkdir "$claim" 2>/dev/null; then
    printf '%s %s\n' "$owner" "$(date +%s)" > "$claim/meta" 2>/dev/null || true
    return 0
  fi
  return 1
}

# sup_refresh_claim <issue> [owner]
#   Bump a held claim's mtime so its TTL window restarts (e.g. on a successful
#   dispatch, keep the issue parked while its PR/lockout state settles). No-op if
#   the claim does not exist.
sup_refresh_claim() {
  local issue="$1"; [ -n "$issue" ] || return 0
  local claim; claim="$(sup_claims_dir)/claim.$issue"
  [ -e "$claim" ] || return 0
  # Bump the claim DIRECTORY's mtime (the value sup_claim_is_active /
  # sup_gc_stale_claims read); overwriting an existing inner file does not
  # reliably update the directory mtime on all filesystems.
  touch "$claim" 2>/dev/null || true
  printf '%s %s\n' "${2:-$$}" "$(date +%s)" > "$claim/meta" 2>/dev/null || true
}

# sup_release_claim <issue>
#   Free an issue's claim immediately (e.g. a transient rate-limit/error, or a
#   pre-dispatch bail) so another slot/account can retry it without waiting for
#   the TTL to elapse.
sup_release_claim() {
  local issue="$1"; [ -n "$issue" ] || return 0
  rm -rf "$(sup_claims_dir)/claim.$issue" 2>/dev/null || true
}

# pick_and_claim_unlocked_issue <start_index> <issue...>
#   Like pick_unlocked_issue, but additionally enforces cross-slot distinctness:
#   it GCs stale claims, then scans the rotation skipping issues that are CLOSED,
#   already have an open PR, OR already have a fresh in-flight claim, and
#   ATOMICALLY claims the first remaining issue before returning it. Two slots
#   racing the same ordered pool therefore resolve to DISTINCT issues. rc 0 with
#   the claimed issue on stdout; rc 1 when no distinct unlocked+unclaimed issue
#   is available.
pick_and_claim_unlocked_issue() {
  local start="$1"; shift
  local arr=("$@")
  local n="${#arr[@]}"
  [ "$n" -gt 0 ] || return 1
  sup_gc_stale_claims
  local k i issue
  for (( k=0; k<n; k++ )); do
    i=$(( (start + k) % n ))
    issue="${arr[$i]}"
    # Epics and standing ops tasks are not one-shot coding dispatch work.
    if issue_has_non_dispatch_metadata "$issue"; then
      continue
    fi
    # Already reserved by another slot this window -> spread to a distinct issue.
    if sup_claim_is_active "$issue"; then
      continue
    fi
    # Closed/merged during the run (stale-snapshot redo guard).
    if ! issue_is_open "$issue"; then
      continue
    fi
    # Already has an open PR (duplicate-PR guard).
    if issue_has_open_pr "$issue"; then
      continue
    fi
    # Atomic acquire: only the winning slot proceeds; a loser falls through to
    # the next candidate so concurrent slots never collide on one issue.
    if sup_try_claim_issue "$issue"; then
      printf '%s' "$issue"
      return 0
    fi
  done
  return 1
}
