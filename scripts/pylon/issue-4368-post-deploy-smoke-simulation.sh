#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

STAMP="${ISSUE_4368_SMOKE_STAMP:-$(date -u +%Y%m%d%H%M%S)}"
OUT_DIR="${ISSUE_4368_SMOKE_OUT_DIR:-var/proof/issue-4368-post-deploy-smoke-${STAMP}}"
mkdir -p "$OUT_DIR"

deployed_image="${ISSUE_4368_SMOKE_DEPLOYED_IMAGE:-local:nexus-relay-under-test}"
previous_image="${ISSUE_4368_SMOKE_PREVIOUS_IMAGE:-local:nexus-relay-previous}"
scenario="${ISSUE_4368_SMOKE_SCENARIO:-funding_target_timeout_status_poisoning}"
expected_decision="${ISSUE_4368_SMOKE_EXPECT_DECISION:-rollback}"
service_state="${ISSUE_4368_SMOKE_SERVICE_STATE:-active}"
service_uptime_seconds="${ISSUE_4368_SMOKE_SERVICE_UPTIME_SECONDS:-378}"
warmup_grace_seconds="${ISSUE_4368_SMOKE_WARMUP_GRACE_SECONDS:-180}"
recent_completed="${ISSUE_4368_SMOKE_RECENT_COMPLETED:-0}"
inference_ready="${ISSUE_4368_SMOKE_INFERENCE_READY:-45}"
wallet_runtime_status="${ISSUE_4368_SMOKE_WALLET_RUNTIME_STATUS:-error}"
wallet_last_error="${ISSUE_4368_SMOKE_WALLET_LAST_ERROR-treasury_funding_target_timeout:10000}"
wallet_balance_sats="${ISSUE_4368_SMOKE_WALLET_BALANCE_SATS:-130}"
payout_sats_per_window="${ISSUE_4368_SMOKE_PAYOUT_SATS_PER_WINDOW:-600}"
placeholder_payout_mode="${ISSUE_4368_SMOKE_PLACEHOLDER_PAYOUT_MODE:-presence_only}"
payout_loop_runtime_status="${ISSUE_4368_SMOKE_PAYOUT_LOOP_RUNTIME_STATUS:-degraded}"
payout_loop_last_error="${ISSUE_4368_SMOKE_PAYOUT_LOOP_LAST_ERROR:-}"
accepted_work_pending_payout_count="${ISSUE_4368_SMOKE_ACCEPTED_WORK_PENDING_PAYOUT_COUNT:-0}"
payouts_dispatched_24h="${ISSUE_4368_SMOKE_PAYOUTS_DISPATCHED_24H:-0}"
payouts_confirmed_24h="${ISSUE_4368_SMOKE_PAYOUTS_CONFIRMED_24H:-0}"
last_dispatch_at_unix_ms="${ISSUE_4368_SMOKE_LAST_DISPATCH_AT_UNIX_MS:-1776207790501}"

decision="wait"
reason="startup_or_pending"
failure_class="none"
reproduced_current_failure="false"
expected_outcome_reproduced="false"

if [[ "$service_state" != "active" ]]; then
  decision="wait"
  reason="service_not_active"
elif [[ "$placeholder_payout_mode" == "disabled" ]] \
  && [[ "$accepted_work_pending_payout_count" =~ ^[0-9]+$ ]] \
  && (( accepted_work_pending_payout_count == 0 )); then
  decision="pass"
  reason="placeholder_payouts_disabled_no_pending_accepted_work"
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

if [[ "$recent_completed" =~ ^[0-9]+$ ]] \
  && [[ "$inference_ready" =~ ^[0-9]+$ ]] \
  && [[ "$wallet_balance_sats" =~ ^[0-9]+$ ]] \
  && [[ "$payout_sats_per_window" =~ ^[0-9]+$ ]]; then
  if [[ "$wallet_runtime_status" == "error" \
    && "$wallet_last_error" == treasury_funding_target_timeout:* \
    && "$decision" == "rollback" \
    && "$recent_completed" == "0" \
    && "$inference_ready" -gt 0 ]]; then
    failure_class="funding_target_timeout_status_poisoning"
  elif [[ "$wallet_runtime_status" == "connected" \
    && "$decision" == "rollback" \
    && "$recent_completed" == "0" \
    && "$inference_ready" -gt 0 \
    && "$wallet_balance_sats" -lt "$payout_sats_per_window" ]]; then
    failure_class="connected_wallet_insufficient_balance_for_policy"
  elif [[ "$decision" == "rollback" \
    && "$recent_completed" == "0" \
    && "$inference_ready" -gt 0 \
    && "$payout_loop_runtime_status" == "degraded" ]]; then
    failure_class="payout_dispatch_not_recovering"
  fi
fi

if [[ "$decision" == "rollback" && "$failure_class" == "$scenario" ]]; then
  reproduced_current_failure="true"
fi
if [[ "$decision" == "$expected_decision" ]]; then
  if [[ "$expected_decision" == "rollback" && "$failure_class" == "$scenario" ]]; then
    expected_outcome_reproduced="true"
  elif [[ "$expected_decision" != "rollback" ]]; then
    expected_outcome_reproduced="true"
  fi
fi

report_path="${OUT_DIR}/post-deploy-smoke-simulation.json"
jq -n \
  --arg generated_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --arg deployed_image "$deployed_image" \
  --arg previous_image "$previous_image" \
  --arg scenario "$scenario" \
  --arg expected_decision "$expected_decision" \
  --arg service_state "$service_state" \
  --arg wallet_runtime_status "$wallet_runtime_status" \
  --arg wallet_last_error "$wallet_last_error" \
  --arg placeholder_payout_mode "$placeholder_payout_mode" \
  --arg payout_loop_runtime_status "$payout_loop_runtime_status" \
  --arg payout_loop_last_error "$payout_loop_last_error" \
  --arg decision "$decision" \
  --arg reason "$reason" \
  --arg failure_class "$failure_class" \
  --argjson reproduced_current_failure "$reproduced_current_failure" \
  --argjson expected_outcome_reproduced "$expected_outcome_reproduced" \
  --argjson service_uptime_seconds "$service_uptime_seconds" \
  --argjson warmup_grace_seconds "$warmup_grace_seconds" \
  --argjson recent_completed "$recent_completed" \
  --argjson inference_ready "$inference_ready" \
  --argjson wallet_balance_sats "$wallet_balance_sats" \
  --argjson payout_sats_per_window "$payout_sats_per_window" \
  --argjson accepted_work_pending_payout_count "$accepted_work_pending_payout_count" \
  --argjson payouts_dispatched_24h "$payouts_dispatched_24h" \
  --argjson payouts_confirmed_24h "$payouts_confirmed_24h" \
  --argjson last_dispatch_at_unix_ms "$last_dispatch_at_unix_ms" \
  '{
    generated_at: $generated_at,
    issue: 4368,
    scenario: $scenario,
    expected_decision: $expected_decision,
    deployed_image: $deployed_image,
    previous_image: $previous_image,
    service_state: $service_state,
    service_uptime_seconds: $service_uptime_seconds,
    warmup_grace_seconds: $warmup_grace_seconds,
    recent_completed: $recent_completed,
    inference_ready_online_payout_targets: $inference_ready,
    wallet_runtime_status: $wallet_runtime_status,
    wallet_last_error: $wallet_last_error,
    wallet_balance_sats: $wallet_balance_sats,
    payout_sats_per_window: $payout_sats_per_window,
    placeholder_payout_mode: $placeholder_payout_mode,
    payout_loop_runtime_status: $payout_loop_runtime_status,
    payout_loop_last_error: $payout_loop_last_error,
    accepted_work_pending_payout_count: $accepted_work_pending_payout_count,
    payouts_dispatched_24h: $payouts_dispatched_24h,
    payouts_confirmed_24h: $payouts_confirmed_24h,
    last_dispatch_at_unix_ms: $last_dispatch_at_unix_ms,
    decision: $decision,
    reason: $reason,
    failure_class: $failure_class,
    reproduced_current_failure: $reproduced_current_failure,
    expected_outcome_reproduced: $expected_outcome_reproduced
  }' | tee "$report_path"

jq -e '
  if .expected_decision == "rollback" then
    .decision == "rollback"
    and .reproduced_current_failure == true
    and .expected_outcome_reproduced == true
    and .recent_completed == 0
    and .inference_ready_online_payout_targets > 0
    and .failure_class == .scenario
  else
    .decision == .expected_decision
    and .expected_outcome_reproduced == true
  end
' "$report_path" >/dev/null

printf '[issue-4368-smoke] reproduced Nexus post-deploy smoke failure report=%s\n' "$report_path"
