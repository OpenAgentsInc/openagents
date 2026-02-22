#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
APP_DIR="${ROOT_DIR}/apps/openagents.com"
TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")"
OUTPUT_DIR="${OUTPUT_DIR:-${APP_DIR}/storage/app/parity-regression/${TIMESTAMP}}"
RESULTS_JSONL="${OUTPUT_DIR}/step-results.jsonl"
SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
SUMMARY_MD="${OUTPUT_DIR}/SUMMARY.md"

mkdir -p "${OUTPUT_DIR}"
: >"${RESULTS_JSONL}"

require_command() {
  local command_name="$1"
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "error: missing required command: ${command_name}" >&2
    exit 1
  fi
}

require_command cargo
require_command jq

overall_failed=0

run_step() {
  local step_id="$1"
  local category="$2"
  local description="$3"
  local command="$4"

  local log_path="${OUTPUT_DIR}/${step_id}.log"
  local started_at ended_at status reason
  started_at="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

  if (
    cd "${ROOT_DIR}"
    bash -lc "${command}"
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
    --arg step_id "${step_id}" \
    --arg category "${category}" \
    --arg description "${description}" \
    --arg command "${command}" \
    --arg status "${status}" \
    --arg reason "${reason}" \
    --arg started_at "${started_at}" \
    --arg ended_at "${ended_at}" \
    --arg log_path "${log_path}" \
    '{
      step_id: $step_id,
      category: $category,
      description: $description,
      command: $command,
      status: $status,
      reason: (if $reason == "" then null else $reason end),
      started_at: $started_at,
      ended_at: $ended_at,
      log_path: $log_path
    }' >>"${RESULTS_JSONL}"

  echo "[web-parity-regression] ${step_id}: ${status}"
}

run_step "service-compile" "api" "Rust control service compile baseline" \
  "cargo check --manifest-path apps/openagents.com/service/Cargo.toml"

run_step "web-shell-compile" "ui" "Rust/WGPUI web-shell wasm compile baseline" \
  "cargo check --target wasm32-unknown-unknown --manifest-path apps/openagents.com/web-shell/Cargo.toml"

run_step "vercel-sse-fixture-drift" "stream" "Vercel compatibility fixture corpus drift check" \
  "./apps/openagents.com/scripts/run-vercel-sse-fixture-harness.sh"

run_step "legacy-chat-alias-authority" "api" "Legacy /api/chats aliases map to codex thread authority" \
  "cargo test --manifest-path apps/openagents.com/service/Cargo.toml legacy_chats_aliases_map_to_codex_threads"

run_step "legacy-chat-stream-alias" "stream" "Legacy /api/chat/stream alias bridges to codex worker control" \
  "cargo test --manifest-path apps/openagents.com/service/Cargo.toml legacy_chat_stream_alias_bridges_to_codex_control_request"

run_step "legacy-chat-stream-path-alias" "stream" "Legacy /api/chats/{id}/stream alias uses codex thread path authority" \
  "cargo test --manifest-path apps/openagents.com/service/Cargo.toml legacy_chats_stream_alias_uses_path_thread_id_and_accepts_structured_content"

run_step "legacy-chat-retirement-invalid-payload" "stream" "Legacy stream endpoint keeps explicit retirement validation semantics" \
  "cargo test --manifest-path apps/openagents.com/service/Cargo.toml legacy_chat_stream_alias_rejects_payload_without_user_text"

run_step "codex-control-authority" "internal" "Codex app-server worker control remains canonical write authority" \
  "cargo test --manifest-path apps/openagents.com/service/Cargo.toml runtime_codex_control_request_accepts_turn_start_and_persists_message"

run_step "khala-ws-smoke-contract" "stream" "Khala WS-only smoke contract metadata remains canonical" \
  "cargo test --manifest-path apps/openagents.com/service/Cargo.toml smoke_stream_returns_khala_ws_contract_metadata"

if [[ "${OA_WEB_PARITY_FULL_SERVICE_TESTS:-0}" == "1" ]]; then
  run_step "service-full-suite" "api" "Full Rust control service regression suite" \
    "cargo test --manifest-path apps/openagents.com/service/Cargo.toml"
fi

jq -s --arg generated_at "${TIMESTAMP}" '
  def count_status(s): map(select(.status == s)) | length;
  {
    schema: "openagents.webparity.regression.v1",
    generated_at: $generated_at,
    totals: {
      step_count: length,
      passed: count_status("pass"),
      failed: count_status("fail")
    },
    overall_status: (if count_status("fail") > 0 then "failed" else "passed" end),
    steps: .
  }
' "${RESULTS_JSONL}" >"${SUMMARY_JSON}"

{
  echo "# Web Parity Regression"
  echo
  echo "- Generated at: ${TIMESTAMP}"
  echo "- Overall status: $(jq -r '.overall_status' "${SUMMARY_JSON}")"
  echo "- Totals: $(jq -r '.totals.passed' "${SUMMARY_JSON}") pass / $(jq -r '.totals.failed' "${SUMMARY_JSON}") fail"
  echo
  echo "| Step | Category | Status | Reason | Log |"
  echo "| --- | --- | --- | --- | --- |"
  jq -r '.steps[] | "| \(.step_id) | \(.category) | \(.status) | \(.reason // "") | `\(.log_path)` |"' "${SUMMARY_JSON}"
} >"${SUMMARY_MD}"

echo "[web-parity-regression] summary: ${SUMMARY_JSON}"
echo "[web-parity-regression] report: ${SUMMARY_MD}"

if [[ "${overall_failed}" -ne 0 ]]; then
  echo "error: parity regression suite detected failures" >&2
  exit 1
fi
