#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUNTIME_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

PROJECT_ID="${PROJECT_ID:-openagentsgemini}"
REGION="${REGION:-us-central1}"
KHALA_BACKEND_SERVICE="${KHALA_BACKEND_SERVICE:-oa-khala-backend-nonprod}"
KHALA_DASHBOARD_SERVICE="${KHALA_DASHBOARD_SERVICE:-oa-khala-dashboard-nonprod}"
KHALA_ADMIN_KEY_SECRET="${KHALA_ADMIN_KEY_SECRET:-oa-khala-nonprod-admin-key}"

pass_count=0
fail_count=0

require_cmd() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "Missing required command: $cmd" >&2
    exit 1
  fi
}

record_pass() {
  local message="$1"
  pass_count=$((pass_count + 1))
  echo "[PASS] $message"
}

record_fail() {
  local message="$1"
  fail_count=$((fail_count + 1))
  echo "[FAIL] $message"
}

require_cmd gcloud
require_cmd jq
require_cmd rg
require_cmd mix

if gcloud secrets describe "$KHALA_ADMIN_KEY_SECRET" --project "$PROJECT_ID" >/dev/null 2>&1; then
  record_pass "Khala admin key secret exists in Secret Manager ($KHALA_ADMIN_KEY_SECRET)."
else
  record_fail "Khala admin key secret is missing in Secret Manager ($KHALA_ADMIN_KEY_SECRET)."
fi

backend_envs="$(
  gcloud run services describe "$KHALA_BACKEND_SERVICE" \
    --project "$PROJECT_ID" \
    --region "$REGION" \
    --format=json \
    | jq -r '.spec.template.spec.containers[] | (.env // [])[]?.name'
)"

dashboard_envs="$(
  gcloud run services describe "$KHALA_DASHBOARD_SERVICE" \
    --project "$PROJECT_ID" \
    --region "$REGION" \
    --format=json \
    | jq -r '.spec.template.spec.containers[] | (.env // [])[]?.name'
)"

admin_key_pattern='^(KHALA_SELF_HOSTED_ADMIN_KEY|KHALA_ADMIN_KEY|ADMIN_KEY)$'

if printf '%s\n' "$backend_envs" | rg -q "$admin_key_pattern"; then
  record_fail "Backend service exposes an admin-key environment variable."
else
  record_pass "Backend service does not expose Khala admin key environment variables."
fi

if printf '%s\n' "$dashboard_envs" | rg -q "$admin_key_pattern"; then
  record_fail "Dashboard service exposes an admin-key environment variable."
else
  record_pass "Dashboard service does not expose Khala admin key environment variables."
fi

backend_sa="$(gcloud run services describe "$KHALA_BACKEND_SERVICE" --project "$PROJECT_ID" --region "$REGION" --format='value(spec.template.spec.serviceAccountName)')"
dashboard_sa="$(gcloud run services describe "$KHALA_DASHBOARD_SERVICE" --project "$PROJECT_ID" --region "$REGION" --format='value(spec.template.spec.serviceAccountName)')"

if [[ -z "$backend_sa" || "$backend_sa" == *"-compute@developer.gserviceaccount.com" ]]; then
  record_fail "Backend service account is not least-privilege scoped ($backend_sa)."
else
  record_pass "Backend service uses dedicated service account ($backend_sa)."
fi

if [[ -z "$dashboard_sa" || "$dashboard_sa" == *"-compute@developer.gserviceaccount.com" ]]; then
  record_fail "Dashboard service account is not least-privilege scoped ($dashboard_sa)."
else
  record_pass "Dashboard service uses dedicated service account ($dashboard_sa)."
fi

backend_container_names="$(
  gcloud run services describe "$KHALA_BACKEND_SERVICE" \
    --project "$PROJECT_ID" \
    --region "$REGION" \
    --format=json \
    | jq -r '.spec.template.spec.containers[].name'
)"

if printf '%s\n' "$backend_container_names" | rg -q '^cloud-sql-proxy$'; then
  record_pass "Backend service includes cloud-sql-proxy sidecar for constrained DB connectivity."
else
  record_fail "Backend service is missing cloud-sql-proxy sidecar."
fi

if "$SCRIPT_DIR/mcp-production-access-gate.sh" >/dev/null 2>&1; then
  record_fail "MCP production access gate did not deny default invocation."
else
  record_pass "MCP production access is denied by default."
fi

if (
  cd "$RUNTIME_DIR"
  mix test \
    test/openagents_runtime/security/sanitizer_test.exs \
    test/openagents_runtime/security/sanitization_integration_test.exs \
    test/openagents_runtime/deploy/network_policy_assets_test.exs >/dev/null
); then
  record_pass "Runtime sanitizer and network policy checks passed."
else
  record_fail "Runtime sanitizer/network policy checks failed."
fi

echo
echo "Security review checklist summary:"
echo "  passes: $pass_count"
echo "  fails:  $fail_count"

if (( fail_count > 0 )); then
  exit 1
fi
