#!/usr/bin/env bash
# Fetch node info and TLS cert from Voltage API (for L402/Aperture setup).
# Requires: VOLTAGE_API_KEY (from env or repo root .env.local; never commit .env.local to public repo).
# Optional: VOLTAGE_ORG_ID (default: placeholder; API accepts it when key is valid). VOLTAGE_NODE_ID (default: list nodes and pick first or openagents).
# Usage: ./voltage-api-fetch.sh [output_dir]
#   output_dir: optional; if set, writes tls.cert there and prints node JSON to node.json.

set -euo pipefail
BASE_URL="${VOLTAGE_API_BASE_URL:-https://api.voltage.cloud}"

# Load VOLTAGE_* from repo root .env.local if key not already set
if [[ -z "${VOLTAGE_API_KEY:-}" ]]; then
  SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
  if [[ -f "$ROOT/.env.local" ]]; then
    set -a
    # shellcheck source=/dev/null
    source "$ROOT/.env.local" 2>/dev/null || true
    set +a
  fi
fi

if [[ -z "${VOLTAGE_API_KEY:-}" ]]; then
  echo "VOLTAGE_API_KEY is required (set in env or in repo root .env.local)" >&2
  exit 1
fi

# API accepts placeholder org id when key is valid; no need to look up real org id
VOLTAGE_ORG_ID="${VOLTAGE_ORG_ID:-00000000-0000-0000-0000-000000000000}"
OUTPUT_DIR="${1:-}"
NODE_ID="${VOLTAGE_NODE_ID:-}"

curl_one() {
  /usr/bin/curl -s -H "Accept: application/json" -H "Content-Type: application/json" -H "X-VOLTAGE-AUTH: $VOLTAGE_API_KEY" "$@"
}

# Resolve node id if not set: list nodes and take first (or match by api_endpoint)
if [[ -z "$NODE_ID" ]]; then
  echo "Listing nodes (organization_id=$VOLTAGE_ORG_ID)..." >&2
  NODES_JSON=$(curl_one "$BASE_URL/organizations/$VOLTAGE_ORG_ID/nodes")
  if echo "$NODES_JSON" | grep -q '"message"'; then
    echo "$NODES_JSON" >&2
    exit 1
  fi
  if command -v jq &>/dev/null; then
    NODE_ID=$(echo "$NODES_JSON" | jq -r '.nodes | (map(select(.api_endpoint | test("openagents"; "i"))) | .[0]) // .[0] | .node_id // empty')
  elif command -v node &>/dev/null; then
    NODE_ID=$(echo "$NODES_JSON" | node -e "
      const d = JSON.parse(require('fs').readFileSync('/dev/stdin', 'utf8'));
      const nodes = d.nodes || [];
      if (nodes.length === 0) { process.stderr.write('No nodes in org'); process.exit(1); }
      const n = nodes.find(n => n.api_endpoint && String(n.api_endpoint).includes('openagents')) || nodes[0];
      console.log(n.node_id);
    ")
  else
    echo "Set VOLTAGE_NODE_ID (or install jq/node to auto-detect from list)" >&2
    echo "Nodes response (first 800 chars): ${NODES_JSON:0:800}" >&2
    exit 1
  fi
  echo "Using node_id=$NODE_ID" >&2
fi

# Get single node details
echo "Fetching node details..." >&2
NODE_JSON=$(curl_one "$BASE_URL/organizations/$VOLTAGE_ORG_ID/nodes/$NODE_ID")
if echo "$NODE_JSON" | grep -q '"message"'; then
  echo "Node fetch failed: $NODE_JSON" >&2
  exit 1
fi
echo "$NODE_JSON"

# Get TLS cert (JSON with tls_cert field, base64-encoded PEM)
CERT_JSON=$(curl_one "$BASE_URL/organizations/$VOLTAGE_ORG_ID/nodes/$NODE_ID/cert")
if echo "$CERT_JSON" | grep -q '"tls_cert"'; then
  if [[ -n "$OUTPUT_DIR" ]]; then
    mkdir -p "$OUTPUT_DIR"
    CERT_B64=$(echo "$CERT_JSON" | jq -r '.tls_cert // empty' 2>/dev/null || echo "$CERT_JSON" | sed -n 's/.*"tls_cert"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')
    if [[ -n "$CERT_B64" ]]; then
      if command -v base64 &>/dev/null; then
        ( echo "$CERT_B64" | base64 -d 2>/dev/null || echo "$CERT_B64" | base64 -D 2>/dev/null ) > "$OUTPUT_DIR/tls.cert" && echo "Wrote $OUTPUT_DIR/tls.cert" >&2
      elif command -v node &>/dev/null; then
        echo "$CERT_B64" | node -e "const b=require('fs').readFileSync('/dev/stdin','utf8').trim(); require('fs').writeFileSync('$OUTPUT_DIR/tls.cert', Buffer.from(b,'base64'));" && echo "Wrote $OUTPUT_DIR/tls.cert" >&2
      fi
    fi
    echo "$NODE_JSON" > "$OUTPUT_DIR/node.json"
  fi
else
  echo "Cert fetch response (no tls_cert?): $CERT_JSON" >&2
fi
