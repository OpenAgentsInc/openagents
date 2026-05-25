#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ARTIFACTS_DIR="${OPENAGENTS_PYLON_ACCEPTED_WORK_HARNESS_ARTIFACTS_DIR:-${ROOT_DIR}/target/pylon-ldk-accepted-work-payout/latest}"

cd "$ROOT_DIR"

OPENAGENTS_PYLON_LDK_HARNESS_ARTIFACTS_DIR="$ARTIFACTS_DIR" \
  scripts/pylon/ldk-wallet-regtest-harness.sh

SUMMARY_PATH="${ARTIFACTS_DIR}/harness-summary.json"

jq -e '
  .accepted_work.no_manual_external_payout_destination == true
  and .accepted_work.wallet_registration.wallet_registration_mode == "wallet_generated_bolt11_fallback"
  and .accepted_work.treasury_dispatch.amount_sats > 0
  and .accepted_work.pylon_observation.wallet_history_status == "Succeeded"
  and .accepted_work.pylon_observation.balance_increase_sats == .accepted_work.treasury_dispatch.amount_sats
  and .accepted_work.withdrawal.balance_decreased == true
  and .accepted_work.reconciliation.reconciliation_status == "settled"
' "$SUMMARY_PATH" >/dev/null

echo "Pylon accepted-work LDK payout harness complete."
echo "Summary: ${SUMMARY_PATH}"
