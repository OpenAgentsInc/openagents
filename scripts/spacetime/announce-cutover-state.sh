#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

OUTPUT_DIR=""
CONTROL_BASE_URL="${OA_CONTROL_BASE_URL:-}"
CONTROL_AUTH_TOKEN="${OA_CONTROL_AUTH_TOKEN:-}"
RUNTIME_BASE_URL="${OA_RUNTIME_BASE_URL:-}"
SKIP_REMOTE=0

usage() {
  cat <<'EOF'
Usage: scripts/spacetime/announce-cutover-state.sh [options]

Options:
  --output-dir <dir>         Write artifacts to this directory.
  --control-base-url <url>   Control service base URL (required unless --skip-remote).
  --auth-token <token>       Bearer token for control status (required unless --skip-remote).
  --runtime-base-url <url>   Runtime service base URL (required unless --skip-remote).
  --skip-remote              Generate cutover announcement artifact without live probes.
  -h, --help                 Show this help.

Environment equivalents:
  OA_CONTROL_BASE_URL
  OA_CONTROL_AUTH_TOKEN
  OA_RUNTIME_BASE_URL
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --output-dir)
      OUTPUT_DIR="$2"
      shift 2
      ;;
    --control-base-url)
      CONTROL_BASE_URL="$2"
      shift 2
      ;;
    --auth-token)
      CONTROL_AUTH_TOKEN="$2"
      shift 2
      ;;
    --runtime-base-url)
      RUNTIME_BASE_URL="$2"
      shift 2
      ;;
    --skip-remote)
      SKIP_REMOTE=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

timestamp="$(date -u +"%Y%m%dT%H%M%SZ")"
if [[ -z "$OUTPUT_DIR" ]]; then
  OUTPUT_DIR="$ROOT_DIR/output/canary/spacetime/cutover-state-$timestamp"
fi

CONTROL_JSON="$OUTPUT_DIR/control-status.json"
RUNTIME_JSON="$OUTPUT_DIR/runtime-spacetime-metrics.json"
SUMMARY_MD="$OUTPUT_DIR/SUMMARY.md"
RESULTS_JSON="$OUTPUT_DIR/result.json"

mkdir -p "$OUTPUT_DIR"

if [[ "$SKIP_REMOTE" -eq 0 ]]; then
  if [[ -z "$CONTROL_BASE_URL" || -z "$CONTROL_AUTH_TOKEN" || -z "$RUNTIME_BASE_URL" ]]; then
    echo "control/runtime URLs and auth token are required unless --skip-remote is set" >&2
    exit 2
  fi
  if ! command -v jq >/dev/null 2>&1; then
    echo "jq is required for cutover-state parsing" >&2
    exit 1
  fi

  curl -fsS \
    -H "authorization: Bearer ${CONTROL_AUTH_TOKEN}" \
    "${CONTROL_BASE_URL%/}/api/v1/control/status" >"$CONTROL_JSON"
  curl -fsS "${RUNTIME_BASE_URL%/}/internal/v1/spacetime/sync/metrics" >"$RUNTIME_JSON"

  control_transport="$(jq -r '.data.syncCutover.defaultTransport // empty' "$CONTROL_JSON")"
  control_khala_emergency="$(jq -r '.data.syncCutover.khalaEmergencyModeEnabled // empty' "$CONTROL_JSON")"
  runtime_transport="$(jq -r '.transport // empty' "$RUNTIME_JSON")"
  runtime_khala_emergency="$(jq -r '.khala_emergency_mode_enabled // empty' "$RUNTIME_JSON")"

  decision="allow"
  if [[ "$control_transport" != "spacetime_ws" || "$runtime_transport" != "spacetime_ws" ]]; then
    decision="block"
  fi
else
  control_transport="spacetime_ws"
  control_khala_emergency="unknown"
  runtime_transport="spacetime_ws"
  runtime_khala_emergency="unknown"
  decision="allow"
fi

cat >"$RESULTS_JSON" <<EOF
{
  "timestamp_utc": "$timestamp",
  "control_transport": "$control_transport",
  "control_khala_emergency_mode_enabled": "$control_khala_emergency",
  "runtime_transport": "$runtime_transport",
  "runtime_khala_emergency_mode_enabled": "$runtime_khala_emergency",
  "decision": "$decision",
  "skip_remote": $SKIP_REMOTE
}
EOF

{
  echo "# Spacetime Cutover State Announcement"
  echo
  echo "- Timestamp (UTC): $timestamp"
  echo "- Decision: $decision"
  echo "- Control default transport: $control_transport"
  echo "- Control khala emergency mode enabled: $control_khala_emergency"
  echo "- Runtime transport: $runtime_transport"
  echo "- Runtime khala emergency mode enabled: $runtime_khala_emergency"
  echo "- Skip remote probes: $SKIP_REMOTE"
  echo
  if [[ "$SKIP_REMOTE" -eq 0 ]]; then
    echo "## Evidence"
    echo
    echo "- $CONTROL_JSON"
    echo "- $RUNTIME_JSON"
  else
    echo "## Evidence"
    echo
    echo "- Remote probes skipped by operator request."
  fi
} >"$SUMMARY_MD"

echo "Cutover state artifacts:"
echo "  $RESULTS_JSON"
echo "  $SUMMARY_MD"
if [[ "$SKIP_REMOTE" -eq 0 ]]; then
  echo "  $CONTROL_JSON"
  echo "  $RUNTIME_JSON"
fi

if [[ "$decision" != "allow" ]]; then
  echo "Cutover state announcement blocked." >&2
  exit 1
fi

echo "Cutover state announcement ready."
