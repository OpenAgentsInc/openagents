#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")"
OUTPUT_DIR="${OUTPUT_DIR:-${ROOT_DIR}/apps/openagents.com/storage/app/webhook-parity-harness/${TIMESTAMP}}"
RESULTS_JSONL="${OUTPUT_DIR}/checks.jsonl"
SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
SUMMARY_MD="${OUTPUT_DIR}/SUMMARY.md"

mkdir -p "${OUTPUT_DIR}"
: >"${RESULTS_JSONL}"

overall_failed=0

run_step() {
  local check_id="$1"
  local description="$2"
  shift 2

  local log_path="${OUTPUT_DIR}/${check_id}.log"
  local started_at ended_at status reason
  started_at="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

  if (
    cd "${ROOT_DIR}"
    "$@"
  ) >"${log_path}" 2>&1; then
    status="pass"
    reason=""
  else
    status="fail"
    reason="command_failed"
    overall_failed=1
  fi

  ended_at="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

  jq -n \
    --arg check_id "${check_id}" \
    --arg description "${description}" \
    --arg command "$(printf '%q ' "$@")" \
    --arg status "${status}" \
    --arg reason "${reason}" \
    --arg started_at "${started_at}" \
    --arg ended_at "${ended_at}" \
    --arg log_path "${log_path}" \
    '{
      check_id: $check_id,
      description: $description,
      command: $command,
      status: $status,
      reason: (if $reason == "" then null else $reason end),
      started_at: $started_at,
      ended_at: $ended_at,
      log_path: $log_path
    }' >>"${RESULTS_JSONL}"

  echo "[webhook-parity-harness] ${check_id}: ${status}"
}

run_step "signature-invalid" "Invalid webhook signature is rejected and replay stays invalid" \
  cargo test --manifest-path apps/openagents.com/service/Cargo.toml resend_webhook_rejects_invalid_signature_and_reuses_audit_event_id

run_step "signature-stale" "Stale Svix timestamp fails signature tolerance validation" \
  cargo test --manifest-path apps/openagents.com/service/Cargo.toml resend_webhook_rejects_stale_timestamp_signature

run_step "idempotency-replay-conflict" "Webhook idempotency replay and conflict semantics are enforced" \
  cargo test --manifest-path apps/openagents.com/service/Cargo.toml resend_webhook_deduplicates_replays_and_detects_conflicts

run_step "retry-forwarding-projection" "Forwarding retries complete and delivery projection is updated" \
  cargo test --manifest-path apps/openagents.com/service/Cargo.toml resend_webhook_forwarding_retries_and_projects_delivery

run_step "retry-state-transition" "Retrying transition is persisted before final forwarding success" \
  cargo test --manifest-path apps/openagents.com/service/Cargo.toml resend_webhook_records_forward_retrying_state_before_success

jq -s \
  --arg generated_at "${TIMESTAMP}" \
  '{
    schema: "openagents.webparity.webhook_parity_harness.v1",
    generated_at: $generated_at,
    totals: {
      check_count: length,
      passed: (map(select(.status == "pass")) | length),
      failed: (map(select(.status == "fail")) | length)
    },
    overall_status: (if (map(select(.status == "fail")) | length) > 0 then "failed" else "passed" end),
    checks: .
  }' "${RESULTS_JSONL}" >"${SUMMARY_JSON}"

{
  echo "# Webhook Parity Harness"
  echo
  echo "- Generated at: ${TIMESTAMP}"
  echo "- Overall status: $(jq -r '.overall_status' "${SUMMARY_JSON}")"
  echo "- Totals: $(jq -r '.totals.passed' "${SUMMARY_JSON}") pass / $(jq -r '.totals.failed' "${SUMMARY_JSON}") fail"
  echo
  echo "| Check | Status | Description | Log |"
  echo "| --- | --- | --- | --- |"
  jq -r '.checks[] | "| \(.check_id) | \(.status) | \(.description) | `\(.log_path)` |"' "${SUMMARY_JSON}"
} >"${SUMMARY_MD}"

echo "[webhook-parity-harness] summary: ${SUMMARY_JSON}"
echo "[webhook-parity-harness] report: ${SUMMARY_MD}"

if [[ "${overall_failed}" -ne 0 ]]; then
  echo "error: webhook parity harness failed" >&2
  exit 1
fi
