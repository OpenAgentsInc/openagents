#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")"
OUTPUT_DIR="${OUTPUT_DIR:-${ROOT_DIR}/apps/openagents.com/storage/app/rust-only-terminal-gate/${TIMESTAMP}}"
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

  echo "[rust-only-terminal-gate] ${check_id}: ${status}"
}

run_step "terminal-gate-script-syntax" "Terminal gate verifier script syntax is valid" \
  bash -n apps/openagents.com/scripts/verify-rust-only-terminal-gate.sh

run_step "terminal-gate-verifier" "Active openagents.com lanes are Rust-only; legacy lanes are archived" \
  ./apps/openagents.com/scripts/verify-rust-only-terminal-gate.sh

run_step "service-compile" "Rust control service compiles" \
  cargo check --manifest-path apps/openagents.com/service/Cargo.toml

run_step "landing-routes-baseline" "Landing-mode health/readiness routes stay green" \
  cargo test --manifest-path apps/openagents.com/service/Cargo.toml healthz_route_returns_ok

jq -s \
  --arg generated_at "${TIMESTAMP}" \
  '{
    schema: "openagents.webparity.rust_only_terminal_gate.v1",
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
  echo "# Rust-Only Terminal Gate Harness"
  echo
  echo "- Generated at: ${TIMESTAMP}"
  echo "- Overall status: $(jq -r '.overall_status' "${SUMMARY_JSON}")"
  echo "- Totals: $(jq -r '.totals.passed' "${SUMMARY_JSON}") pass / $(jq -r '.totals.failed' "${SUMMARY_JSON}") fail"
  echo
  echo "| Check | Status | Description | Log |"
  echo "| --- | --- | --- | --- |"
  jq -r '.checks[] | "| \(.check_id) | \(.status) | \(.description) | `\(.log_path)` |"' "${SUMMARY_JSON}"
} >"${SUMMARY_MD}"

echo "[rust-only-terminal-gate] summary: ${SUMMARY_JSON}"
echo "[rust-only-terminal-gate] report: ${SUMMARY_MD}"

if [[ "${overall_failed}" -ne 0 ]]; then
  echo "error: rust-only terminal gate harness failed" >&2
  exit 1
fi
