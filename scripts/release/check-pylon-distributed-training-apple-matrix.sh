#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")-$$"
OUTPUT_DIR="${OPENAGENTS_PYLON_APPLE_TRAINING_REHEARSAL_OUTPUT_DIR:-$ROOT_DIR/target/pylon-distributed-training-apple-rehearsal/$TIMESTAMP}"
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
  scripts/release/check-pylon-distributed-training-apple-matrix.sh [options]

Runs the retained Phase 6 Apple Silicon and Metal rehearsal matrix across the
standalone Psionic runtime seam plus the Pylon and Nexus control-plane seam.

Options:
  --output-dir <path>         Write summary artifacts under this directory.
  --psionic-repo <path>       Override the sibling Psionic checkout.
  --psionic-manifest <path>   Override the Psionic Cargo.toml path.
  -h, --help                  Show this help text.

Environment:
  OPENAGENTS_PYLON_APPLE_TRAINING_REHEARSAL_OUTPUT_DIR
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

printf '# Pylon Distributed Training Apple Rehearsal Summary\n\n' >"$SUMMARY_MD"
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
  "apple_single_node" \
  "Apple single-node dry run: admitted Apple lane, Apple-capable Pylon projection, and retained Apple node publication" \
  "cargo test --manifest-path '$PSIONIC_MANIFEST' -p psionic-train --test psionic_train_cli apple_manifest_start_emits_metal_capability_projection -- --nocapture && cargo test -p pylon --lib adapter_training_detection_marks_apple_hosts_ready_when_runtime_and_host_posture_match -- --nocapture && cargo test -p pylon --lib training_trn_mapping_preserves_apple_backend_capabilities_in_node_records -- --nocapture"

run_step \
  "apple_multi_node" \
  "Apple multi-node rehearsal: Apple lane admission plus backend-homogeneous worker scheduling beside CUDA" \
  "cargo test --manifest-path '$PSIONIC_MANIFEST' -p psionic-train --lib apple_lane_is_admitted_by_machine_contract -- --nocapture && cargo test -p nexus-control --lib training_scheduler_matches_worker_leases_to_backend_homogeneous_runs -- --nocapture"

run_step \
  "apple_validator_accepted" \
  "Apple validator accepted case: Apple replay receipt plus accepted-outcome gating under the shared authority contract" \
  "cargo test --manifest-path '$PSIONIC_MANIFEST' -p psionic-train --test psionic_train_cli apple_validator_manifest_emits_accepted_score_receipt_for_valid_contribution -- --nocapture && cargo test -p nexus-control --lib apple_training_outcomes_require_eval_and_runtime_validation_before_acceptance -- --nocapture"

run_step \
  "apple_checkpoint_rejoin" \
  "Apple checkpoint restore and rejoin drill: checkpoint emission, handoff serving, resume from peer checkpoint, and retained Pylon checkpoint serving" \
  "cargo test --manifest-path '$PSIONIC_MANIFEST' -p psionic-train --test psionic_train_cli apple_manifest_record_checkpoint_persists_generic_checkpoint_artifacts -- --nocapture && cargo test --manifest-path '$PSIONIC_MANIFEST' -p psionic-train --test psionic_train_cli apple_manifest_serve_checkpoint_retains_primary_handoff_receipt -- --nocapture && cargo test --manifest-path '$PSIONIC_MANIFEST' -p psionic-train --test psionic_train_cli apple_manifest_resume_can_seed_from_peer_checkpoint_handoff -- --nocapture && cargo test --manifest-path '$PSIONIC_MANIFEST' -p psionic-train --test psionic_train_cli apple_manifest_resume_refuses_without_any_admitted_checkpoint -- --nocapture && cargo test -p pylon --lib training_checkpoint_server_serves_local_checkpoint_paths -- --nocapture"

run_step \
  "dual_backend_claim_gate" \
  "Dual-backend claim gate: validator family matching plus explicit Apple versus CUDA publication across shared TRN shapes" \
  "cargo test -p nexus-control --lib training_validator_claims_skip_windows_from_mismatched_backend_families -- --nocapture && cargo test -p nexus-control --lib training_trn_mapping -- --nocapture && cargo test -p pylon --lib training_trn_mapping -- --nocapture"

printf '## Result\n\n' >>"$SUMMARY_MD"
if ((failed == 0)); then
  printf 'Distributed-training Apple rehearsal matrix passed.\n' >>"$SUMMARY_MD"
  echo "Distributed-training Apple rehearsal matrix passed."
else
  printf 'Distributed-training Apple rehearsal matrix failed.\n' >>"$SUMMARY_MD"
  echo "Distributed-training Apple rehearsal matrix failed." >&2
  exit 1
fi
