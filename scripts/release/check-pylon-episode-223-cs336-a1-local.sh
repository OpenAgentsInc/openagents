#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")-$$"
OUTPUT_DIR="${OPENAGENTS_EP223_CS336_A1_LOCAL_OUTPUT_DIR:-$ROOT_DIR/target/pylon-episode-223-cs336-a1-local/$TIMESTAMP}"
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
  scripts/release/check-pylon-episode-223-cs336-a1-local.sh [options]

Runs the retained local Episode 223 CS336 A1 dry run across the packaged
Psionic lane plus the current local Nexus and Pylon authority/test surfaces.

Options:
  --output-dir <path>         Write summary artifacts under this directory.
  --psionic-repo <path>       Override the Psionic checkout used for cargo test.
  --psionic-manifest <path>   Override the Psionic Cargo.toml path.
  -h, --help                  Show this help text.

Environment:
  OPENAGENTS_EP223_CS336_A1_LOCAL_OUTPUT_DIR
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

printf '# Episode 223 Local CS336 A1 Dry Run Summary\n\n' >"$SUMMARY_MD"
printf -- '- timestamp_utc: `%s`\n' "$TIMESTAMP" >>"$SUMMARY_MD"
printf -- '- output_dir: `%s`\n' "$OUTPUT_DIR" >>"$SUMMARY_MD"
printf -- '- psionic_manifest: `%s`\n\n' "$PSIONIC_MANIFEST" >>"$SUMMARY_MD"
cat >>"$SUMMARY_MD" <<'EOF'
## Scope

This retained gate is the local honest rehearsal for Episode 223 before a live
fleet run:

- the packaged `psionic-train` CS336 A1 lane writes retained checkpoint and
  closeout outputs through the machine-manifest path
- `Nexus` can schedule and surface one named `CS336 A1 Demo` run with two
  worker slots
- `Pylon` maps that run into the packaged demo lane, claims it, launches it,
  and syncs terminal artifacts back to `Nexus`
- weak Apple and consumer-CUDA hosts both promote into the bounded A1 trainer
  lane instead of staying stranded below trainer tier

This is a local dry run. It is not a claim that the live public fleet already
did the run.

## Hard Claim Coverage

| Gate | Covered by |
| --- | --- |
| Packaged CS336 lane executes through machine manifest | `psionic_machine_manifest` |
| Weak Mac host can do the homework | `mac_cs336_fallback` |
| Linux consumer CUDA host can do the homework | `linux_cs336_fallback` |
| Nexus can schedule and name the dual-host run | `nexus_named_run` |
| Pylon maps the environment into the packaged lane | `pylon_manifest_mapping` |
| Pylon claims and acknowledges the assignment | `pylon_assignment_intake` |
| Pylon launches the retained runtime | `pylon_runtime_launch` |
| Pylon syncs terminal artifacts back to Nexus | `pylon_terminal_sync` |

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
  "psionic_machine_manifest" \
  "Psionic packaged CS336 A1 machine-manifest path writes retained checkpoint and closeout outputs" \
  "cargo test --manifest-path '$PSIONIC_MANIFEST' -p psionic-train --test psionic_train_cli machine_manifest_cs336_a1_demo_writes_checkpoint_and_closeout -- --nocapture"

run_step \
  "mac_cs336_fallback" \
  "Pylon upgrades a weak Apple host into the bounded CS336 A1 trainer lane instead of leaving it below trainer tier" \
  "cargo test -p pylon --manifest-path '$ROOT_DIR/Cargo.toml' --lib adapter_training_detection_routes_subfloor_apple_hosts_into_cs336_a1_demo_lane -- --nocapture && cargo test -p pylon --manifest-path '$ROOT_DIR/Cargo.toml' --lib training_capability_tier_marks_subfloor_apple_cs336_worker_as_tier2_trainer -- --nocapture"

run_step \
  "linux_cs336_fallback" \
  "Pylon upgrades a consumer CUDA Linux host into the bounded CS336 A1 trainer lane instead of leaving it below trainer tier" \
  "cargo test -p pylon --manifest-path '$ROOT_DIR/Cargo.toml' --lib adapter_training_detection_routes_non_admitted_cuda_hosts_into_cs336_a1_demo_lane -- --nocapture && cargo test -p pylon --manifest-path '$ROOT_DIR/Cargo.toml' --lib training_capability_tier_marks_consumer_cuda_cs336_worker_as_tier2_trainer -- --nocapture"

run_step \
  "nexus_named_run" \
  "Nexus schedules one named dual-host CS336 A1 Demo run and projects the display name through the public summary surfaces" \
  "cargo test -p nexus-control --manifest-path '$ROOT_DIR/Cargo.toml' --lib training_scheduler_claims_two_slot_named_cs336_a1_demo_run_and_surfaces_display_name -- --nocapture"

run_step \
  "pylon_manifest_mapping" \
  "Pylon maps the CS336 A1 environment into the packaged psionic-train lane and small-model work class" \
  "cargo test -p pylon --manifest-path '$ROOT_DIR/Cargo.toml' --lib build_psionic_train_invocation_manifest_maps_cs336_a1_demo_environment_to_packaged_lane -- --nocapture"

run_step \
  "pylon_assignment_intake" \
  "Pylon automatically claims, acknowledges, and materializes one retained assignment" \
  "cargo test -p pylon --manifest-path '$ROOT_DIR/Cargo.toml' --lib training_assignment_intake_claims_and_acks_assignment_and_updates_status -- --nocapture"

run_step \
  "pylon_runtime_launch" \
  "Pylon launches the retained runtime from the leased assignment and preserves machine status packets" \
  "cargo test -p pylon --manifest-path '$ROOT_DIR/Cargo.toml' --lib auto_launch_starts_supervisor_from_retained_assignment_and_preserves_packets -- --nocapture"

run_step \
  "pylon_terminal_sync" \
  "Pylon uploads retained artifacts and reports success back to Nexus without duplicate publication" \
  "cargo test -p pylon --manifest-path '$ROOT_DIR/Cargo.toml' --lib training_terminal_sync_uploads_artifacts_and_reports_success_to_nexus -- --nocapture"

printf '## Result\n\n' >>"$SUMMARY_MD"
if ((failed == 0)); then
  printf 'Episode 223 local CS336 A1 dry run passed.\n' >>"$SUMMARY_MD"
  echo "Episode 223 local CS336 A1 dry run passed."
else
  printf 'Episode 223 local CS336 A1 dry run failed.\n' >>"$SUMMARY_MD"
  echo "Episode 223 local CS336 A1 dry run failed." >&2
  exit 1
fi
