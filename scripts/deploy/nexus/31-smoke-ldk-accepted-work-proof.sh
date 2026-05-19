#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd curl
require_cmd jq

NEXUS_BASE_URL="${NEXUS_BASE_URL:-${NEXUS_PUBLIC_URL:-https://nexus.openagents.com}}"
NEXUS_BASE_URL="${NEXUS_BASE_URL%/}"
NEXUS_ADMIN_TOKEN="${NEXUS_CONTROL_ADMIN_BEARER_TOKEN:-${NEXUS_ADMIN_BEARER_TOKEN:-}}"
NEXUS_LDK_ACCEPTED_WORK_MODE="${NEXUS_LDK_ACCEPTED_WORK_MODE:-launch}"
NEXUS_LDK_ACCEPTED_WORK_ARTIFACT_DIR="${NEXUS_LDK_ACCEPTED_WORK_ARTIFACT_DIR:-docs/reports/nexus/ldk-accepted-work-smoke-$(date -u +%Y%m%dT%H%M%SZ)}"
NEXUS_LDK_ACCEPTED_WORK_AMOUNT_SATS="${NEXUS_LDK_ACCEPTED_WORK_AMOUNT_SATS:-1}"
NEXUS_LDK_ACCEPTED_WORK_RUN_COUNT="${NEXUS_LDK_ACCEPTED_WORK_RUN_COUNT:-1}"
NEXUS_LDK_ACCEPTED_WORK_CONTRIBUTORS="${NEXUS_LDK_ACCEPTED_WORK_CONTRIBUTORS:-1}"
NEXUS_LDK_ACCEPTED_WORK_WINDOW_SECONDS="${NEXUS_LDK_ACCEPTED_WORK_WINDOW_SECONDS:-1800}"
NEXUS_LDK_ACCEPTED_WORK_MIN_PYLON_VERSION="${NEXUS_LDK_ACCEPTED_WORK_MIN_PYLON_VERSION:-0.1.7}"
NEXUS_LDK_ACCEPTED_WORK_RUN_SLUG_PREFIX="${NEXUS_LDK_ACCEPTED_WORK_RUN_SLUG_PREFIX:-ldk.accepted_work.smoke}"
NEXUS_LDK_ACCEPTED_WORK_DISPLAY_PREFIX="${NEXUS_LDK_ACCEPTED_WORK_DISPLAY_PREFIX:-LDK accepted-work smoke}"
NEXUS_LDK_ACCEPTED_WORK_POLL_SECONDS="${NEXUS_LDK_ACCEPTED_WORK_POLL_SECONDS:-15}"
NEXUS_LDK_ACCEPTED_WORK_TIMEOUT_SECONDS="${NEXUS_LDK_ACCEPTED_WORK_TIMEOUT_SECONDS:-1800}"
NEXUS_LDK_ACCEPTED_WORK_HTTP_TIMEOUT_SECONDS="${NEXUS_LDK_ACCEPTED_WORK_HTTP_TIMEOUT_SECONDS:-30}"
NEXUS_LDK_ACCEPTED_WORK_REQUIRE_READY="${NEXUS_LDK_ACCEPTED_WORK_REQUIRE_READY:-true}"
NEXUS_LDK_ACCEPTED_WORK_RUN_ID="${NEXUS_LDK_ACCEPTED_WORK_RUN_ID:-}"

mkdir -p "$NEXUS_LDK_ACCEPTED_WORK_ARTIFACT_DIR"

api_url() {
  printf '%s%s\n' "$NEXUS_BASE_URL" "$1"
}

curl_json() {
  local method="$1"
  local path="$2"
  local body="$3"
  local output="$4"
  local -a args
  args=(
    --connect-timeout 10
    --max-time "$NEXUS_LDK_ACCEPTED_WORK_HTTP_TIMEOUT_SECONDS"
    -fsS
    -X "$method"
    -H 'content-type: application/json'
  )
  if [[ -n "$body" ]]; then
    args+=(--data "$body")
  fi
  curl "${args[@]}" "$(api_url "$path")" -o "$output"
}

curl_admin_json() {
  local path="$1"
  local body="$2"
  local output="$3"
  [[ -n "$NEXUS_ADMIN_TOKEN" ]] || die "Set NEXUS_CONTROL_ADMIN_BEARER_TOKEN or NEXUS_ADMIN_BEARER_TOKEN"
  curl \
    --connect-timeout 10 \
    --max-time "$NEXUS_LDK_ACCEPTED_WORK_HTTP_TIMEOUT_SECONDS" \
    -fsS \
    -X POST \
    -H 'content-type: application/json' \
    -H "authorization: Bearer ${NEXUS_ADMIN_TOKEN}" \
    --data "$body" \
    "$(api_url "$path")" \
    -o "$output"
}

write_receipt() {
  local status="$1"
  local reason="$2"
  local receipt_path="${NEXUS_LDK_ACCEPTED_WORK_ARTIFACT_DIR}/receipt.json"
  local treasury_path="${NEXUS_LDK_ACCEPTED_WORK_ARTIFACT_DIR}/treasury-status.json"
  local run_path="${NEXUS_LDK_ACCEPTED_WORK_ARTIFACT_DIR}/run-detail.json"
  local dispatch_path="${NEXUS_LDK_ACCEPTED_WORK_ARTIFACT_DIR}/dispatch.json"

  jq -n \
    --arg status "$status" \
    --arg reason "$reason" \
    --arg generated_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    --arg mode "$NEXUS_LDK_ACCEPTED_WORK_MODE" \
    --arg base_url "$NEXUS_BASE_URL" \
    --arg run_id "$NEXUS_LDK_ACCEPTED_WORK_RUN_ID" \
    --slurpfile treasury "$treasury_path" \
    --slurpfile run "$run_path" \
    --slurpfile dispatch "$dispatch_path" '
      ($run[0] // {}) as $run_detail |
      ($run_detail.featured_window // {}) as $window |
      (($run_detail.contributions // []) | map(select(.accepted_for_aggregation == true or .validator_disposition == "accepted")) | .[0] // {}) as $contribution |
      (($run_detail.payouts // []) | map(select(.status == "confirmed" and .reconciliation_status == "settled")) | .[0] // {}) as $settled_payout |
      {
        status: $status,
        reason: (if $reason == "" then null else $reason end),
        generated_at,
        mode: $mode,
        nexus_base_url: $base_url,
        urls: {
          treasury_status: ($base_url + "/v1/treasury/status"),
          run_detail: (if (($run_detail.training_run_id // $run_id) | length) > 0 then ($base_url + "/api/training/runs/" + ($run_detail.training_run_id // $run_id)) else null end)
        },
        run_id: ($run_detail.training_run_id // $run_id),
        window_id: ($window.window_id // $run_detail.featured_window_id // null),
        window_status: ($window.status // null),
        window_closeout_status: ($window.closeout_status // null),
        contribution_id: ($contribution.contribution_id // null),
        contribution_validator_disposition: ($contribution.validator_disposition // null),
        contribution_accepted_for_aggregation: ($contribution.accepted_for_aggregation // null),
        payout_key: ($settled_payout.payout_key // null),
        amount_sats: ($settled_payout.amount_sats // null),
        payout_status: ($settled_payout.status // null),
        payout_reconciliation_status: ($settled_payout.reconciliation_status // null),
        treasury: {
          active_treasury_provider: ($treasury[0].active_treasury_provider // null),
          active_treasury_rail: ($treasury[0].active_treasury_rail // null),
          ldk_readiness_state: ($treasury[0].ldk_readiness.state // null),
          wallet_runtime_status: ($treasury[0].wallet_runtime_status // null),
          payout_loop_health: ($treasury[0].payout_loop_health // null),
          accepted_work_pending_payout_count: ($treasury[0].training_payout_ledger_summary.accepted_work_pending_payout_count // null),
          current_ldk_attention_payout_count: ($treasury[0].training_payout_ledger_summary.current_ldk_attention_payout_count // null)
        },
        dispatch: ($dispatch[0] // null)
      }' >"$receipt_path"
}

fail_with_receipt() {
  local reason="$1"
  write_receipt "failed" "$reason"
  die "${reason}. Receipt: ${NEXUS_LDK_ACCEPTED_WORK_ARTIFACT_DIR}/receipt.json"
}

treasury_status_path="${NEXUS_LDK_ACCEPTED_WORK_ARTIFACT_DIR}/treasury-status.json"
run_detail_path="${NEXUS_LDK_ACCEPTED_WORK_ARTIFACT_DIR}/run-detail.json"
dispatch_path="${NEXUS_LDK_ACCEPTED_WORK_ARTIFACT_DIR}/dispatch.json"
printf '{}\n' >"$run_detail_path"
printf '{}\n' >"$dispatch_path"

curl_json GET /v1/treasury/status "" "$treasury_status_path"
jq -e '.active_treasury_provider == "ldk" and .active_treasury_rail == "ldk"' "$treasury_status_path" >/dev/null \
  || fail_with_receipt "treasury_not_on_ldk_rail"

if [[ "$NEXUS_LDK_ACCEPTED_WORK_REQUIRE_READY" == "true" ]]; then
  jq -e '.ldk_readiness.state == "ready"' "$treasury_status_path" >/dev/null \
    || fail_with_receipt "ldk_readiness_not_ready"
fi

case "$NEXUS_LDK_ACCEPTED_WORK_MODE" in
  launch)
    body="$(jq -n \
      --argjson run_count "$NEXUS_LDK_ACCEPTED_WORK_RUN_COUNT" \
      --argjson contributors "$NEXUS_LDK_ACCEPTED_WORK_CONTRIBUTORS" \
      --argjson amount_sats "$NEXUS_LDK_ACCEPTED_WORK_AMOUNT_SATS" \
      --argjson window_seconds "$NEXUS_LDK_ACCEPTED_WORK_WINDOW_SECONDS" \
      --arg min_version "$NEXUS_LDK_ACCEPTED_WORK_MIN_PYLON_VERSION" \
      --arg slug_prefix "$NEXUS_LDK_ACCEPTED_WORK_RUN_SLUG_PREFIX" \
      --arg display_prefix "$NEXUS_LDK_ACCEPTED_WORK_DISPLAY_PREFIX" \
      '{
        run_count: $run_count,
        max_contributors_per_run: $contributors,
        amount_sats: $amount_sats,
        total_budget_sats: ($run_count * $contributors * $amount_sats),
        run_slug_prefix: $slug_prefix,
        display_name_prefix: $display_prefix,
        reuse_existing_run: false,
        only_online: true,
        min_pylon_version: $min_version,
        require_updated_build: false,
        window_duration_seconds: $window_seconds,
        continue_on_error: false
      }')"
    curl_admin_json /v1/admin/homework/cs336-a1/dispatch "$body" "$dispatch_path"
    NEXUS_LDK_ACCEPTED_WORK_RUN_ID="$(jq -r '.launches[0].training_run_id // empty' "$dispatch_path")"
    [[ -n "$NEXUS_LDK_ACCEPTED_WORK_RUN_ID" ]] || fail_with_receipt "dispatch_returned_no_training_run_id"
    ;;
  verify-existing)
    [[ -n "$NEXUS_LDK_ACCEPTED_WORK_RUN_ID" ]] || die "Set NEXUS_LDK_ACCEPTED_WORK_RUN_ID for verify-existing mode"
    ;;
  *)
    die "Unknown NEXUS_LDK_ACCEPTED_WORK_MODE: ${NEXUS_LDK_ACCEPTED_WORK_MODE}"
    ;;
esac

deadline=$(( $(date +%s) + NEXUS_LDK_ACCEPTED_WORK_TIMEOUT_SECONDS ))
while true; do
  curl_json GET "/api/training/runs/${NEXUS_LDK_ACCEPTED_WORK_RUN_ID}?refresh=true" "" "$run_detail_path"
  curl_json GET /v1/treasury/status "" "$treasury_status_path"

  if jq -e '
      (.featured_window.status == "reconciled")
      and (.featured_window.closeout_status == "rewarded" or (.featured_window.payout_eligible == true and .featured_window.status == "reconciled"))
      and ((.contributions // []) | any(.accepted_for_aggregation == true or .validator_disposition == "accepted"))
      and ((.payouts // []) | any(.status == "confirmed" and .reconciliation_status == "settled"))
    ' "$run_detail_path" >/dev/null; then
    write_receipt "passed" ""
    log "LDK accepted-work proof smoke passed"
    log "Receipt: ${NEXUS_LDK_ACCEPTED_WORK_ARTIFACT_DIR}/receipt.json"
    exit 0
  fi

  if (( $(date +%s) >= deadline )); then
    fail_with_receipt "accepted_work_proof_timeout"
  fi

  sleep "$NEXUS_LDK_ACCEPTED_WORK_POLL_SECONDS"
done
