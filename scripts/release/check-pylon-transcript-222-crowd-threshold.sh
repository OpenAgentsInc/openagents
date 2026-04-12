#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")-$$"
OUTPUT_DIR="${OPENAGENTS_TRANSCRIPT_222_CROWD_OUTPUT_DIR:-$ROOT_DIR/target/pylon-transcript-222-crowd-threshold/$TIMESTAMP}"
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
  scripts/release/check-pylon-transcript-222-crowd-threshold.sh [options]

Runs the retained Transcript 222 widened crowd rehearsal gate. This extends the
small-cohort canary over the >70 participant threshold while preserving the
same scheduler, payout, and public-state truth paths.

Options:
  --output-dir <path>         Write summary artifacts under this directory.
  --psionic-repo <path>       Override the sibling Psionic checkout.
  --psionic-manifest <path>   Override the Psionic Cargo.toml path.
  -h, --help                  Show this help text.

Environment:
  OPENAGENTS_TRANSCRIPT_222_CROWD_OUTPUT_DIR
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

printf '# Transcript 222 Crowd-Threshold Rehearsal Summary\n\n' >"$SUMMARY_MD"
printf -- '- timestamp_utc: `%s`\n' "$TIMESTAMP" >>"$SUMMARY_MD"
printf -- '- output_dir: `%s`\n' "$OUTPUT_DIR" >>"$SUMMARY_MD"
printf -- '- psionic_manifest: `%s`\n\n' "$PSIONIC_MANIFEST" >>"$SUMMARY_MD"
cat >>"$SUMMARY_MD" <<'EOF'
## Scope

This retained gate proves that the existing Transcript 222 canary expands past
the participant threshold without changing truth semantics:

- the retained canary gate still passes
- the public counters distinguish online, assigned, accepted, and
  model-progress contributors at scale
- the weak-device and strong-lane payout totals split correctly
- settlement-level closeouts still count accepted weak-device work even though
  the progress-only public closeout counter remains strong-lane only

## Hard Claim Coverage

| Gate | Covered by |
| --- | --- |
| small-cohort canary still green | `canary_gate` |
| >70 truthful assigned and accepted contributors | `crowd_threshold_projection` |
| weak-device and strong-lane payout totals stay split | `crowd_threshold_projection` |
| online versus assigned participant truth stays explicit | `crowd_threshold_projection` |
| settlement closeouts remain broader than progress closeouts | `crowd_threshold_projection` |

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
  "canary_gate" \
  "Transcript 222 small-cohort canary gate remains green before widening the participant threshold" \
  "OPENAGENTS_PSIONIC_REPO='$PSIONIC_REPO' OPENAGENTS_PSIONIC_MANIFEST='$PSIONIC_MANIFEST' '$ROOT_DIR/scripts/release/check-pylon-transcript-222-canary.sh' --output-dir '$OUTPUT_DIR/canary'"

run_step \
  "crowd_threshold_projection" \
  "Nexus projects truthful assigned, accepted, payout, and threshold-scale public state for a >70 participant rehearsal" \
  "cargo test -p nexus-control --manifest-path '$ROOT_DIR/Cargo.toml' --lib transcript_222_crowd_threshold_projects_public_truth_and_payouts -- --nocapture"

printf '## Result\n\n' >>"$SUMMARY_MD"
if ((failed == 0)); then
  printf 'Transcript 222 crowd-threshold rehearsal gate passed.\n' >>"$SUMMARY_MD"
  echo "Transcript 222 crowd-threshold rehearsal gate passed."
else
  printf 'Transcript 222 crowd-threshold rehearsal gate failed.\n' >>"$SUMMARY_MD"
  echo "Transcript 222 crowd-threshold rehearsal gate failed." >&2
  exit 1
fi
