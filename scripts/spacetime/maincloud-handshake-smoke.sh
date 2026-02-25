#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

DB="${OA_SPACETIME_DEV_DATABASE:-}"
SERVER="${OA_SPACETIME_DEV_SERVER:-maincloud}"
TIMEOUT_SECONDS=20
SLEEP_SECONDS=4
OUTPUT_DIR=""

usage() {
  cat <<'EOF'
Usage:
  scripts/spacetime/maincloud-handshake-smoke.sh --db <database> [--server <name>] [--timeout <seconds>] [--sleep <seconds>] [--output-dir <dir>]

Environment:
  OA_SPACETIME_DEV_DATABASE  default database id/name if --db omitted
  OA_SPACETIME_DEV_SERVER    default server (default: maincloud)
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --db)
      DB="$2"
      shift 2
      ;;
    --server)
      SERVER="$2"
      shift 2
      ;;
    --timeout)
      TIMEOUT_SECONDS="$2"
      shift 2
      ;;
    --sleep)
      SLEEP_SECONDS="$2"
      shift 2
      ;;
    --output-dir)
      OUTPUT_DIR="$2"
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

if [[ -z "$DB" ]]; then
  echo "missing database id/name. pass --db or set OA_SPACETIME_DEV_DATABASE." >&2
  exit 2
fi

for cmd in spacetime rg; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "missing required command: $cmd" >&2
    exit 2
  fi
done

timestamp="$(date -u +"%Y%m%dT%H%M%SZ")"
if [[ -z "$OUTPUT_DIR" ]]; then
  OUTPUT_DIR="$ROOT_DIR/output/spacetime/handshake/$timestamp"
fi
mkdir -p "$OUTPUT_DIR"

query_count_raw() {
  spacetime sql "$DB" "SELECT COUNT(*) AS connected_clients FROM active_connection" --server "$SERVER"
}

extract_count() {
  local raw="$1"
  local parsed
  parsed="$(printf '%s\n' "$raw" | rg -o '[0-9]+' | tail -n 1 || true)"
  if [[ -z "$parsed" ]]; then
    return 1
  fi
  printf '%s' "$parsed"
}

run_count_check() {
  local label="$1"
  local raw
  raw="$(query_count_raw)"
  printf '%s\n' "$raw" >"$OUTPUT_DIR/${label}.txt"
  extract_count "$raw"
}

pid1=""
pid2=""
cleanup() {
  for pid in "$pid1" "$pid2"; do
    if [[ -n "$pid" ]] && kill -0 "$pid" >/dev/null 2>&1; then
      kill "$pid" >/dev/null 2>&1 || true
    fi
  done
}
trap cleanup EXIT

echo "==> baseline count"
baseline="$(run_count_check baseline)"
echo "baseline connected_clients=$baseline"

echo "==> opening two concurrent subscriptions"
spacetime subscribe "$DB" "SELECT * FROM active_connection" \
  --server "$SERVER" \
  --anonymous \
  --timeout "$TIMEOUT_SECONDS" \
  --print-initial-update \
  --yes >"$OUTPUT_DIR/sub1.log" 2>&1 &
pid1="$!"

spacetime subscribe "$DB" "SELECT * FROM active_connection" \
  --server "$SERVER" \
  --anonymous \
  --timeout "$TIMEOUT_SECONDS" \
  --print-initial-update \
  --yes >"$OUTPUT_DIR/sub2.log" 2>&1 &
pid2="$!"

sleep "$SLEEP_SECONDS"

echo "==> checking active count"
during="$(run_count_check during)"
echo "during connected_clients=$during"

min_expected=$((baseline + 2))
if (( during < min_expected )); then
  echo "handshake check failed: expected during >= $min_expected, got $during" >&2
  exit 1
fi

echo "==> waiting for subscriptions to close"
wait "$pid1" || true
wait "$pid2" || true
pid1=""
pid2=""

echo "==> checking final count"
final="$(run_count_check final)"
echo "final connected_clients=$final"

if (( final != baseline )); then
  echo "handshake check failed: expected final == baseline ($baseline), got $final" >&2
  exit 1
fi

cat >"$OUTPUT_DIR/SUMMARY.md" <<EOF
# Spacetime Maincloud Handshake Smoke Summary

- Timestamp (UTC): $timestamp
- Server: $SERVER
- Database: $DB
- Baseline connected clients: $baseline
- During subscriptions connected clients: $during
- Final connected clients: $final
- Result: PASS
EOF

echo "Handshake smoke passed."
echo "Artifacts: $OUTPUT_DIR"
