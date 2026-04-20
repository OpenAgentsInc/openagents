#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

STAMP="${ISSUE_4368_PROOF_STAMP:-$(date -u +%Y%m%d%H%M%S)}"
OUT_DIR="${ISSUE_4368_PROOF_OUT_DIR:-var/proof/issue-4368-local-closure-${STAMP}}"
SMOKE_FUNDING_TIMEOUT_OUT_DIR="${OUT_DIR}/post-deploy-smoke-funding-timeout"
SMOKE_CONNECTED_INSUFFICIENT_OUT_DIR="${OUT_DIR}/post-deploy-smoke-connected-insufficient-balance"
REPLACE_NS="proof.4368.local.${STAMP}.replace"
STALE_NS="proof.4368.local.${STAMP}.stale"

mkdir -p "$OUT_DIR"

log() {
  printf '[issue-4368-local] %s\n' "$*" | tee -a "$OUT_DIR/closure.log"
}

run_logged() {
  local label="$1"
  shift
  log "running ${label}: $*"
  "$@" 2>&1 | tee "$OUT_DIR/${label}.log"
}

cleanup_namespace() {
  local namespace="$1"
  target/debug/oa proof fleet down --namespace "$namespace" --json >"$OUT_DIR/${namespace}.fleet-down.json" 2>/dev/null || true
  target/debug/oa proof authority down --namespace "$namespace" --json >"$OUT_DIR/${namespace}.authority-down.json" 2>/dev/null || true
}

cleanup() {
  cleanup_namespace "$REPLACE_NS"
  cleanup_namespace "$STALE_NS"
}
trap cleanup EXIT

copy_proof_artifacts() {
  local namespace="$1"
  local source_root="${HOME}/.openagents/pylon/proof/namespaces/${namespace}/fleet"
  mkdir -p "$OUT_DIR/${namespace}"
  cp "$source_root/run-report.json" "$OUT_DIR/${namespace}/run-report.json"
  cp "$source_root/authority-state-trace.json" "$OUT_DIR/${namespace}/authority-state-trace.json"
  cp "$source_root/proof-summary.json" "$OUT_DIR/${namespace}/proof-summary.json"
}

assert_report() {
  local label="$1"
  local report_path="$2"
  local expected_closeout="$3"

  jq -e --arg expected_closeout "$expected_closeout" '
    .status == "completed"
    and (.observed_run.caveat_count // 0) == 0
    and (.observed_run.run.latest_closeout_status // "") == $expected_closeout
  ' "$report_path" >/dev/null
  log "verified ${label}: completed caveat_count=0 closeout=${expected_closeout}"
}

log "starting issue #4368 local closure proof"
log "stamp=${STAMP}"
log "out_dir=${OUT_DIR}"
log "replacement_namespace=${REPLACE_NS}"
log "stale_namespace=${STALE_NS}"

run_logged cargo-fmt cargo fmt -p nexus-control --check
run_logged diff-check git diff --check
run_logged recovery-comparison cargo test -p nexus-control recovery_comparison -- --nocapture
run_logged recovery-cutover cargo test -p nexus-control recovery_cutover -- --nocapture
run_logged homework-pay cargo test -p nexus-control launch_homework_on_all_updated_online_pylons_and_pay_on_accept -- --nocapture
run_logged build-oa cargo build -p pylon --bin oa
run_logged build-nexus-relay cargo build -p nexus-relay --bin nexus-relay
run_logged post-deploy-smoke-funding-timeout env \
  ISSUE_4368_SMOKE_OUT_DIR="$SMOKE_FUNDING_TIMEOUT_OUT_DIR" \
  ISSUE_4368_SMOKE_SCENARIO=funding_target_timeout_status_poisoning \
  scripts/pylon/issue-4368-post-deploy-smoke-simulation.sh
run_logged post-deploy-smoke-connected-insufficient-balance env \
  ISSUE_4368_SMOKE_OUT_DIR="$SMOKE_CONNECTED_INSUFFICIENT_OUT_DIR" \
  ISSUE_4368_SMOKE_SCENARIO=connected_wallet_insufficient_balance_for_policy \
  ISSUE_4368_SMOKE_WALLET_RUNTIME_STATUS=connected \
  ISSUE_4368_SMOKE_WALLET_LAST_ERROR= \
  ISSUE_4368_SMOKE_WALLET_BALANCE_SATS=80 \
  ISSUE_4368_SMOKE_PAYOUT_SATS_PER_WINDOW=600 \
  ISSUE_4368_SMOKE_PAYOUT_LOOP_RUNTIME_STATUS=degraded \
  ISSUE_4368_SMOKE_PAYOUT_LOOP_LAST_ERROR=reconciliation_horizon_exceeded:1776162103131 \
  ISSUE_4368_SMOKE_INFERENCE_READY=49 \
  ISSUE_4368_SMOKE_PAYOUTS_DISPATCHED_24H=0 \
  ISSUE_4368_SMOKE_PAYOUTS_CONFIRMED_24H=0 \
  scripts/pylon/issue-4368-post-deploy-smoke-simulation.sh

log "running replacement-attempt proof lane"
target/debug/oa proof run cs336-a1-replacement-attempt \
  --namespace "$REPLACE_NS" \
  --workers 0 \
  --validators 0 \
  --timeout-seconds 180 \
  --json | tee "$OUT_DIR/replacement-run-output.json"
copy_proof_artifacts "$REPLACE_NS"

log "running stale-recovery proof lane"
target/debug/oa proof run cs336-a1-stale-recovery \
  --namespace "$STALE_NS" \
  --workers 1 \
  --validators 1 \
  --timeout-seconds 300 \
  --json | tee "$OUT_DIR/stale-run-output.json"
copy_proof_artifacts "$STALE_NS"

assert_report "replacement-attempt" "$OUT_DIR/${REPLACE_NS}/run-report.json" "refused"
assert_report "stale-recovery" "$OUT_DIR/${STALE_NS}/run-report.json" "rewarded"

jq -n \
  --arg generated_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --arg git_sha "$(git rev-parse HEAD)" \
  --arg out_dir "$OUT_DIR" \
  --arg replacement_namespace "$REPLACE_NS" \
  --arg stale_namespace "$STALE_NS" \
  --slurpfile smoke_funding_timeout "$SMOKE_FUNDING_TIMEOUT_OUT_DIR/post-deploy-smoke-simulation.json" \
  --slurpfile smoke_connected_insufficient "$SMOKE_CONNECTED_INSUFFICIENT_OUT_DIR/post-deploy-smoke-simulation.json" \
  --slurpfile replacement "$OUT_DIR/${REPLACE_NS}/run-report.json" \
  --slurpfile stale "$OUT_DIR/${STALE_NS}/run-report.json" \
  '{
    generated_at: $generated_at,
    issue: 4368,
    git_sha: $git_sha,
    out_dir: $out_dir,
    replacement_namespace: $replacement_namespace,
    stale_namespace: $stale_namespace,
    status: "completed",
    post_deploy_smoke_simulations: [
      {
        scenario: $smoke_funding_timeout[0].scenario,
        decision: $smoke_funding_timeout[0].decision,
        reason: $smoke_funding_timeout[0].reason,
        failure_class: $smoke_funding_timeout[0].failure_class,
        reproduced_current_failure: $smoke_funding_timeout[0].reproduced_current_failure,
        report: ($out_dir + "/post-deploy-smoke-funding-timeout/post-deploy-smoke-simulation.json")
      },
      {
        scenario: $smoke_connected_insufficient[0].scenario,
        decision: $smoke_connected_insufficient[0].decision,
        reason: $smoke_connected_insufficient[0].reason,
        failure_class: $smoke_connected_insufficient[0].failure_class,
        reproduced_current_failure: $smoke_connected_insufficient[0].reproduced_current_failure,
        wallet_balance_sats: $smoke_connected_insufficient[0].wallet_balance_sats,
        payout_sats_per_window: $smoke_connected_insufficient[0].payout_sats_per_window,
        payout_loop_runtime_status: $smoke_connected_insufficient[0].payout_loop_runtime_status,
        payout_loop_last_error: $smoke_connected_insufficient[0].payout_loop_last_error,
        report: ($out_dir + "/post-deploy-smoke-connected-insufficient-balance/post-deploy-smoke-simulation.json")
      }
    ],
    replacement: {
      status: $replacement[0].status,
      closeout: $replacement[0].observed_run.run.latest_closeout_status,
      caveat_count: $replacement[0].observed_run.caveat_count,
      run_report: ($out_dir + "/" + $replacement_namespace + "/run-report.json")
    },
    stale_recovery: {
      status: $stale[0].status,
      closeout: $stale[0].observed_run.run.latest_closeout_status,
      caveat_count: $stale[0].observed_run.caveat_count,
      accepted_contributions: $stale[0].observed_run.contribution_count,
      node_count: $stale[0].observed_run.node_count,
      run_report: ($out_dir + "/" + $stale_namespace + "/run-report.json")
    }
  }' | tee "$OUT_DIR/closure-summary.json"

log "local closure proof completed"
log "summary=${OUT_DIR}/closure-summary.json"
