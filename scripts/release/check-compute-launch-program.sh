#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")-$$"
OUTPUT_DIR="${OPENAGENTS_COMPUTE_VALIDATION_OUTPUT_DIR:-$ROOT_DIR/target/compute-launch-program/$TIMESTAMP}"
SOAK_ITERATIONS="${OPENAGENTS_COMPUTE_VALIDATION_SOAK_ITERATIONS:-0}"
INCLUDE_HEADLESS_LIVE="${OPENAGENTS_COMPUTE_VALIDATION_INCLUDE_HEADLESS_LIVE:-0}"
INCLUDE_PACKAGED_MACOS="${OPENAGENTS_COMPUTE_VALIDATION_INCLUDE_PACKAGED_MACOS:-0}"
INCLUDE_NVIDIA="${OPENAGENTS_COMPUTE_VALIDATION_INCLUDE_NVIDIA:-0}"
INCLUDE_CLUSTER_BENCH="${OPENAGENTS_COMPUTE_VALIDATION_INCLUDE_CLUSTER_BENCH:-0}"
MANIFEST_PATH="${OPENAGENTS_COMPUTE_VALIDATION_MANIFEST:-}"
AUTOPILOTCTL_BIN="${OPENAGENTS_COMPUTE_VALIDATION_AUTOPILOTCTL_BIN:-$ROOT_DIR/target/release/autopilotctl}"

usage() {
  cat <<'USAGE'
Usage:
  scripts/release/check-compute-launch-program.sh [options]

Runs the cross-stack compute launch program. The default gate is self-contained
and covers desktop-control, Psionic cluster/sandbox/evidence, validator
protocols, and kernel authority compute flows. Optional flags widen the run
into funded headless, packaged macOS, Linux NVIDIA, cluster-benchmark, and
live desktop-manifest capture legs.

Options:
  --output-dir <path>         Write artifacts under this directory.
  --soak-iterations <count>   Repeat the integrated soak trio this many times.
  --include-headless-live     Run funded headless smoke + roundtrip scripts.
  --include-packaged-macos    Run packaged macOS desktop + autopilotctl checks.
  --include-nvidia            Run Linux NVIDIA GPT-OSS Mission Control smoke.
  --include-cluster-bench     Run ignored Psionic cluster benchmark gates.
  --manifest <path>           Capture live desktop-control snapshots with autopilotctl.
  --autopilotctl-bin <path>   Override the autopilotctl binary used for manifest capture.
  -h, --help                  Show this help text.

Environment:
  OPENAGENTS_COMPUTE_VALIDATION_OUTPUT_DIR
  OPENAGENTS_COMPUTE_VALIDATION_SOAK_ITERATIONS
  OPENAGENTS_COMPUTE_VALIDATION_INCLUDE_HEADLESS_LIVE=1
  OPENAGENTS_COMPUTE_VALIDATION_INCLUDE_PACKAGED_MACOS=1
  OPENAGENTS_COMPUTE_VALIDATION_INCLUDE_NVIDIA=1
  OPENAGENTS_COMPUTE_VALIDATION_INCLUDE_CLUSTER_BENCH=1
  OPENAGENTS_COMPUTE_VALIDATION_MANIFEST=/path/to/desktop-control.json
  OPENAGENTS_COMPUTE_VALIDATION_AUTOPILOTCTL_BIN=/path/to/autopilotctl
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --output-dir)
      OUTPUT_DIR="$2"
      shift 2
      ;;
    --soak-iterations)
      SOAK_ITERATIONS="$2"
      shift 2
      ;;
    --include-headless-live)
      INCLUDE_HEADLESS_LIVE=1
      shift
      ;;
    --include-packaged-macos)
      INCLUDE_PACKAGED_MACOS=1
      shift
      ;;
    --include-nvidia)
      INCLUDE_NVIDIA=1
      shift
      ;;
    --include-cluster-bench)
      INCLUDE_CLUSTER_BENCH=1
      shift
      ;;
    --manifest)
      MANIFEST_PATH="$2"
      shift 2
      ;;
    --autopilotctl-bin)
      AUTOPILOTCTL_BIN="$2"
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
if ! command -v python3 >/dev/null 2>&1; then
  echo "missing required command: python3" >&2
  exit 2
fi
if ! [[ "$SOAK_ITERATIONS" =~ ^[0-9]+$ ]]; then
  echo "invalid soak iteration count: $SOAK_ITERATIONS" >&2
  exit 2
fi

mkdir -p "$OUTPUT_DIR"

SUMMARY_MD="$OUTPUT_DIR/SUMMARY.md"
SUMMARY_JSON="$OUTPUT_DIR/summary.json"
STEPS_TSV="$OUTPUT_DIR/steps.tsv"
: >"$STEPS_TSV"

printf '# Compute Launch Program Summary\n\n' >"$SUMMARY_MD"
printf -- '- timestamp_utc: `%s`\n' "$TIMESTAMP" >>"$SUMMARY_MD"
printf -- '- output_dir: `%s`\n' "$OUTPUT_DIR" >>"$SUMMARY_MD"
printf -- '- soak_iterations: `%s`\n' "$SOAK_ITERATIONS" >>"$SUMMARY_MD"
printf -- '- include_headless_live: `%s`\n' "$INCLUDE_HEADLESS_LIVE" >>"$SUMMARY_MD"
printf -- '- include_packaged_macos: `%s`\n' "$INCLUDE_PACKAGED_MACOS" >>"$SUMMARY_MD"
printf -- '- include_nvidia: `%s`\n' "$INCLUDE_NVIDIA" >>"$SUMMARY_MD"
printf -- '- include_cluster_bench: `%s`\n' "$INCLUDE_CLUSTER_BENCH" >>"$SUMMARY_MD"
printf -- '- manifest_capture: `%s`\n\n' "${MANIFEST_PATH:-disabled}" >>"$SUMMARY_MD"

declare -i failed=0

record_step() {
  local id="$1"
  local name="$2"
  local status="$3"
  local elapsed="$4"
  local required="$5"
  local log_path="$6"
  printf '%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$id" "$name" "$status" "$elapsed" "$required" "$log_path" >>"$STEPS_TSV"
}

run_step() {
  local id="$1"
  local name="$2"
  local required="$3"
  local command="$4"
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

  record_step "$id" "$name" "$status" "$elapsed" "$required" "$log_path"

  printf -- '- `%s` %s (%ss)\n' "$id" "$status" "$elapsed" >>"$SUMMARY_MD"
  printf -- '  - %s\n' "$name" >>"$SUMMARY_MD"
  printf -- '  - required: `%s`\n' "$required" >>"$SUMMARY_MD"
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

skip_step() {
  local id="$1"
  local name="$2"
  local required="$3"
  local reason="$4"

  record_step "$id" "$name" "skipped" "0" "$required" "$reason"

  printf -- '- `%s` skipped\n' "$id" >>"$SUMMARY_MD"
  printf -- '  - %s\n' "$name" >>"$SUMMARY_MD"
  printf -- '  - required: `%s`\n' "$required" >>"$SUMMARY_MD"
  printf -- '  - reason: `%s`\n\n' "$reason" >>"$SUMMARY_MD"

  echo "==> [$id] $name"
  echo "    -> SKIPPED ($reason)"
}

run_step \
  "desktop_control_and_mcp" \
  "Desktop-control, autopilotctl, and MCP proof/challenge/operator truth" \
  "true" \
  "cargo test -p autopilot-desktop desktop_control::tests::buy_mode_request_status_preserves_result_invoice_and_payable_roles -- --exact --nocapture && cargo test -p autopilot-desktop desktop_control::tests::proof_history_surfaces_settlement_and_identity_review_fields -- --exact --nocapture && cargo test -p autopilot-desktop desktop_control::tests::settlement_and_challenge_history_stay_linked_to_same_delivery -- --exact --nocapture && cargo test -p autopilot-desktop desktop_control::tests::snapshot_change_events_emit_local_runtime_and_gpt_oss_domains -- --exact --nocapture && cargo test -p autopilot-desktop desktop_control::tests::snapshot_signature_changes_when_sandbox_truth_changes -- --exact --nocapture && cargo test -p autopilot-desktop compute_mcp::tests::server_maps_representative_tools_to_desktop_actions -- --exact --nocapture && cargo test -p autopilot-desktop --bin autopilotctl lifecycle_commands_map_to_control_requests -- --exact --nocapture"

run_step \
  "headless_compute_units" \
  "Headless buyer/provider flow, payment coupling, and result publication semantics" \
  "true" \
  "cargo test -p autopilot-desktop headless_compute::tests::buyer_request_success_requires_result_payment_and_provider_success_feedback -- --exact --nocapture && cargo test -p autopilot-desktop headless_compute::tests::buyer_payment_dispatch_uses_same_provider_for_result_and_invoice -- --exact --nocapture && cargo test -p autopilot-desktop headless_compute::tests::provider_invoice_waits_for_result_publish_confirmation -- --exact --nocapture"

run_step \
  "psionic_sandbox_jobs" \
  "Sandbox execution, receipts, background jobs, uploads, waits, and artifacts" \
  "true" \
  "cargo test -p psionic-sandbox execution::tests::local_subprocess_success_emits_receipt_and_artifacts -- --exact --nocapture && cargo test -p psionic-sandbox execution::tests::policy_rejection_is_receipted -- --exact --nocapture && cargo test -p psionic-sandbox jobs::tests::background_job_lifecycle_supports_upload_poll_wait_and_artifact_download -- --exact --nocapture"

run_step \
  "psionic_cluster_matrix" \
  "Cluster transport, discovery, admission, sharding, recovery, and fault matrix" \
  "true" \
  "cargo test -p psionic-cluster --test local_cluster_transport -- --nocapture && cargo test -p psionic-cluster --test cluster_validation_matrix -- --nocapture"

run_step \
  "psionic_evidence_and_receipts" \
  "Cluster evidence bundles, delivered execution context, and provider receipts" \
  "true" \
  "cargo test -p psionic-runtime tests::delivered_execution_context_can_carry_cluster_evidence -- --exact --nocapture && cargo test -p psionic-runtime tests::signed_cluster_evidence_bundle_round_trips_and_verifies -- --exact --nocapture && cargo test -p psionic-provider tests::capability_envelope_can_surface_cluster_execution_context -- --exact --nocapture && cargo test -p psionic-provider tests::text_generation_receipt_surfaces_pipeline_sharded_cluster_execution_truth -- --exact --nocapture && cargo test -p psionic-provider tests::text_generation_receipt_surfaces_layer_sharded_cluster_execution_truth -- --exact --nocapture && cargo test -p psionic-provider tests::text_generation_receipt_surfaces_tensor_sharded_cluster_execution_truth -- --exact --nocapture && cargo test -p psionic-provider tests::sandbox_execution_receipt_can_surface_accelerator_deliverability -- --exact --nocapture"

run_step \
  "validator_service" \
  "Validator queueing, lease expiry, verified verdicts, and rejected verdicts" \
  "true" \
  "cargo test -p openagents-validator-service -- --nocapture"

run_step \
  "nexus_compute_authority" \
  "Kernel receipts, environments, evals, synthetic data, challenges, and index methodology" \
  "true" \
  "cargo test -p nexus-control tests::compute_market_flow_persists_authoritative_objects_and_metrics -- --exact --nocapture && cargo test -p nexus-control kernel::tests::compute_evaluation_run_lifecycle_finalizes_summary_and_links_delivery -- --exact --nocapture && cargo test -p nexus-control kernel::tests::compute_synthetic_data_pipeline_links_generation_and_verification -- --exact --nocapture && cargo test -p nexus-control tests::validator_challenge_routes_schedule_lease_finalize_and_list -- --exact --nocapture && cargo test -p nexus-control kernel::tests::compute_index_methodology_matrix_covers_major_compute_lanes -- --exact --nocapture && cargo test -p nexus-control kernel::tests::clustered_compute_index_correction_tracks_challenge_quality -- --exact --nocapture"

if [[ -n "$MANIFEST_PATH" ]]; then
  run_step \
    "desktop_manifest_capture" \
    "Capture live desktop-control snapshots for status, runtime, cluster, proof, challenge, and sandbox truth" \
    "false" \
    "mkdir -p '$OUTPUT_DIR/desktop-manifest' && '$AUTOPILOTCTL_BIN' --manifest '$MANIFEST_PATH' --json status >'$OUTPUT_DIR/desktop-manifest/status.json' && '$AUTOPILOTCTL_BIN' --manifest '$MANIFEST_PATH' --json local-runtime status >'$OUTPUT_DIR/desktop-manifest/local-runtime.json' && '$AUTOPILOTCTL_BIN' --manifest '$MANIFEST_PATH' --json cluster status >'$OUTPUT_DIR/desktop-manifest/cluster.json' && '$AUTOPILOTCTL_BIN' --manifest '$MANIFEST_PATH' --json proof status >'$OUTPUT_DIR/desktop-manifest/proof.json' && '$AUTOPILOTCTL_BIN' --manifest '$MANIFEST_PATH' --json challenge status >'$OUTPUT_DIR/desktop-manifest/challenge.json' && '$AUTOPILOTCTL_BIN' --manifest '$MANIFEST_PATH' --json sandbox status >'$OUTPUT_DIR/desktop-manifest/sandbox.json' && '$AUTOPILOTCTL_BIN' --manifest '$MANIFEST_PATH' logs --tail 200 >'$OUTPUT_DIR/desktop-manifest/logs-tail.txt'"
else
  skip_step \
    "desktop_manifest_capture" \
    "Capture live desktop-control snapshots for status, runtime, cluster, proof, challenge, and sandbox truth" \
    "false" \
    "no --manifest path supplied"
fi

if [[ "$INCLUDE_CLUSTER_BENCH" == "1" ]]; then
  run_step \
    "psionic_cluster_benchmarks" \
    "Ignored cluster benchmark gates with JSON benchmark receipts" \
    "false" \
    "mkdir -p '$OUTPUT_DIR/cluster-bench' && PSIONIC_CLUSTER_BENCH_JSON_OUT='$OUTPUT_DIR/cluster-bench' cargo test -p psionic-cluster --test cluster_benchmark_gates -- --ignored --nocapture"
else
  skip_step \
    "psionic_cluster_benchmarks" \
    "Ignored cluster benchmark gates with JSON benchmark receipts" \
    "false" \
    "enable with --include-cluster-bench"
fi

if [[ "$INCLUDE_HEADLESS_LIVE" == "1" ]]; then
  run_step \
    "headless_live_roundtrip" \
    "Funded headless provider and buyer smoke plus reciprocal roundtrip" \
    "false" \
    "OPENAGENTS_HEADLESS_RUN_DIR='$OUTPUT_DIR/headless-smoke' scripts/autopilot/headless-compute-smoke.sh && OPENAGENTS_HEADLESS_RUN_DIR='$OUTPUT_DIR/headless-roundtrip' scripts/autopilot/headless-compute-roundtrip.sh"
else
  skip_step \
    "headless_live_roundtrip" \
    "Funded headless provider and buyer smoke plus reciprocal roundtrip" \
    "false" \
    "enable with --include-headless-live"
fi

if [[ "$INCLUDE_PACKAGED_MACOS" == "1" ]]; then
  run_step \
    "packaged_macos" \
    "Packaged macOS app, autopilotctl roundtrip, and packaged compute smoke" \
    "false" \
    "OPENAGENTS_PACKAGED_RUN_DIR='$OUTPUT_DIR/packaged-compute' scripts/release/check-v01-packaged-compute.sh && OPENAGENTS_AUTOPILOTCTL_RUN_DIR='$OUTPUT_DIR/packaged-autopilotctl' scripts/release/check-v01-packaged-autopilotctl-roundtrip.sh"
else
  skip_step \
    "packaged_macos" \
    "Packaged macOS app, autopilotctl roundtrip, and packaged compute smoke" \
    "false" \
    "enable with --include-packaged-macos"
fi

if [[ "$INCLUDE_NVIDIA" == "1" ]]; then
  run_step \
    "linux_nvidia_mission_control" \
    "Linux NVIDIA GPT-OSS Mission Control smoke with desktop-control capture" \
    "false" \
    "OPENAGENTS_GPT_OSS_NVIDIA_RUN_DIR='$OUTPUT_DIR/gpt-oss-nvidia' scripts/release/check-gpt-oss-nvidia-mission-control.sh"
else
  skip_step \
    "linux_nvidia_mission_control" \
    "Linux NVIDIA GPT-OSS Mission Control smoke with desktop-control capture" \
    "false" \
    "enable with --include-nvidia"
fi

if (( SOAK_ITERATIONS > 0 )); then
  for iteration in $(seq 1 "$SOAK_ITERATIONS"); do
    run_step \
      "soak_desktop_control_${iteration}" \
      "Soak iteration ${iteration}: desktop-control proof, challenge, and settlement projection" \
      "false" \
      "cargo test -p autopilot-desktop desktop_control::tests::proof_history_surfaces_settlement_and_identity_review_fields -- --exact --nocapture && cargo test -p autopilot-desktop desktop_control::tests::settlement_and_challenge_history_stay_linked_to_same_delivery -- --exact --nocapture"
    run_step \
      "soak_cluster_matrix_${iteration}" \
      "Soak iteration ${iteration}: cluster discovery, failover, and recovery matrix" \
      "false" \
      "cargo test -p psionic-cluster --test cluster_validation_matrix discovery_validation_covers_intake_refusal_expiry_and_reconciliation -- --exact --nocapture && cargo test -p psionic-cluster --test cluster_validation_matrix coordinator_authority_validation_surfaces_stale_leader_and_failover_fence_rotation -- --exact --nocapture"
    run_step \
      "soak_kernel_validator_${iteration}" \
      "Soak iteration ${iteration}: kernel challenge routing and settlement projection" \
      "false" \
      "cargo test -p nexus-control tests::validator_challenge_routes_schedule_lease_finalize_and_list -- --exact --nocapture"
  done
fi

python3 - "$STEPS_TSV" "$SUMMARY_JSON" "$TIMESTAMP" "$OUTPUT_DIR" "$SOAK_ITERATIONS" \
  "$INCLUDE_HEADLESS_LIVE" "$INCLUDE_PACKAGED_MACOS" "$INCLUDE_NVIDIA" "$INCLUDE_CLUSTER_BENCH" \
  "$MANIFEST_PATH" <<'PY'
import csv
import json
import pathlib
import sys

steps_path = pathlib.Path(sys.argv[1])
summary_path = pathlib.Path(sys.argv[2])
timestamp = sys.argv[3]
output_dir = sys.argv[4]
soak_iterations = int(sys.argv[5])
include_headless_live = sys.argv[6] == "1"
include_packaged_macos = sys.argv[7] == "1"
include_nvidia = sys.argv[8] == "1"
include_cluster_bench = sys.argv[9] == "1"
manifest_path = sys.argv[10]

steps = []
with steps_path.open() as handle:
    reader = csv.reader(handle, delimiter="\t")
    for row in reader:
        if not row:
            continue
        step_id, name, status, elapsed, required, log_or_reason = row
        steps.append(
            {
                "id": step_id,
                "name": name,
                "status": status,
                "elapsed_seconds": int(elapsed),
                "required": required == "true",
                "artifact": log_or_reason,
            }
        )

result = "pass"
for step in steps:
    if step["status"] == "fail":
        result = "fail"
        break

payload = {
    "timestamp_utc": timestamp,
    "output_dir": output_dir,
    "soak_iterations": soak_iterations,
    "include_headless_live": include_headless_live,
    "include_packaged_macos": include_packaged_macos,
    "include_nvidia": include_nvidia,
    "include_cluster_bench": include_cluster_bench,
    "manifest_capture": manifest_path or None,
    "result": result,
    "steps": steps,
}
summary_path.write_text(json.dumps(payload, indent=2) + "\n")
PY

if (( failed != 0 )); then
  printf 'Result: FAIL\n' >>"$SUMMARY_MD"
  echo "Compute launch program failed. Summary: $SUMMARY_MD"
  exit 1
fi

printf 'Result: PASS\n' >>"$SUMMARY_MD"
echo "Compute launch program passed. Summary: $SUMMARY_MD"
