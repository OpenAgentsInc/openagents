#!/usr/bin/env bash
#
# priority-dispatch.sh — label-priority dispatch ordering for codex-supervisor (#6711)
#
# Fleet-saturation engine, part 1: the supervisor should never let a slot idle
# while higher-leverage work exists, and it should always burn the highest-value
# tier first. This helper reorders the dispatchable open-issue pool by the
# fleet-saturation priority labels:
#
#   prio:0-pr-burndown        (rank 0, highest)
#   prio:1-continual-learning (rank 1)
#   prio:2-issue-triage       (rank 2)
#   prio:3-product-promises   (rank 3)
#   prio:4-backstop-burn      (rank 4, lowest — the infinite own-capacity floor)
#   (no prio:* label)         (rank 99 — dispatched only after every tier above)
#
# The supervisor then scans the reordered pool from the top with the existing
# `pick_unlocked_issue` lockout (skip CLOSED issues and issues that already have
# an open PR), so it prefers the highest tier and FALLS THROUGH locked/empty
# tiers automatically — slots never idle while any dispatchable issue exists.
#
# Design:
#   * Sourceable + test-friendly: `gh` is indirected via $SUP_GH_BIN, and the
#     per-issue label lookup (`sup_labels_for_issue`) can be overridden in tests
#     so the ranking + ordering logic is verifiable with no network.
#   * Intra-tier round-robin spread is preserved: the input pool is rotated by
#     the caller's start index BEFORE the stable bucket-by-rank, so two slots do
#     not always collide on the same top-of-tier issue.
#   * Fails soft: when gh is missing/errors the label map is empty, every issue
#     ranks 99, and the pool order degrades to the caller's rotation — dispatch
#     still proceeds, it just loses tier preference until gh recovers.
#
# This file performs no work at source time; it only defines functions.

: "${SUP_GH_BIN:=gh}"
: "${SUP_REPO:=OpenAgentsInc/openagents}"
: "${SUP_STATE_DIR:=$HOME/.codex-supervisor}"
: "${SUP_GH_TIMEOUT_SECS:=15}"
# TTL (s) for the cached open-issue label map.
: "${SUP_PRIORITY_MAP_TTL_SECS:=${SUP_LOCKOUT_TTL_SECS:-90}}"
# Max open issues to pull labels for.
: "${SUP_PRIORITY_MAP_LIMIT:=${SUP_OPEN_SET_LIMIT:-300}}"

# Defensive fallbacks: codex-supervisor.sh sources lockout.sh (which defines
# sup_file_mtime + sup_run_timeout) BEFORE this file, so the canonical helpers
# are normally present. Define equivalents here for standalone use.
if ! command -v sup_file_mtime >/dev/null 2>&1; then
  sup_file_mtime() {
    stat -f %m "$1" 2>/dev/null || stat -c %Y "$1" 2>/dev/null || echo 0
  }
fi
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

# The five fleet-saturation tiers, highest priority first.
SUP_PRIORITY_LABELS=(
  "prio:0-pr-burndown"
  "prio:1-continual-learning"
  "prio:2-issue-triage"
  "prio:3-product-promises"
  "prio:4-backstop-burn"
)

# sup_priority_rank_for_labels <comma-separated-labels>
#   Prints the numeric priority rank for an issue's label set: the rank of the
#   highest (lowest-numbered) prio:* label present, or 99 when none is present.
#   Pure: no external calls.
sup_priority_rank_for_labels() {
  local labels=",${1:-},"
  local i
  for i in 0 1 2 3 4; do
    case "$labels" in
      *",${SUP_PRIORITY_LABELS[$i]},"*) printf '%s' "$i"; return 0 ;;
    esac
  done
  printf '99'
}

sup_issue_label_map_file() {
  local cache_dir="${SUP_LOCKOUT_CACHE_DIR:-$SUP_STATE_DIR}"
  mkdir -p "$cache_dir" 2>/dev/null || true
  printf '%s/issue-label-map.tsv' "$cache_dir"
}

# sup_fetch_issue_label_map
#   Refreshes (TTL-cached) a TSV map of the repo's OPEN issues to their labels:
#   one line per issue, "<number>\t<label1,label2,...>". rc 0 when the map is
#   fresh/written, rc 1 when gh is missing/errors (callers fail soft).
sup_fetch_issue_label_map() {
  command -v "$SUP_GH_BIN" >/dev/null 2>&1 || return 1
  local map; map="$(sup_issue_label_map_file)"
  if [ -f "$map" ] && [ -s "$map" ]; then
    local now age
    now=$(date +%s)
    age=$(( now - $(sup_file_mtime "$map") ))
    if [ "$age" -ge 0 ] && [ "$age" -lt "$SUP_PRIORITY_MAP_TTL_SECS" ]; then
      return 0
    fi
  fi
  local json
  json=$(sup_run_timeout "$SUP_GH_TIMEOUT_SECS" "$SUP_GH_BIN" issue list --repo "$SUP_REPO" --state open \
    --limit "$SUP_PRIORITY_MAP_LIMIT" --json number,labels 2>/dev/null) || return 1
  [ -n "$json" ] || return 1
  local tsv
  tsv=$(printf '%s' "$json" | python3 -c "import sys,json
try:
    rows=json.load(sys.stdin)
except Exception:
    sys.exit(1)
if not isinstance(rows,list):
    sys.exit(1)
for r in rows:
    n=r.get('number')
    if not isinstance(n,int):
        continue
    labels=[(l.get('name') or '') for l in (r.get('labels') or []) if isinstance(l,dict)]
    print(str(n)+chr(9)+','.join([x for x in labels if x]))" 2>/dev/null) || return 1
  [ -n "$tsv" ] || return 1
  printf '%s\n' "$tsv" > "$map" 2>/dev/null || true
  return 0
}

# sup_labels_for_issue <issue>
#   Prints the comma-separated label set for an issue from the cached map (built
#   by sup_fetch_issue_label_map). Prints nothing when unknown. Tests override
#   this function directly to inject labels with no gh.
sup_labels_for_issue() {
  local issue="$1"
  [ -n "$issue" ] || return 0
  local map; map="$(sup_issue_label_map_file)"
  [ -f "$map" ] || return 0
  awk -F'\t' -v n="$issue" '$1==n {print $2; exit}' "$map" 2>/dev/null
}

# sup_order_pool_by_priority <start_index> <issue...>
#   Prints the issue pool reordered by priority tier (rank 0 first, 99 last),
#   stable WITHIN each tier after a round-robin rotation by <start_index> so
#   intra-tier load is spread across slots. Issue ranks come from
#   sup_labels_for_issue. Empty pool prints nothing.
sup_order_pool_by_priority() {
  local start="$1"; shift
  local arr=("$@")
  local n="${#arr[@]}"
  [ "$n" -gt 0 ] || return 0
  [ "$start" -ge 0 ] 2>/dev/null || start=0

  # Rotate the input by start to preserve intra-tier round-robin spread.
  local rotated=()
  local k i
  for (( k=0; k<n; k++ )); do
    i=$(( (start + k) % n ))
    rotated+=("${arr[$i]}")
  done

  # Stable bucket by rank: 0,1,2,3,4 then 99 (unlabelled).
  local out=()
  local r issue rank
  for r in 0 1 2 3 4 99; do
    for issue in "${rotated[@]}"; do
      rank=$(sup_priority_rank_for_labels "$(sup_labels_for_issue "$issue")")
      if [ "$rank" = "$r" ]; then
        out+=("$issue")
      fi
    done
  done
  printf '%s\n' "${out[@]}"
}
