#!/usr/bin/env bash
# Recurring Nexus/Pylon LDK accepted-work proof smoke.
#
# Proves the full chain:
#   fresh targeted training run -> worker claim -> validator closeout
#   -> rewarded window -> confirmed and settled LDK payout
#
# Usage:
#   scripts/nexus/ldk-accepted-work-proof-smoke.sh [--lane LANE] [--timeout SECONDS]
#
# Environment:
#   OA_PROOF_LANE       proof lane to run (default: cs336-a1-hosted-starter)
#   OA_PROOF_TIMEOUT    timeout in seconds (default: 600)
#   OA_PROOF_NAMESPACE  proof namespace override
#   OA_PROOF_ARTIFACTS  directory to save receipts
#                       (default: docs/reports/nexus/)
#
# Exit codes:
#   0  proof passed
#   1  proof failed — see receipt and links printed at exit
#   2  setup error (binary not found, etc.)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

LANE="${OA_PROOF_LANE:-cs336-a1-hosted-starter}"
TIMEOUT="${OA_PROOF_TIMEOUT:-600}"
ARTIFACTS_DIR="${OA_PROOF_ARTIFACTS:-${REPO_ROOT}/docs/reports/nexus}"
NEXUS_BASE="${OA_NEXUS_BASE:-https://nexus.openagents.com}"

TIMESTAMP="$(date -u +%Y%m%d-%H%M%S)"
RECEIPT_PATH="${ARTIFACTS_DIR}/ldk-accepted-work-smoke-${TIMESTAMP}.json"

cd "${REPO_ROOT}"

OA_BIN="${OA_BIN:-}"
if [[ -z "${OA_BIN}" ]]; then
  if command -v oa &>/dev/null; then
    OA_BIN="oa"
  elif [[ -x "${REPO_ROOT}/target/debug/oa" ]]; then
    OA_BIN="${REPO_ROOT}/target/debug/oa"
  elif [[ -x "${REPO_ROOT}/target/release/oa" ]]; then
    OA_BIN="${REPO_ROOT}/target/release/oa"
  else
    echo "ERROR: oa binary not found. Build with: cargo build -p pylon --bin oa" >&2
    exit 2
  fi
fi

mkdir -p "${ARTIFACTS_DIR}"

echo "=== Nexus/Pylon LDK accepted-work proof smoke ==="
echo "Lane:    ${LANE}"
echo "Timeout: ${TIMEOUT}s"
echo "Receipt: ${RECEIPT_PATH}"
echo ""

PROOF_ARGS=(
  proof run
  --lane "${LANE}"
  --timeout "${TIMEOUT}"
  --json
)
if [[ -n "${OA_PROOF_NAMESPACE:-}" ]]; then
  PROOF_ARGS+=(--namespace "${OA_PROOF_NAMESPACE}")
fi

if ! "${OA_BIN}" "${PROOF_ARGS[@]}" >"${RECEIPT_PATH}" 2>&1; then
  echo ""
  echo "FAIL: proof run exited non-zero"
  echo "Receipt:        ${RECEIPT_PATH}"
  echo "Nexus stats:    ${NEXUS_BASE}/api/stats"
  echo "Treasury:       ${NEXUS_BASE}/v1/treasury/status"
  exit 1
fi

if ! command -v jq &>/dev/null; then
  echo "PASS: proof run completed (install jq for structured validation)"
  echo "Receipt: ${RECEIPT_PATH}"
  exit 0
fi

STATUS="$(jq -r '.status // "unknown"' "${RECEIPT_PATH}")"
DETAIL="$(jq -r '.detail // ""' "${RECEIPT_PATH}")"
LANE_OUT="$(jq -r '.lane // ""' "${RECEIPT_PATH}")"
RUN_ID="$(jq -r '.launch.training_run_id // .observed_run.run.run_id // "none"' "${RECEIPT_PATH}")"
WINDOW_ID="$(jq -r '.observed_run.run.current_window_id // "none"' "${RECEIPT_PATH}")"

echo "Status:    ${STATUS}"
echo "Lane:      ${LANE_OUT}"
echo "Run ID:    ${RUN_ID}"
echo "Window ID: ${WINDOW_ID}"
[[ -n "${DETAIL}" ]] && echo "Detail:    ${DETAIL}"

if [[ "${STATUS}" != "completed" ]]; then
  BLOCKER="$(jq -r '.blocker_id // "none"' "${RECEIPT_PATH}")"
  echo ""
  echo "FAIL: proof did not complete (status=${STATUS}, blocker=${BLOCKER})"
  echo "Receipt:     ${RECEIPT_PATH}"
  echo "Nexus stats: ${NEXUS_BASE}/api/stats"
  echo "Treasury:    ${NEXUS_BASE}/v1/treasury/status"
  if [[ "${RUN_ID}" != "none" ]]; then
    echo "Run detail:  ${NEXUS_BASE}/api/training/runs/${RUN_ID}"
  fi
  exit 1
fi

echo ""
echo "PASS: proof completed — ${LANE_OUT}"
echo "Receipt: ${RECEIPT_PATH}"
