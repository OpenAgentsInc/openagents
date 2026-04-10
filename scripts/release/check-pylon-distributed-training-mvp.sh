#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")-$$"
OUTPUT_DIR="${OPENAGENTS_PYLON_TRAINING_REHEARSAL_OUTPUT_DIR:-$ROOT_DIR/target/pylon-distributed-training-rehearsal/$TIMESTAMP}"
GIT_COMMON_DIR="$(git -C "$ROOT_DIR" rev-parse --git-common-dir 2>/dev/null || true)"
if [[ -n "$GIT_COMMON_DIR" ]]; then
  if [[ "$GIT_COMMON_DIR" = /* ]]; then
    COMMON_GIT_DIR="$GIT_COMMON_DIR"
  else
    COMMON_GIT_DIR="$ROOT_DIR/$GIT_COMMON_DIR"
  fi
  COMMON_REPO_ROOT="$(cd "$COMMON_GIT_DIR/.." && pwd)"
  WORKSPACE_ROOT="$(cd "$COMMON_REPO_ROOT/.." && pwd)"
else
  WORKSPACE_ROOT="$(cd "$ROOT_DIR/.." && pwd)"
fi
DEFAULT_PSIONIC_REPO="$ROOT_DIR/../psionic"
if [[ ! -f "$DEFAULT_PSIONIC_REPO/Cargo.toml" ]] && [[ -f "$WORKSPACE_ROOT/psionic/Cargo.toml" ]]; then
  DEFAULT_PSIONIC_REPO="$WORKSPACE_ROOT/psionic"
fi
PSIONIC_REPO="${OPENAGENTS_PSIONIC_REPO:-$DEFAULT_PSIONIC_REPO}"
PSIONIC_MANIFEST="${OPENAGENTS_PSIONIC_MANIFEST:-$PSIONIC_REPO/Cargo.toml}"

usage() {
  cat <<'USAGE'
Usage:
  scripts/release/check-pylon-distributed-training-mvp.sh [options]

Runs the retained distributed-training MVP rehearsal matrix across the
standalone Psionic runtime seam plus the Pylon and Nexus control-plane seam.

Options:
  --output-dir <path>         Write summary artifacts under this directory.
  --psionic-repo <path>       Override the sibling Psionic checkout.
  --psionic-manifest <path>   Override the Psionic Cargo.toml path.
  -h, --help                  Show this help text.

Environment:
  OPENAGENTS_PYLON_TRAINING_REHEARSAL_OUTPUT_DIR
  OPENAGENTS_PSIONIC_REPO
  OPENAGENTS_PSIONIC_MANIFEST
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --output-dir)
      OUTPUT_DIR="$2"
      shift 2
      ;;
    --psionic-repo)
      PSIONIC_REPO="$2"
      PSIONIC_MANIFEST="$PSIONIC_REPO/Cargo.toml"
      shift 2
      ;;
    --psionic-manifest)
      PSIONIC_MANIFEST="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if ! command -v cargo >/dev/null 2>&1; then
  echo "missing required command: cargo" >&2
  exit 2
fi
if [[ ! -f "$PSIONIC_MANIFEST" ]]; then
  echo "missing standalone Psionic manifest: $PSIONIC_MANIFEST" >&2
  echo "set OPENAGENTS_PSIONIC_REPO or OPENAGENTS_PSIONIC_MANIFEST" >&2
  exit 2
fi

mkdir -p "$OUTPUT_DIR"

SUMMARY_MD="$OUTPUT_DIR/SUMMARY.md"
STEPS_TSV="$OUTPUT_DIR/steps.tsv"
: >"$STEPS_TSV"

printf '# Pylon Distributed Training Rehearsal Summary\n\n' >"$SUMMARY_MD"
printf -- '- timestamp_utc: `%s`\n' "$TIMESTAMP" >>"$SUMMARY_MD"
printf -- '- output_dir: `%s`\n' "$OUTPUT_DIR" >>"$SUMMARY_MD"
printf -- '- psionic_manifest: `%s`\n\n' "$PSIONIC_MANIFEST" >>"$SUMMARY_MD"

declare -i failed=0

record_step() {
  local id="$1"
  local name="$2"
  local status="$3"
  local elapsed="$4"
  local log_path="$5"
  printf '%s\t%s\t%s\t%s\t%s\n' "$id" "$name" "$status" "$elapsed" "$log_path" >>"$STEPS_TSV"
}

run_step() {
  local id="$1"
  local name="$2"
  local command="$3"
  local log_path="$OUTPUT_DIR/${id}.log"
  local started ended elapsed status

  started="$(date +%s)"
  echo "==> [$id] $name"
  echo "    command: $command"
  if (cd "$ROOT_DIR" && bash -lc "$command") >"$log_path" 2>&1; then
    status="pass"
  else
    status="fail"
    failed=1
  fi
  ended="$(date +%s)"
  elapsed=$((ended - started))

  record_step "$id" "$name" "$status" "$elapsed" "$log_path"

  printf -- '- `%s` %s (%ss)\n' "$id" "$status" "$elapsed" >>"$SUMMARY_MD"
  printf -- '  - %s\n' "$name" >>"$SUMMARY_MD"
  printf -- '  - command: `%s`\n' "$command" >>"$SUMMARY_MD"
  printf -- '  - log: `%s`\n' "$log_path" >>"$SUMMARY_MD"
  if [[ "$status" == "fail" ]]; then
    printf -- '  - tail:\n\n```text\n' >>"$SUMMARY_MD"
    tail -n 80 "$log_path" >>"$SUMMARY_MD" || true
    printf '```\n' >>"$SUMMARY_MD"
  fi
  printf '\n' >>"$SUMMARY_MD"

  if [[ "$status" == "pass" ]]; then
    echo "    -> PASS ($log_path)"
  else
    echo "    -> FAIL ($log_path)"
  fi
}

run_step \
  "proving_slice" \
  "First proving slice: one worker, one validator, one window, one checkpoint, one durable upload, one closeout, one TRN trail" \
  "cargo test --manifest-path '$PSIONIC_MANIFEST' -p psionic-train --lib launch_manifest_requires_explicit_output_root -- --nocapture && cargo test --manifest-path '$PSIONIC_MANIFEST' -p psionic-train --lib worker_manifest_requires_node_pubkey -- --nocapture && cargo test --manifest-path '$PSIONIC_MANIFEST' -p psionic-train --lib validator_manifest_requires_replay_target_paths -- --nocapture && cargo test --manifest-path '$PSIONIC_MANIFEST' -p psionic-train --lib sync_checkpoint_writeback_publishes_atomically -- --nocapture && cargo test -p pylon --lib training_artifact_courier_uploads_downloads_and_verifies_bundles -- --nocapture && cargo test -p pylon --lib training_checkpoint_server_serves_local_checkpoint_paths -- --nocapture && cargo test -p nexus-control --lib training_window_routes_plan_activate_seal_and_reconcile -- --nocapture && cargo test -p nexus-control --lib publish_training_trn_state_publishes_and_reuses_authoritative_coordinator_events -- --nocapture"

run_step \
  "multi_node_and_rejoin" \
  "Multi-node membership, late join checkpoint serving, lease expiry, and reassignment" \
  "cargo test --manifest-path '$PSIONIC_MANIFEST' -p psionic-train --lib expired_same_node_receipt_rejoins_without_manual_metadata_edits -- --nocapture && cargo test --manifest-path '$PSIONIC_MANIFEST' -p psionic-train --lib failure_revision_can_rejoin_later -- --nocapture && cargo test -p nexus-control --lib training_scheduler_claims_leases_for_running_runs_and_replaces_expired_workers -- --nocapture"

run_step \
  "crash_and_restart" \
  "Crash, drain, restart, and retained-state recovery across Psionic, Pylon, and Nexus" \
  "cargo test --manifest-path '$PSIONIC_MANIFEST' -p psionic-train --lib checkpoint_and_restart_faults_recover_cleanly -- --nocapture && cargo test -p pylon --lib training_supervisor_records_logs_heartbeat_and_failure_receipt_on_failed_exit -- --nocapture && cargo test -p pylon --lib draining_and_restarting_training_supervisor_rotates_attempt_logs_without_losing_history -- --nocapture && cargo test -p pylon --lib training_runtime_state_round_trips_across_restart -- --nocapture && cargo test -p nexus-control --lib training_scheduler_state_reloads_after_restart_and_reuses_trn_publications -- --nocapture"

run_step \
  "validator_paths" \
  "Validator accepted, replay-required, held-escalation, rejected-digest, and timeout paths" \
  "cargo test --manifest-path '$PSIONIC_MANIFEST' -p psionic-train --lib validator_accepts_replayed_contribution_and_scores_window -- --nocapture && cargo test --manifest-path '$PSIONIC_MANIFEST' -p psionic-train --lib validator_marks_missing_sampled_replay_as_replay_required -- --nocapture && cargo test -p nexus-control --lib training_window_reconcile_refuses_stale_manifest_replay_and_bad_artifact_digest -- --nocapture && cargo test -p nexus-control --lib training_window_validation_escalates_and_blocks_held_reconcile -- --nocapture && cargo test -p nexus-control --lib training_window_timeout_publishes_held_closeout_and_validator_poor_label -- --nocapture"

run_step \
  "publication_outage" \
  "TRN outage, queued retry, and replay-safe catch-up across restart" \
  "cargo test -p pylon --lib training_publish_queues_retry_state_when_relays_are_unavailable -- --nocapture && cargo test -p nexus-control --lib publish_training_trn_state_queues_retry_state_across_restarts_until_relays_recover -- --nocapture"

run_step \
  "operator_and_policy" \
  "Closeout and reputation ingestion, operator visibility, and environment refusal guardrails" \
  "cargo test --manifest-path '$PSIONIC_MANIFEST' -p psionic-train --lib curriculum_refuses_environment_mismatch -- --nocapture && cargo test -p pylon --lib training_admin_routes_serve_status_and_refresh_node_records -- --nocapture && cargo test -p pylon --lib training_sync_ingests_closeouts_and_reputation_and_blocks_readvertisement -- --nocapture && cargo test -p pylon --lib training_status_report_surfaces_operator_state -- --nocapture && cargo test -p nexus-control --lib training_operator_summary_and_stats_surface_live_run_state -- --nocapture"

printf '## Result\n\n' >>"$SUMMARY_MD"
if ((failed == 0)); then
  printf 'Distributed-training MVP rehearsal matrix passed.\n' >>"$SUMMARY_MD"
  echo "Distributed-training MVP rehearsal matrix passed."
else
  printf 'Distributed-training MVP rehearsal matrix failed.\n' >>"$SUMMARY_MD"
  echo "Distributed-training MVP rehearsal matrix failed." >&2
  exit 1
fi
