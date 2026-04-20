#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

STAMP="${ISSUE_4368_SMOKE_STAMP:-$(date -u +%Y%m%d%H%M%S)}"
OUT_DIR="${ISSUE_4368_SMOKE_OUT_DIR:-var/proof/issue-4368-post-deploy-smoke-${STAMP}}"
mkdir -p "$OUT_DIR"

deployed_image="${ISSUE_4368_SMOKE_DEPLOYED_IMAGE:-local:nexus-relay-under-test}"
previous_image="${ISSUE_4368_SMOKE_PREVIOUS_IMAGE:-local:nexus-relay-previous}"
service_state="${ISSUE_4368_SMOKE_SERVICE_STATE:-active}"
service_uptime_seconds="${ISSUE_4368_SMOKE_SERVICE_UPTIME_SECONDS:-378}"
warmup_grace_seconds="${ISSUE_4368_SMOKE_WARMUP_GRACE_SECONDS:-180}"
recent_completed="${ISSUE_4368_SMOKE_RECENT_COMPLETED:-0}"
inference_ready="${ISSUE_4368_SMOKE_INFERENCE_READY:-45}"
wallet_runtime_status="${ISSUE_4368_SMOKE_WALLET_RUNTIME_STATUS:-error}"
wallet_last_error="${ISSUE_4368_SMOKE_WALLET_LAST_ERROR:-treasury_funding_target_timeout:10000}"
last_dispatch_at_unix_ms="${ISSUE_4368_SMOKE_LAST_DISPATCH_AT_UNIX_MS:-1776207790501}"

decision="wait"
reason="startup_or_pending"

if [[ "$service_state" != "active" ]]; then
  decision="wait"
  reason="service_not_active"
elif [[ "$recent_completed" =~ ^[0-9]+$ ]] && (( recent_completed > 0 )); then
  decision="pass"
  reason="fresh_completed_send"
elif [[ "$inference_ready" =~ ^[0-9]+$ ]] && (( inference_ready == 0 )) \
  && [[ "$wallet_runtime_status" == "connected" ]]; then
  decision="pass"
  reason="zero_inference_ready_targets"
elif [[ "$service_uptime_seconds" =~ ^[0-9]+$ ]] \
  && (( service_uptime_seconds < warmup_grace_seconds )); then
  decision="wait"
  reason="warmup_grace"
elif [[ -n "$previous_image" && "$previous_image" != "$deployed_image" ]]; then
  decision="rollback"
  reason="post_deploy_payout_smoke_stalled"
else
  decision="fail"
  reason="post_deploy_payout_smoke_stalled_no_rollback"
fi

report_path="${OUT_DIR}/post-deploy-smoke-simulation.json"
jq -n \
  --arg generated_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --arg deployed_image "$deployed_image" \
  --arg previous_image "$previous_image" \
  --arg service_state "$service_state" \
  --arg wallet_runtime_status "$wallet_runtime_status" \
  --arg wallet_last_error "$wallet_last_error" \
  --arg decision "$decision" \
  --arg reason "$reason" \
  --argjson service_uptime_seconds "$service_uptime_seconds" \
  --argjson warmup_grace_seconds "$warmup_grace_seconds" \
  --argjson recent_completed "$recent_completed" \
  --argjson inference_ready "$inference_ready" \
  --argjson last_dispatch_at_unix_ms "$last_dispatch_at_unix_ms" \
  '{
    generated_at: $generated_at,
    issue: 4368,
    scenario: "nexus_post_deploy_payout_smoke_failure",
    deployed_image: $deployed_image,
    previous_image: $previous_image,
    service_state: $service_state,
    service_uptime_seconds: $service_uptime_seconds,
    warmup_grace_seconds: $warmup_grace_seconds,
    recent_completed: $recent_completed,
    inference_ready_online_payout_targets: $inference_ready,
    wallet_runtime_status: $wallet_runtime_status,
    wallet_last_error: $wallet_last_error,
    last_dispatch_at_unix_ms: $last_dispatch_at_unix_ms,
    decision: $decision,
    reason: $reason,
    reproduced_current_failure: (
      $decision == "rollback"
      and $recent_completed == 0
      and $inference_ready > 0
      and $wallet_runtime_status == "error"
      and ($wallet_last_error | startswith("treasury_funding_target_timeout:"))
    )
  }' | tee "$report_path"

jq -e '
  .decision == "rollback"
  and .reproduced_current_failure == true
  and .recent_completed == 0
  and .inference_ready_online_payout_targets > 0
  and .wallet_runtime_status == "error"
' "$report_path" >/dev/null

printf '[issue-4368-smoke] reproduced Nexus post-deploy smoke failure report=%s\n' "$report_path"
