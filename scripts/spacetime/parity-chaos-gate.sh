#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")"
OUTPUT_DIR="${OUTPUT_DIR:-$ROOT_DIR/output/spacetime/parity-chaos/$TIMESTAMP}"
mkdir -p "$OUTPUT_DIR"

usage() {
  cat <<'USAGE'
Usage:
  scripts/spacetime/parity-chaos-gate.sh [--output-dir <path>]

Runs deterministic Spacetime parity + chaos harness checks and writes per-gate artifacts.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --output-dir)
      OUTPUT_DIR="$2"
      mkdir -p "$OUTPUT_DIR"
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

summary_md="$OUTPUT_DIR/SUMMARY.md"
summary_json="$OUTPUT_DIR/summary.json"

printf '# Spacetime Parity + Chaos Gate Summary\n\n' >"$summary_md"
printf -- '{\n  "timestamp_utc": "%s",\n  "gates": [\n' "$TIMESTAMP" >"$summary_json"

first_json_row=true

declare -i failed=0

gate() {
  local id="$1"
  local name="$2"
  local command="$3"
  local log="$OUTPUT_DIR/${id}.log"
  local started ended elapsed status

  started="$(date +%s)"
  echo "==> [$id] $name"
  echo "    command: $command"

  if (cd "$ROOT_DIR" && bash -lc "$command") >"$log" 2>&1; then
    status="pass"
  else
    status="fail"
    failed=1
  fi

  ended="$(date +%s)"
  elapsed=$((ended - started))

  printf -- '- `%s` %s (%ss)\n' "$id" "$status" "$elapsed" >>"$summary_md"
  printf -- '  - %s\n' "$name" >>"$summary_md"
  printf -- '  - log: `%s`\n' "$log" >>"$summary_md"

  if [[ "$status" == "fail" ]]; then
    printf -- '  - tail:\n\n```text\n' >>"$summary_md"
    tail -n 40 "$log" >>"$summary_md" || true
    printf '```\n' >>"$summary_md"
  fi

  if [[ "$first_json_row" == true ]]; then
    first_json_row=false
  else
    printf ',\n' >>"$summary_json"
  fi
  printf -- '    {"id":"%s","name":"%s","status":"%s","elapsed_seconds":%s,"log":"%s"}' \
    "$id" "$name" "$status" "$elapsed" "$log" >>"$summary_json"

  if [[ "$status" == "fail" ]]; then
    echo "    -> FAIL ($log)"
  else
    echo "    -> PASS ($log)"
  fi
}

gate "parity_replay_resume" \
  "Replay/resume parity across shared stream consumers" \
  "cargo test -p autopilot-spacetime multi_client_subscribe_preserves_ordering_for_shared_stream -- --nocapture"

gate "stale_cursor_recovery" \
  "Stale-cursor recovery and rebootstrap signaling" \
  "cargo test -p autopilot-spacetime subscribe_rejects_stale_cursor -- --nocapture && cargo test -p autopilot-desktop sync_apply::tests::out_of_order_seq_requests_rebootstrap -- --nocapture"

gate "duplicate_delivery" \
  "Duplicate delivery/idempotent apply handling" \
  "cargo test -p autopilot-desktop sync_apply::tests::apply_seq_accepts_in_order_and_drops_duplicates -- --nocapture"

gate "reconnect_backoff_churn" \
  "Reconnect storm/backoff churn remains deterministic" \
  "cargo test -p autopilot-spacetime reconnect_storm_resubscribe_keeps_duplicate_delivery_deterministic -- --nocapture && cargo test -p autopilot-desktop sync_lifecycle::tests::reconnect_backoff_grows_and_caps_across_disconnects -- --nocapture"

printf '\n  ]\n}\n' >>"$summary_json"

if (( failed != 0 )); then
  printf '\nResult: FAIL\n' >>"$summary_md"
  echo "Spacetime parity/chaos gate failed. See $summary_md"
  exit 1
fi

printf '\nResult: PASS\n' >>"$summary_md"
echo "Spacetime parity/chaos gate passed. Summary: $summary_md"
