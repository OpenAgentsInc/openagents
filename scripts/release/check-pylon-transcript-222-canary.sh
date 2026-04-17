#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")-$$"
OUTPUT_DIR="${OPENAGENTS_TRANSCRIPT_222_CANARY_OUTPUT_DIR:-$ROOT_DIR/target/pylon-transcript-222-canary/$TIMESTAMP}"
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
  scripts/release/check-pylon-transcript-222-canary.sh [options]

Runs the retained Transcript 222 small-cohort canary gate across the existing
Psionic machine-runtime lane plus the current Pylon and Nexus authority,
treasury, and public-stats surfaces.

Options:
  --output-dir <path>         Write summary artifacts under this directory.
  --psionic-repo <path>       Override the sibling Psionic checkout.
  --psionic-manifest <path>   Override the Psionic Cargo.toml path.
  -h, --help                  Show this help text.

Environment:
  OPENAGENTS_TRANSCRIPT_222_CANARY_OUTPUT_DIR
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

printf '# Transcript 222 Small-Cohort Canary Summary\n\n' >"$SUMMARY_MD"
printf -- '- timestamp_utc: `%s`\n' "$TIMESTAMP" >>"$SUMMARY_MD"
printf -- '- output_dir: `%s`\n' "$OUTPUT_DIR" >>"$SUMMARY_MD"
printf -- '- psionic_manifest: `%s`\n\n' "$PSIONIC_MANIFEST" >>"$SUMMARY_MD"
cat >>"$SUMMARY_MD" <<'EOF'
## Scope

This retained gate covers the hard-claim slices that must exist together before
Transcript 222 can claim a small-cohort canary:

- strong-node assigned work compiles into the admitted `psionic-train` lane
- weak-device validator replay emits the accepted weak-device proof bundle
- `Pylon` automatically claims, acknowledges, materializes, and launches one
  retained assignment
- `Pylon` automatically turns retained worker output into seal, validator
  finalize, reconcile, and payout observation
- `Nexus` closes one weak-device accepted-work payout and one strong-lane
  accepted-work payout
- `/api/stats`, `/api/training/summary`, and `/api/homepage` project the same
  training/public truth from the current authority state

## Hard Claim Coverage

| Gate | Covered by |
| --- | --- |
| Strong-node accepted lane | `psionic_actual_lane`, `pylon_assignment_intake`, `pylon_runtime_launch`, `pylon_autonomous_closeout`, `nexus_strong_lane_payout` |
| Weak-device accepted lane | `psionic_weak_device_proof`, `nexus_weak_lane_payout` |
| Zero-touch assignment intake | `pylon_assignment_intake` |
| Zero-touch artifact fetch and runtime launch | `pylon_assignment_intake`, `pylon_runtime_launch` |
| Automatic post-worker closeout | `pylon_autonomous_closeout` |
| Public stats truth | `nexus_public_stats_projection`, `nexus_homepage_projection` |
| Payout-linked accepted closeout | `pylon_autonomous_closeout`, `nexus_weak_lane_payout`, `nexus_strong_lane_payout` |

EOF

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
  "psionic_actual_lane" \
  "Psionic strong-node automatic execution request compiles the admitted actual-pretraining manifest and outputs" \
  "cargo test --manifest-path '$PSIONIC_MANIFEST' -p psionic-train --lib actual_pretraining_automatic_execution_request_builds_start_manifest_and_outputs -- --nocapture"

run_step \
  "psionic_weak_device_proof" \
  "Psionic weak-device validator replay emits the accepted weak-device proof bundle" \
  "cargo test --manifest-path '$PSIONIC_MANIFEST' -p psionic-train --lib apple_validation_replay_emits_weak_device_proof -- --nocapture"

run_step \
  "pylon_assignment_intake" \
  "Pylon automatically admits, claims, acknowledges, and materializes a retained assignment with resolver-backed artifacts" \
  "cargo test -p pylon --manifest-path '$ROOT_DIR/Cargo.toml' --lib training_assignment_intake_claims_and_acks_assignment_and_updates_status -- --nocapture"

run_step \
  "pylon_runtime_launch" \
  "Pylon auto-launch starts the retained runtime from the leased assignment and preserves machine status packets" \
  "cargo test -p pylon --manifest-path '$ROOT_DIR/Cargo.toml' --lib auto_launch_starts_supervisor_from_retained_assignment_and_preserves_packets -- --nocapture"

run_step \
  "pylon_autonomous_closeout" \
  "Pylon uses retained worker and validator state to seal, finalize, reconcile, and observe payout without manual closeout calls" \
  "cargo test -p pylon --manifest-path '$ROOT_DIR/Cargo.toml' --lib pylon_autonomously_closes_homework_assignment_from_worker_completion_to_paid_receipt -- --nocapture"

run_step \
  "nexus_public_stats_projection" \
  "Nexus operator summary and public stats project live run state, assigned contributors, validation pressure, and queue health" \
  "cargo test -p nexus-control --manifest-path '$ROOT_DIR/Cargo.toml' --lib training_operator_summary_and_stats_surface_live_run_state -- --nocapture"

run_step \
  "nexus_weak_lane_payout" \
  "Nexus weak-device validation replay closeout queues and dispatches accepted-work payout with public stats projection" \
  "cargo test -p nexus-control --manifest-path '$ROOT_DIR/Cargo.toml' --lib validation_replay_closeout_queues_and_dispatches_weak_device_payout -- --nocapture"

run_step \
  "nexus_strong_lane_payout" \
  "Nexus strong full-island closeout queues and dispatches accepted-work payout with public stats projection" \
  "cargo test -p nexus-control --manifest-path '$ROOT_DIR/Cargo.toml' --lib full_island_closeout_queues_and_dispatches_strong_lane_payout -- --nocapture"

run_step \
  "nexus_homepage_projection" \
  "Homepage snapshot aggregates public stats training state and recent TRN publications from the same authority truth" \
  "cargo test -p nexus-control --manifest-path '$ROOT_DIR/Cargo.toml' --lib homepage_snapshot_aggregates_public_stats_training_state_and_recent_trn -- --nocapture"

printf '## Result\n\n' >>"$SUMMARY_MD"
if ((failed == 0)); then
  printf 'Transcript 222 small-cohort canary gate passed.\n' >>"$SUMMARY_MD"
  echo "Transcript 222 small-cohort canary gate passed."
else
  printf 'Transcript 222 small-cohort canary gate failed.\n' >>"$SUMMARY_MD"
  echo "Transcript 222 small-cohort canary gate failed." >&2
  exit 1
fi
