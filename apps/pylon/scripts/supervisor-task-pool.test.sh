#!/usr/bin/env bash
#
# supervisor-task-pool.test.sh — focused tests for dynamic supervisor issue pool.
#
# Stubs curl and gh (no network) and asserts that:
#   * linked unsupported-request rows become issue numbers,
#   * duplicate refs are deduped in ledger order,
#   * closed GitHub issues are filtered out,
#   * route/auth failures fall back to the bounded fallback issue list.
#
# Run: bash apps/pylon/scripts/supervisor-task-pool.test.sh
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

PASS=0
FAIL=0
ok()  { PASS=$((PASS+1)); printf 'ok   - %s\n' "$1"; }
bad() { FAIL=$((FAIL+1)); printf 'FAIL - %s\n' "$1"; }

cat > "$WORK/curl" <<'STUB'
#!/usr/bin/env bash
if [ "${SUP_TEST_CURL_FAIL:-0}" = "1" ]; then exit 22; fi
url=""
for arg in "$@"; do
  case "$arg" in
    http*) url="$arg" ;;
  esac
done
case "$url" in
  *status=issue_opened*)
    cat "$SUP_TEST_FIXTURES/issue_opened.json"
    ;;
  *status=open*)
    cat "$SUP_TEST_FIXTURES/open.json"
    ;;
  *)
    echo '{"unsupportedRequests":[]}'
    ;;
esac
STUB
chmod +x "$WORK/curl"

cat > "$WORK/gh" <<'STUB'
#!/usr/bin/env bash
issue="$3"
case "$issue" in
  7001|7002|7004) echo "OPEN" ;;
  7003) echo "CLOSED" ;;
  *) echo "OPEN" ;;
esac
STUB
chmod +x "$WORK/gh"

FIXTURES="$WORK/fixtures"
mkdir -p "$FIXTURES"
cat > "$FIXTURES/issue_opened.json" <<'JSON'
{
  "unsupportedRequests": [
    { "githubIssueRef": "OpenAgentsInc/openagents#7001" },
    { "githubIssueRef": "https://github.com/OpenAgentsInc/openagents/issues/7002" },
    { "githubIssueRef": "#7001" },
    { "githubIssueRef": "OpenAgentsInc/openagents#7003" }
  ]
}
JSON
cat > "$FIXTURES/open.json" <<'JSON'
{
  "unsupportedRequests": [
    { "githubIssueRef": "OpenAgentsInc/openagents#7004" }
  ]
}
JSON

export PATH="$WORK:$PATH"
export SUP_TEST_FIXTURES="$FIXTURES"
export OPENAGENTS_AGENT_TOKEN="test-token"
export SUP_GH_BIN="$WORK/gh"
export SUP_REPO="OpenAgentsInc/openagents"
export SUP_STATE_DIR="$WORK/state"
export SUP_TASK_POOL_CACHE_DIR="$WORK/cache"
export SUP_TASK_POOL_CACHE_TTL_SECS=0
export SUP_TASK_POOL_FALLBACK_ISSUES="8001 8002 8001"

# shellcheck source=supervisor-task-pool.sh
source "$SCRIPT_DIR/supervisor-task-pool.sh"

got="$(supervisor_task_pool_issues | paste -sd ' ' -)"
if [ "$got" = "7001 7002 7004" ]; then
  ok "dynamic pool extracts linked open issues, dedupes, and filters closed issues"
else
  bad "dynamic pool returned '$got' (want '7001 7002 7004')"
fi

export SUP_TEST_CURL_FAIL=1
got="$(supervisor_task_pool_issues | paste -sd ' ' -)"
if [ "$got" = "8001 8002" ]; then
  ok "route failure falls back to bounded fallback issues"
else
  bad "fallback returned '$got' (want '8001 8002')"
fi

unset OPENAGENTS_AGENT_TOKEN
unset OPENAGENTS_ADMIN_API_TOKEN
unset SUP_UNSUPPORTED_REQUESTS_TOKEN
export SUP_TEST_CURL_FAIL=0
got="$(supervisor_task_pool_issues | paste -sd ' ' -)"
if [ "$got" = "8001 8002" ]; then
  ok "missing auth token falls back to bounded fallback issues"
else
  bad "missing-auth fallback returned '$got' (want '8001 8002')"
fi

printf '\n%d passed, %d failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
