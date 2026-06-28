#!/usr/bin/env bash
#
# supervisor-task-pool.sh — dynamic public issue pool for owner-local supervisors.
#
# The Codex and Claude supervisors should burn down real user/agent pain first,
# not a stale hard-coded issue list. This helper reads the unsupported-requests
# ledger, extracts linked GitHub issues, filters to issues that are still open,
# and prints a deduped issue-number list for worker rotation.
#
# This file performs no work at source time; it only defines functions.

: "${PYLON_OPENAGENTS_BASE_URL:=https://openagents.com}"
: "${SUP_REPO:=OpenAgentsInc/openagents}"
: "${SUP_GH_BIN:=gh}"
: "${SUP_TASK_POOL_LIMIT:=100}"
: "${SUP_TASK_POOL_CACHE_TTL_SECS:=120}"

if ! command -v sup_file_mtime >/dev/null 2>&1; then
  sup_file_mtime() {
    stat -f %m "$1" 2>/dev/null || stat -c %Y "$1" 2>/dev/null || echo 0
  }
fi

supervisor_task_pool_cache_file() {
  local cache_dir="${SUP_TASK_POOL_CACHE_DIR:-${SUP_STATE_DIR:-$HOME/.pylon-supervisor}/task-pool-cache}"
  mkdir -p "$cache_dir" 2>/dev/null || true
  printf '%s/task-pool.issues' "$cache_dir"
}

supervisor_task_pool_auth_header() {
  local token="${SUP_UNSUPPORTED_REQUESTS_TOKEN:-${OPENAGENTS_ADMIN_API_TOKEN:-${OPENAGENTS_AGENT_TOKEN:-}}}"
  [ -n "$token" ] || return 1
  printf 'Authorization: Bearer %s' "$token"
}

supervisor_issue_numbers_from_json() {
  python3 -c "import json,re,sys
try:
    data=json.load(sys.stdin)
except Exception:
    sys.exit(0)
rows=data.get('unsupportedRequests', []) if isinstance(data, dict) else []
seen=set()
for row in rows:
    if not isinstance(row, dict):
        continue
    ref=str(row.get('githubIssueRef') or '')
    if not ref:
        continue
    match=re.search(r'(?:^|[#/])([1-9][0-9]{0,8})(?:$|[^0-9])', ref)
    if not match:
        continue
    issue=match.group(1)
    if issue in seen:
        continue
    seen.add(issue)
    print(issue)" 2>/dev/null
}

supervisor_issue_is_open() {
  local issue="$1"
  [ -n "$issue" ] || return 1
  command -v "$SUP_GH_BIN" >/dev/null 2>&1 || return 0

  local state
  state=$("$SUP_GH_BIN" issue view "$issue" --repo "$SUP_REPO" --json state \
    --jq .state 2>/dev/null | tr '[:upper:]' '[:lower:]')
  [ -z "$state" ] && return 0
  [ "$state" = "open" ]
}

supervisor_filter_open_issues() {
  local issue
  while IFS= read -r issue; do
    [ -n "$issue" ] || continue
    if supervisor_issue_is_open "$issue"; then
      printf '%s\n' "$issue"
    fi
  done
}

supervisor_dedupe_issues() {
  awk 'NF && !seen[$1]++ { print $1 }'
}

supervisor_fetch_unsupported_request_issues() {
  local auth_header
  auth_header="$(supervisor_task_pool_auth_header)" || return 1

  local url base="${PYLON_OPENAGENTS_BASE_URL%/}"
  for status in issue_opened open; do
    url="$base/api/operator/khala/unsupported-requests?status=$status&limit=$SUP_TASK_POOL_LIMIT"
    curl -fsS "$url" -H "$auth_header" 2>/dev/null | supervisor_issue_numbers_from_json
  done | supervisor_dedupe_issues | supervisor_filter_open_issues | supervisor_dedupe_issues
}

supervisor_fallback_issues() {
  local fallback="${SUP_TASK_POOL_FALLBACK_ISSUES:-6310 6311 6320 6354 6355 6358}"
  printf '%s\n' $fallback | supervisor_dedupe_issues
}

supervisor_task_pool_issues() {
  local cache
  cache="$(supervisor_task_pool_cache_file)"
  if [ -f "$cache" ]; then
    local now age
    now=$(date +%s)
    age=$(( now - $(sup_file_mtime "$cache") ))
    if [ "$age" -ge 0 ] && [ "$age" -lt "$SUP_TASK_POOL_CACHE_TTL_SECS" ] && [ -s "$cache" ]; then
      cat "$cache"
      return 0
    fi
  fi

  local fetched
  fetched="$(supervisor_fetch_unsupported_request_issues)"
  if [ -n "$fetched" ]; then
    printf '%s\n' "$fetched" > "$cache" 2>/dev/null || true
    printf '%s\n' "$fetched"
    return 0
  fi

  supervisor_fallback_issues | tee "$cache" 2>/dev/null
}
