#!/usr/bin/env bash
set -euo pipefail

OA_KHALA_MCP_PROD_ACCESS_ENABLED="${OA_KHALA_MCP_PROD_ACCESS_ENABLED:-0}"
OA_CHANGE_TICKET="${OA_CHANGE_TICKET:-}"
OA_MCP_PROD_ACCESS_REASON="${OA_MCP_PROD_ACCESS_REASON:-}"
OA_MCP_PROD_ACCESS_TTL_MINUTES="${OA_MCP_PROD_ACCESS_TTL_MINUTES:-0}"
OA_MCP_PROD_ACKNOWLEDGE_RISK="${OA_MCP_PROD_ACKNOWLEDGE_RISK:-}"

MAX_TTL_MINUTES=60

if [[ "$OA_KHALA_MCP_PROD_ACCESS_ENABLED" != "1" ]]; then
  echo "MCP production access denied: default posture is disabled."
  echo "Set OA_KHALA_MCP_PROD_ACCESS_ENABLED=1 with required change-control fields for temporary enablement."
  exit 1
fi

if [[ -z "$OA_CHANGE_TICKET" ]]; then
  echo "MCP production access denied: OA_CHANGE_TICKET is required." >&2
  exit 1
fi

if [[ -z "$OA_MCP_PROD_ACCESS_REASON" ]]; then
  echo "MCP production access denied: OA_MCP_PROD_ACCESS_REASON is required." >&2
  exit 1
fi

if [[ "$OA_MCP_PROD_ACKNOWLEDGE_RISK" != "YES" ]]; then
  echo "MCP production access denied: set OA_MCP_PROD_ACKNOWLEDGE_RISK=YES." >&2
  exit 1
fi

if ! [[ "$OA_MCP_PROD_ACCESS_TTL_MINUTES" =~ ^[0-9]+$ ]]; then
  echo "MCP production access denied: OA_MCP_PROD_ACCESS_TTL_MINUTES must be an integer." >&2
  exit 1
fi

if (( OA_MCP_PROD_ACCESS_TTL_MINUTES <= 0 || OA_MCP_PROD_ACCESS_TTL_MINUTES > MAX_TTL_MINUTES )); then
  echo "MCP production access denied: TTL must be between 1 and $MAX_TTL_MINUTES minutes." >&2
  exit 1
fi

REQUESTED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

if EXPIRES_AT="$(date -u -v+"$OA_MCP_PROD_ACCESS_TTL_MINUTES"M +%Y-%m-%dT%H:%M:%SZ 2>/dev/null)"; then
  :
elif EXPIRES_AT="$(date -u -d "+$OA_MCP_PROD_ACCESS_TTL_MINUTES minutes" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null)"; then
  :
else
  EXPIRES_AT="unknown"
fi

echo "MCP production access gate: APPROVED (temporary)"
echo "Change ticket: $OA_CHANGE_TICKET"
echo "Reason:        $OA_MCP_PROD_ACCESS_REASON"
echo "Requested at:  $REQUESTED_AT"
echo "Expires at:    $EXPIRES_AT"
