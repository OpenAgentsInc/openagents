#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd curl
require_cmd jq

NEXUS_BASE_URL="${NEXUS_BASE_URL:-${NEXUS_PUBLIC_URL:-}}"
NEXUS_BASE_URL="${NEXUS_BASE_URL%/}"
[[ -n "$NEXUS_BASE_URL" ]] || die "Set NEXUS_BASE_URL or NEXUS_PUBLIC_URL for LDK readiness smoke"
NEXUS_LDK_READINESS_ARTIFACT_DIR="${NEXUS_LDK_READINESS_ARTIFACT_DIR:-target/nexus-ldk-readiness/$(date -u +%Y%m%dT%H%M%SZ)}"
NEXUS_LDK_READINESS_FUNDING_SATS="${NEXUS_LDK_READINESS_FUNDING_SATS:-1000}"
NEXUS_LDK_READINESS_DESCRIPTION="${NEXUS_LDK_READINESS_DESCRIPTION:-OpenAgents Nexus LDK readiness smoke}"
NEXUS_LDK_WRITE_SMOKE="${NEXUS_LDK_WRITE_SMOKE:-false}"
NEXUS_ADMIN_TOKEN="${NEXUS_CONTROL_ADMIN_BEARER_TOKEN:-${NEXUS_ADMIN_BEARER_TOKEN:-}}"

mkdir -p "$NEXUS_LDK_READINESS_ARTIFACT_DIR"

api_url() {
  printf '%s%s\n' "${NEXUS_BASE_URL%/}" "$1"
}

curl_json() {
  local method="$1"
  local path="$2"
  local body="$3"
  local output="$4"
  if [[ -n "$body" ]]; then
    curl -fsS \
      -X "$method" \
      -H 'content-type: application/json' \
      --data "$body" \
      "$(api_url "$path")" \
      -o "$output"
  else
    curl -fsS \
      -X "$method" \
      "$(api_url "$path")" \
      -o "$output"
  fi
}

admin_json() {
  local body="$1"
  local output="$2"
  [[ -n "$NEXUS_ADMIN_TOKEN" ]] || die "Set NEXUS_CONTROL_ADMIN_BEARER_TOKEN or NEXUS_ADMIN_BEARER_TOKEN for admin LDK readiness smoke"
  curl -fsS \
    -X POST \
    -H 'content-type: application/json' \
    -H "authorization: Bearer ${NEXUS_ADMIN_TOKEN}" \
    --data "$body" \
    "$(api_url /v1/admin/treasury/operations)" \
    -o "$output"
}

admin_operation() {
  local operation="$1"
  local params="$2"
  local output="$3"
  admin_json "$(jq -n --arg operation "$operation" --argjson params "$params" '{operation: $operation, params: $params}')" "$output"
}

admin_write_operation() {
  local operation="$1"
  local params="$2"
  local key="$3"
  local output="$4"
  admin_json "$(jq -n --arg operation "$operation" --arg key "$key" --argjson params "$params" '{operation: $operation, idempotency_key: $key, params: $params}')" "$output"
}

status_json="${NEXUS_LDK_READINESS_ARTIFACT_DIR}/treasury-status.json"
curl_json GET /v1/treasury/status "" "$status_json"
jq -e '.active_treasury_provider == "ldk" and .active_treasury_rail == "ldk"' "$status_json" >/dev/null
jq -e '.ldk_readiness.state | type == "string" and length > 0' "$status_json" >/dev/null

funding_json="${NEXUS_LDK_READINESS_ARTIFACT_DIR}/funding-target.json"
curl_json POST /v1/treasury/funding-target \
  "$(jq -n \
    --argjson amount_sats "$NEXUS_LDK_READINESS_FUNDING_SATS" \
    --arg description "$NEXUS_LDK_READINESS_DESCRIPTION" \
    '{amount_sats: $amount_sats, description: $description, expiry_seconds: 3600}')" \
  "$funding_json"
jq -e '.bolt11_invoice | type == "string" and startswith("ln")' "$funding_json" >/dev/null

admin_operation treasury.status '{}' "${NEXUS_LDK_READINESS_ARTIFACT_DIR}/admin-status.json"
admin_operation treasury.listPeers '{}' "${NEXUS_LDK_READINESS_ARTIFACT_DIR}/admin-peers.json"
admin_operation treasury.listChannels '{}' "${NEXUS_LDK_READINESS_ARTIFACT_DIR}/admin-channels.json"
admin_operation treasury.listPayments '{}' "${NEXUS_LDK_READINESS_ARTIFACT_DIR}/admin-payments.json"

if [[ "$NEXUS_LDK_WRITE_SMOKE" == "true" ]]; then
  [[ -n "${NEXUS_LDK_SMOKE_PEER_NODE_ID:-}" ]] || die "Set NEXUS_LDK_SMOKE_PEER_NODE_ID for write smoke"
  admin_write_operation treasury.connectPeer \
    "$(jq -n \
      --arg peer_node_id "$NEXUS_LDK_SMOKE_PEER_NODE_ID" \
      --arg address "${NEXUS_LDK_SMOKE_PEER_ADDRESS:-}" \
      'if $address == "" then {peer_node_id: $peer_node_id} else {peer_node_id: $peer_node_id, address: $address} end')" \
    "ldk-readiness-connect-peer-$(date -u +%Y%m%dT%H%M%SZ)" \
    "${NEXUS_LDK_READINESS_ARTIFACT_DIR}/write-connect-peer.json"

  if [[ -n "${NEXUS_LDK_SMOKE_CHANNEL_AMOUNT_SATS:-}" ]]; then
    admin_write_operation treasury.openChannel \
      "$(jq -n \
        --arg peer_node_id "$NEXUS_LDK_SMOKE_PEER_NODE_ID" \
        --argjson amount_sats "$NEXUS_LDK_SMOKE_CHANNEL_AMOUNT_SATS" \
        '{peer_node_id: $peer_node_id, amount_sats: $amount_sats}')" \
      "ldk-readiness-open-channel-$(date -u +%Y%m%dT%H%M%SZ)" \
      "${NEXUS_LDK_READINESS_ARTIFACT_DIR}/write-open-channel.json"
  fi

  if [[ -n "${NEXUS_LDK_SMOKE_PAY_INVOICE:-}" ]]; then
    admin_write_operation treasury.payInvoice \
      "$(jq -n \
        --arg invoice "$NEXUS_LDK_SMOKE_PAY_INVOICE" \
        --argjson amount_sats "${NEXUS_LDK_SMOKE_PAY_AMOUNT_SATS:-null}" \
        'if $amount_sats == null then {invoice: $invoice} else {invoice: $invoice, amount_sats: $amount_sats} end')" \
      "ldk-readiness-pay-invoice-$(date -u +%Y%m%dT%H%M%SZ)" \
      "${NEXUS_LDK_READINESS_ARTIFACT_DIR}/write-pay-invoice.json"
  fi

  if [[ -n "${NEXUS_LDK_SMOKE_PAY_OFFER:-}" ]]; then
    admin_write_operation treasury.payOffer \
      "$(jq -n \
        --arg offer "$NEXUS_LDK_SMOKE_PAY_OFFER" \
        --argjson amount_sats "${NEXUS_LDK_SMOKE_PAY_AMOUNT_SATS:-null}" \
        'if $amount_sats == null then {offer: $offer} else {offer: $offer, amount_sats: $amount_sats} end')" \
      "ldk-readiness-pay-offer-$(date -u +%Y%m%dT%H%M%SZ)" \
      "${NEXUS_LDK_READINESS_ARTIFACT_DIR}/write-pay-offer.json"
  fi
fi

log "LDK production readiness smoke completed"
log "Artifacts: ${NEXUS_LDK_READINESS_ARTIFACT_DIR}"
