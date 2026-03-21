#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

RUN_DIR="${OPENAGENTS_HEADLESS_DATA_MARKET_E2E_RUN_DIR:-$ROOT_DIR/target/headless-data-market-e2e}"
SELLER_HOME="$RUN_DIR/seller-home"
BUYER_HOME="$RUN_DIR/buyer-home"
SELLER_LOG_DIR="$RUN_DIR/seller-logs"
BUYER_LOG_DIR="$RUN_DIR/buyer-logs"
SELLER_MANIFEST="$RUN_DIR/seller-desktop-control.json"
BUYER_MANIFEST="$RUN_DIR/buyer-desktop-control.json"
SELLER_IDENTITY_PATH="$SELLER_HOME/.openagents/pylon/identity.mnemonic"
BUYER_IDENTITY_PATH="$BUYER_HOME/.openagents/pylon/identity.mnemonic"
SELLER_SETTINGS_PATH="$SELLER_HOME/.openagents/autopilot-settings-v1.conf"
BUYER_SETTINGS_PATH="$BUYER_HOME/.openagents/autopilot-settings-v1.conf"
WAIT_TIMEOUT_MS="${OPENAGENTS_HEADLESS_DATA_MARKET_WAIT_TIMEOUT_MS:-120000}"
REQUEST_TIMEOUT_SECONDS="${OPENAGENTS_HEADLESS_DATA_MARKET_REQUEST_TIMEOUT_SECONDS:-120}"
PRICE_SATS="${OPENAGENTS_HEADLESS_DATA_MARKET_PRICE_SATS:-5}"
BUYER_PREFUND_SATS="${OPENAGENTS_HEADLESS_DATA_MARKET_BUYER_PREFUND_SATS:-0}"
BUYER_PREFUND_TIMEOUT_SECONDS="${OPENAGENTS_HEADLESS_DATA_MARKET_BUYER_PREFUND_TIMEOUT_SECONDS:-90}"
PREFUND_PAYER_IDENTITY_PATH="${OPENAGENTS_HEADLESS_DATA_MARKET_PREFUND_PAYER_IDENTITY_PATH:-$HOME/.openagents/pylon/identity.mnemonic}"
PREFUND_PAYER_STORAGE_DIR="${OPENAGENTS_HEADLESS_DATA_MARKET_PREFUND_PAYER_STORAGE_DIR:-$HOME/.openagents/pylon/spark/mainnet}"
RELAY_URLS_CSV="${OPENAGENTS_HEADLESS_DATA_MARKET_RELAY_URLS:-}"
LIVE_INGEST_WAIT_SECONDS="${OPENAGENTS_HEADLESS_DATA_MARKET_LIVE_INGEST_WAIT_SECONDS:-30}"
REQUIRE_LIVE_INGEST="${OPENAGENTS_HEADLESS_DATA_MARKET_REQUIRE_LIVE_INGEST:-false}"
PUBLIC_SUBSCRIPTION_SETTLE_SECONDS="${OPENAGENTS_HEADLESS_DATA_MARKET_PUBLIC_SUBSCRIPTION_SETTLE_SECONDS:-8}"

rm -rf "$RUN_DIR"
mkdir -p \
  "$RUN_DIR" \
  "$SELLER_HOME/.openagents/pylon" \
  "$BUYER_HOME/.openagents/pylon" \
  "$SELLER_LOG_DIR" \
  "$BUYER_LOG_DIR"

find_free_port() {
  python3 - <<'PY'
import socket
s = socket.socket()
s.bind(("127.0.0.1", 0))
print(s.getsockname()[1])
s.close()
PY
}

json_field() {
  local file="$1"
  local field="$2"
  python3 - "$file" "$field" <<'PY'
import json, pathlib, sys
payload = json.loads(pathlib.Path(sys.argv[1]).read_text())
node = payload
for key in sys.argv[2].split("."):
    if not key:
        continue
    node = node[key]
if node is None:
    print("")
elif isinstance(node, bool):
    print("true" if node else "false")
else:
    print(node)
PY
}

join_by_comma() {
  local IFS=','
  echo "$*"
}

normalize_relay_csv() {
  python3 - "$1" <<'PY'
import sys

raw = sys.argv[1]
parts = []
seen = set()
for value in raw.split(","):
    relay = value.strip()
    if not relay or relay in seen:
        continue
    seen.add(relay)
    parts.append(relay)
print(",".join(parts))
PY
}

write_settings() {
  local path="$1"
  local relay_url="$2"
  local backup_relay_urls="$3"
  local identity_path="$4"
  mkdir -p "$(dirname "$path")"
  cat >"$path" <<EOF
schema_version=2
primary_relay_url=${relay_url}
backup_relay_urls=${backup_relay_urls}
identity_path=${identity_path}
wallet_default_send_sats=1000
provider_max_queue_depth=1
reconnect_required=false
EOF
}

mint_desktop_session() {
  local base_url="$1"
  local client_id="$2"
  local device_name="$3"
  local output_path="$4"
  python3 - "$base_url" "$client_id" "$device_name" "$output_path" <<'PY'
import json
import sys
import urllib.request

base_url = sys.argv[1]
client_id = sys.argv[2]
device_name = sys.argv[3]
output_path = sys.argv[4]
payload = {
    "desktop_client_id": client_id,
    "device_name": device_name,
    "client_version": "headless-data-market-e2e",
}
request = urllib.request.Request(
    base_url + "/api/session/desktop",
    data=json.dumps(payload).encode("utf-8"),
    headers={"content-type": "application/json"},
    method="POST",
)
with urllib.request.urlopen(request, timeout=5) as response:
    body = response.read().decode("utf-8")
parsed = json.loads(body)
with open(output_path, "w", encoding="utf-8") as handle:
    json.dump(parsed, handle)
PY
}

wait_for_file() {
  local path="$1"
  local timeout_seconds="$2"
  local deadline=$((SECONDS + timeout_seconds))
  while (( SECONDS < deadline )); do
    if [[ -f "$path" ]]; then
      return 0
    fi
    sleep 1
  done
  return 1
}

wait_for_status() {
  local manifest="$1"
  local output_path="$2"
  local timeout_seconds="$3"
  local deadline=$((SECONDS + timeout_seconds))
  while (( SECONDS < deadline )); do
    if "$AUTOPILOTCTL_BIN" --manifest "$manifest" --json status >"$output_path" 2>/dev/null; then
      return 0
    fi
    sleep 1
  done
  return 1
}

wait_for_wallet_balance() {
  local identity_path="$1"
  local storage_dir="$2"
  local minimum_sats="$3"
  local output_path="$4"
  local timeout_seconds="$5"
  local deadline=$((SECONDS + timeout_seconds))
  while (( SECONDS < deadline )); do
    "$SPARK_WALLET_CLI_BIN" \
      --identity-path "$identity_path" \
      --storage-dir "$storage_dir" \
      status >"$output_path"
    if python3 - "$output_path" "$minimum_sats" <<'PY'
import json, pathlib, sys
payload = json.loads(pathlib.Path(sys.argv[1]).read_text())
minimum_sats = int(sys.argv[2])
balance = payload.get("balance", {}).get("totalSats", 0)
raise SystemExit(0 if balance >= minimum_sats else 1)
PY
    then
      return 0
    fi
    sleep 2
  done
  return 1
}

prefund_buyer_wallet() {
  local amount_sats="$1"
  if [[ "$amount_sats" -le 0 ]]; then
    return 0
  fi
  if [[ ! -f "$PREFUND_PAYER_IDENTITY_PATH" ]]; then
    echo "buyer prefund payer identity not found at $PREFUND_PAYER_IDENTITY_PATH" >&2
    exit 1
  fi
  if [[ ! -d "$PREFUND_PAYER_STORAGE_DIR" ]]; then
    echo "buyer prefund payer storage dir not found at $PREFUND_PAYER_STORAGE_DIR" >&2
    exit 1
  fi

  local buyer_storage_dir="$BUYER_HOME/.openagents/pylon/spark/mainnet"
  local invoice_output="$RUN_DIR/buyer-prefund-invoice.json"
  local pay_output="$RUN_DIR/buyer-prefund-pay.json"
  local buyer_status_output="$RUN_DIR/buyer-wallet-status-prefunded.json"

  echo "prefunding buyer wallet with ${amount_sats} sats"
  "$SPARK_WALLET_CLI_BIN" \
    --identity-path "$BUYER_IDENTITY_PATH" \
    --storage-dir "$buyer_storage_dir" \
    bolt11-invoice "$amount_sats" \
    --description "headless data-market buyer prefund" \
    --expiry-seconds 600 >"$invoice_output"
  local invoice
  invoice="$(json_field "$invoice_output" invoice)"
  if [[ -z "$invoice" ]]; then
    echo "failed to create buyer prefund invoice" >&2
    cat "$invoice_output" >&2 || true
    exit 1
  fi

  "$SPARK_WALLET_CLI_BIN" \
    --identity-path "$PREFUND_PAYER_IDENTITY_PATH" \
    --storage-dir "$PREFUND_PAYER_STORAGE_DIR" \
    pay-invoice "$invoice" >"$pay_output"

  wait_for_wallet_balance \
    "$BUYER_IDENTITY_PATH" \
    "$buyer_storage_dir" \
    "$amount_sats" \
    "$buyer_status_output" \
    "$BUYER_PREFUND_TIMEOUT_SECONDS" || {
      echo "buyer wallet never reached prefunded balance ${amount_sats} sats" >&2
      cat "$buyer_status_output" >&2 || true
      exit 1
    }
}

wait_for_seller_request_evaluation() {
  local manifest="$1"
  local request_id="$2"
  local output_path="$3"
  local timeout_seconds="$4"
  local expected_disposition="$5"
  local deadline=$((SECONDS + timeout_seconds))
  while (( SECONDS < deadline )); do
    "$AUTOPILOTCTL_BIN" --manifest "$manifest" --json data-market seller-status >"$output_path"
    if python3 - "$output_path" "$request_id" "$expected_disposition" <<'PY'
import json, pathlib, sys
payload = json.loads(pathlib.Path(sys.argv[1]).read_text())
request_id = sys.argv[2]
expected = sys.argv[3]
latest = payload.get("payload", {}).get("seller", {}).get("latest_incoming_request")
if not latest:
    raise SystemExit(1)
if latest.get("request_id") != request_id:
    raise SystemExit(1)
if latest.get("evaluation_disposition") != expected:
    raise SystemExit(1)
raise SystemExit(0)
PY
    then
      return 0
    fi
    sleep 2
  done
  return 1
}

wait_for_seller_payment_settled() {
  local manifest="$1"
  local request_id="$2"
  local output_path="$3"
  local timeout_seconds="$4"
  local deadline=$((SECONDS + timeout_seconds))
  while (( SECONDS < deadline )); do
    "$AUTOPILOTCTL_BIN" --manifest "$manifest" --json data-market seller-status >"$output_path"
    if python3 - "$output_path" "$request_id" <<'PY'
import json, pathlib, sys
payload = json.loads(pathlib.Path(sys.argv[1]).read_text())
request_id = sys.argv[2]
latest = payload.get("payload", {}).get("seller", {}).get("latest_incoming_request")
if not latest:
    raise SystemExit(1)
if latest.get("request_id") != request_id:
    raise SystemExit(1)
payment = latest.get("payment") or {}
if payment.get("state") != "paid":
    raise SystemExit(1)
if not payment.get("payment_pointer"):
    raise SystemExit(1)
raise SystemExit(0)
PY
    then
      return 0
    fi
    sleep 2
  done
  return 1
}

import_seller_request_from_relays() {
  local manifest="$1"
  local request_id="$2"
  local output_path="$3"
  shift 3
  local relay_urls=("$@")
  local args=()
  local relay_url
  for relay_url in "${relay_urls[@]}"; do
    args+=(--relay-url "$relay_url")
  done
  "$AUTOPILOTCTL_BIN" --manifest "$manifest" --json data-market seller-import-request \
    --event-id "$request_id" \
    "${args[@]}" >"$output_path"
}

wait_for_buyer_result() {
  local manifest="$1"
  local request_id="$2"
  local output_path="$3"
  local timeout_seconds="$4"
  local deadline=$((SECONDS + timeout_seconds))
  while (( SECONDS < deadline )); do
    "$AUTOPILOTCTL_BIN" --manifest "$manifest" --json data-market buyer-status >"$output_path"
    if python3 - "$output_path" "$request_id" <<'PY'
import json, pathlib, sys
payload = json.loads(pathlib.Path(sys.argv[1]).read_text())
request_id = sys.argv[2]
latest = payload.get("payload", {}).get("buyer", {}).get("latest_request")
if not latest:
    raise SystemExit(1)
if latest.get("request_id") != request_id:
    raise SystemExit(1)
if not latest.get("last_result_event_id"):
    raise SystemExit(1)
raise SystemExit(0)
PY
    then
      return 0
    fi
    sleep 2
  done
  return 1
}

import_buyer_response_from_relays() {
  local manifest="$1"
  local event_id="$2"
  local output_path="$3"
  shift 3
  local relay_urls=("$@")
  local args=()
  local relay_url
  for relay_url in "${relay_urls[@]}"; do
    args+=(--relay-url "$relay_url")
  done
  "$AUTOPILOTCTL_BIN" --manifest "$manifest" --json data-market buyer-import-response \
    --event-id "$event_id" \
    "${args[@]}" >"$output_path"
}

DEFAULT_SPARK_API_KEY="$(
python3 - <<'PY'
import pathlib, re
text = pathlib.Path("apps/autopilot-desktop/src/spark_wallet.rs").read_text()
match = re.search(r'DEFAULT_OPENAGENTS_SPARK_API_KEY: &str = "([^"]+)";', text)
if not match:
    raise SystemExit("failed to locate default OPENAGENTS_SPARK_API_KEY fallback")
print(match.group(1))
PY
)"
export OPENAGENTS_SPARK_API_KEY="${OPENAGENTS_SPARK_API_KEY:-$DEFAULT_SPARK_API_KEY}"

echo "building headless data-market binaries"
cargo build \
  -p nexus-control \
  -p autopilot-desktop \
  --bin autopilot-headless-compute \
  --bin autopilot_headless_data_market \
  --bin autopilotctl \
  --bin spark-wallet-cli

AUTOPILOTCTL_BIN="$ROOT_DIR/target/debug/autopilotctl"
HEADLESS_COMPUTE_BIN="$ROOT_DIR/target/debug/autopilot-headless-compute"
HEADLESS_DATA_MARKET_BIN="$ROOT_DIR/target/debug/autopilot_headless_data_market"
NEXUS_CONTROL_BIN="$ROOT_DIR/target/debug/nexus-control"
SPARK_WALLET_CLI_BIN="$ROOT_DIR/target/debug/spark-wallet-cli"

cleanup() {
  set +e
  if [[ -n "${BUYER_PID:-}" ]]; then
    kill "$BUYER_PID" 2>/dev/null || true
  fi
  if [[ -n "${SELLER_PID:-}" ]]; then
    kill "$SELLER_PID" 2>/dev/null || true
  fi
  if [[ -n "${RELAY_PID:-}" ]]; then
    kill "$RELAY_PID" 2>/dev/null || true
  fi
  if [[ -n "${NEXUS_PID:-}" ]]; then
    kill "$NEXUS_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

RELAY_URL=""
CONFIGURED_RELAY_URLS=()
if [[ -n "$RELAY_URLS_CSV" ]]; then
  NORMALIZED_RELAY_URLS_CSV="$(normalize_relay_csv "$RELAY_URLS_CSV")"
  if [[ -z "$NORMALIZED_RELAY_URLS_CSV" ]]; then
    echo "OPENAGENTS_HEADLESS_DATA_MARKET_RELAY_URLS did not contain any usable relays" >&2
    exit 1
  fi
  IFS=',' read -r -a CONFIGURED_RELAY_URLS <<< "$NORMALIZED_RELAY_URLS_CSV"
  echo "using configured relay set: ${NORMALIZED_RELAY_URLS_CSV}"
else
  RELAY_PORT="$(find_free_port)"
  RELAY_URL="ws://127.0.0.1:${RELAY_PORT}"
  echo "starting local relay on ${RELAY_URL}"
  "$HEADLESS_COMPUTE_BIN" relay --listen "127.0.0.1:${RELAY_PORT}" >"$RUN_DIR/relay.log" 2>&1 &
  RELAY_PID=$!
  sleep 2
  CONFIGURED_RELAY_URLS=("$RELAY_URL")
  NORMALIZED_RELAY_URLS_CSV="$RELAY_URL"
fi

PRIMARY_RELAY_URL="${CONFIGURED_RELAY_URLS[0]}"
BACKUP_RELAY_URLS_CSV=""
if (( ${#CONFIGURED_RELAY_URLS[@]} > 1 )); then
  BACKUP_RELAY_URLS_CSV="$(join_by_comma "${CONFIGURED_RELAY_URLS[@]:1}")"
fi

echo "starting nexus-control"
NEXUS_CONTROL_LISTEN_ADDR=127.0.0.1:0 \
NEXUS_CONTROL_KERNEL_STATE_PATH="$RUN_DIR/kernel-state.json" \
NEXUS_CONTROL_RECEIPT_LOG_PATH="$RUN_DIR/receipt-log.jsonl" \
"$NEXUS_CONTROL_BIN" >"$RUN_DIR/nexus-control.log" 2>&1 &
NEXUS_PID=$!

NEXUS_BASE_URL=""
for _ in $(seq 1 60); do
  if grep -Eq 'nexus-control listening on 127\.0\.0\.1:[0-9]+' "$RUN_DIR/nexus-control.log"; then
    NEXUS_PORT="$(sed -n 's/.*nexus-control listening on 127\.0\.0\.1:\([0-9][0-9]*\).*/\1/p' "$RUN_DIR/nexus-control.log" | tail -n 1)"
    if [[ -n "$NEXUS_PORT" ]]; then
      NEXUS_BASE_URL="http://127.0.0.1:${NEXUS_PORT}"
      break
    fi
  fi
  sleep 1
done

if [[ -z "$NEXUS_BASE_URL" ]]; then
  echo "nexus-control did not become reachable" >&2
  cat "$RUN_DIR/nexus-control.log" >&2
  exit 1
fi

echo "creating seller and buyer desktop sessions"
mint_desktop_session "$NEXUS_BASE_URL" "headless-data-market-seller" "Headless Data Market Seller" "$RUN_DIR/seller-session.json"
mint_desktop_session "$NEXUS_BASE_URL" "headless-data-market-buyer" "Headless Data Market Buyer" "$RUN_DIR/buyer-session.json"
SELLER_ACCESS_TOKEN="$(json_field "$RUN_DIR/seller-session.json" access_token)"
BUYER_ACCESS_TOKEN="$(json_field "$RUN_DIR/buyer-session.json" access_token)"

echo "creating isolated nostr identities"
"$HEADLESS_COMPUTE_BIN" identity --identity-path "$SELLER_IDENTITY_PATH" >"$RUN_DIR/seller-identity.json"
"$HEADLESS_COMPUTE_BIN" identity --identity-path "$BUYER_IDENTITY_PATH" >"$RUN_DIR/buyer-identity.json"
SELLER_NPUB="$(json_field "$RUN_DIR/seller-identity.json" npub)"
BUYER_NPUB="$(json_field "$RUN_DIR/buyer-identity.json" npub)"

write_settings "$SELLER_SETTINGS_PATH" "$PRIMARY_RELAY_URL" "$BACKUP_RELAY_URLS_CSV" "$SELLER_IDENTITY_PATH"
write_settings "$BUYER_SETTINGS_PATH" "$PRIMARY_RELAY_URL" "$BACKUP_RELAY_URLS_CSV" "$BUYER_IDENTITY_PATH"

echo "launching seller runtime"
HOME="$SELLER_HOME" \
OPENAGENTS_AUTOPILOT_LOG_DIR="$SELLER_LOG_DIR" \
OPENAGENTS_SPARK_API_KEY="$OPENAGENTS_SPARK_API_KEY" \
OA_CONTROL_BASE_URL="$NEXUS_BASE_URL" \
OA_CONTROL_BEARER_TOKEN="$SELLER_ACCESS_TOKEN" \
"$HEADLESS_DATA_MARKET_BIN" --manifest-path "$SELLER_MANIFEST" >"$RUN_DIR/seller.stdout.log" 2>"$RUN_DIR/seller.stderr.log" &
SELLER_PID=$!

echo "launching buyer runtime"
HOME="$BUYER_HOME" \
OPENAGENTS_AUTOPILOT_LOG_DIR="$BUYER_LOG_DIR" \
OPENAGENTS_SPARK_API_KEY="$OPENAGENTS_SPARK_API_KEY" \
OA_CONTROL_BASE_URL="$NEXUS_BASE_URL" \
OA_CONTROL_BEARER_TOKEN="$BUYER_ACCESS_TOKEN" \
"$HEADLESS_DATA_MARKET_BIN" --manifest-path "$BUYER_MANIFEST" >"$RUN_DIR/buyer.stdout.log" 2>"$RUN_DIR/buyer.stderr.log" &
BUYER_PID=$!

wait_for_file "$SELLER_MANIFEST" 60 || { echo "seller manifest did not appear" >&2; exit 1; }
wait_for_file "$BUYER_MANIFEST" 60 || { echo "buyer manifest did not appear" >&2; exit 1; }
wait_for_status "$SELLER_MANIFEST" "$RUN_DIR/seller-status.json" 60 || {
  echo "seller runtime did not become reachable" >&2
  cat "$RUN_DIR/seller.stderr.log" >&2 || true
  exit 1
}
wait_for_status "$BUYER_MANIFEST" "$RUN_DIR/buyer-status.json" 60 || {
  echo "buyer runtime did not become reachable" >&2
  cat "$RUN_DIR/buyer.stderr.log" >&2 || true
  exit 1
}

prefund_buyer_wallet "$BUYER_PREFUND_SATS"

echo "creating dummy dataset"
mkdir -p "$RUN_DIR/source-dataset"
cat >"$RUN_DIR/source-dataset/rows.csv" <<'EOF'
id,value
1,alpha
2,beta
3,gamma
EOF
cat >"$RUN_DIR/source-dataset/README.md" <<'EOF'
# Dummy Dataset

This is a local headless Data Market E2E fixture.
EOF

echo "packaging dataset for sale"
"$ROOT_DIR/scripts/autopilot/data_market_package.py" \
  --source "$RUN_DIR/source-dataset" \
  --output-dir "$RUN_DIR/package" \
  --title "Headless Dummy Dataset" \
  --description "Dummy dataset for local headless Data Market verification." \
  --default-policy targeted_request \
  --grant-policy-template targeted_request \
  --consumer-id "$BUYER_NPUB" \
  --price-sats "$PRICE_SATS" \
  --grant-price-sats "$PRICE_SATS" \
  --grant-expires-hours 24 \
  --grant-warranty-window-hours 4 \
  >"$RUN_DIR/package-summary.stdout.json"

echo "publishing asset and grant"
"$AUTOPILOTCTL_BIN" --manifest "$SELLER_MANIFEST" --json data-market draft-asset --file "$RUN_DIR/package/listing-template.json" >"$RUN_DIR/draft-asset.json"
"$AUTOPILOTCTL_BIN" --manifest "$SELLER_MANIFEST" --json data-market preview-asset >"$RUN_DIR/preview-asset.json"
"$AUTOPILOTCTL_BIN" --manifest "$SELLER_MANIFEST" --json data-market publish-asset --confirm >"$RUN_DIR/publish-asset.json"
ASSET_ID="$(json_field "$RUN_DIR/publish-asset.json" payload.seller.draft.last_published_asset_id)"

"$AUTOPILOTCTL_BIN" --manifest "$SELLER_MANIFEST" --json data-market draft-grant --file "$RUN_DIR/package/grant-template.json" >"$RUN_DIR/draft-grant.json"
"$AUTOPILOTCTL_BIN" --manifest "$SELLER_MANIFEST" --json data-market preview-grant >"$RUN_DIR/preview-grant.json"
"$AUTOPILOTCTL_BIN" --manifest "$SELLER_MANIFEST" --json data-market publish-grant --confirm >"$RUN_DIR/publish-grant.json"
GRANT_ID="$(json_field "$RUN_DIR/publish-grant.json" payload.seller.draft.last_published_grant_id)"

echo "bringing seller runtime online for targeted request intake"
"$AUTOPILOTCTL_BIN" --manifest "$SELLER_MANIFEST" provider online --wait --timeout-ms "$WAIT_TIMEOUT_MS" >"$RUN_DIR/seller-provider-online.json"
if [[ -n "$RELAY_URLS_CSV" && "$PUBLIC_SUBSCRIPTION_SETTLE_SECONDS" -gt 0 ]]; then
  echo "waiting ${PUBLIC_SUBSCRIPTION_SETTLE_SECONDS}s for public relay subscription settle"
  sleep "$PUBLIC_SUBSCRIPTION_SETTLE_SECONDS"
fi

echo "publishing targeted buyer request"
"$AUTOPILOTCTL_BIN" --manifest "$BUYER_MANIFEST" --json data-market buyer-refresh >"$RUN_DIR/buyer-refresh.json"
"$AUTOPILOTCTL_BIN" --manifest "$BUYER_MANIFEST" --json data-market buyer-publish-request --asset-id "$ASSET_ID" --refresh-market >"$RUN_DIR/buyer-request.json"
REQUEST_ID="$(json_field "$RUN_DIR/buyer-request.json" payload.buyer.last_published_request_id)"

echo "bringing buyer runtime online for result tracking"
"$AUTOPILOTCTL_BIN" --manifest "$BUYER_MANIFEST" provider online --wait --timeout-ms "$WAIT_TIMEOUT_MS" >"$RUN_DIR/buyer-provider-online.json"

echo "waiting for seller to receive the request"
SELLER_REQUEST_WAIT_SECONDS="$REQUEST_TIMEOUT_SECONDS"
EXPECTED_SELLER_DISPOSITION="ready_for_delivery"
if [[ "$PRICE_SATS" -gt 0 ]]; then
  EXPECTED_SELLER_DISPOSITION="ready_for_payment_quote"
fi
if [[ -n "$RELAY_URLS_CSV" ]]; then
  SELLER_REQUEST_WAIT_SECONDS="$LIVE_INGEST_WAIT_SECONDS"
fi
if ! wait_for_seller_request_evaluation "$SELLER_MANIFEST" "$REQUEST_ID" "$RUN_DIR/seller-request-ready.json" "$SELLER_REQUEST_WAIT_SECONDS" "$EXPECTED_SELLER_DISPOSITION"; then
  if [[ "$REQUIRE_LIVE_INGEST" == "true" ]]; then
    echo "seller never observed a live request from configured relays" >&2
    cat "$RUN_DIR/seller-request-ready.json" >&2 || true
    exit 1
  elif [[ -n "$RELAY_URLS_CSV" ]]; then
    echo "seller did not ingest live from public relays; importing request by event id"
    import_seller_request_from_relays "$SELLER_MANIFEST" "$REQUEST_ID" "$RUN_DIR/seller-import-request.json" "${CONFIGURED_RELAY_URLS[@]}"
    wait_for_seller_request_evaluation "$SELLER_MANIFEST" "$REQUEST_ID" "$RUN_DIR/seller-request-ready.json" 30 "$EXPECTED_SELLER_DISPOSITION" || {
      echo "seller never observed the expected request disposition $EXPECTED_SELLER_DISPOSITION" >&2
      cat "$RUN_DIR/seller-request-ready.json" >&2 || true
      exit 1
    }
  else
    echo "seller never observed the expected request disposition $EXPECTED_SELLER_DISPOSITION" >&2
    cat "$RUN_DIR/seller-request-ready.json" >&2 || true
    exit 1
  fi
fi

if [[ "$PRICE_SATS" -gt 0 ]]; then
  echo "requesting priced payment quote"
  "$AUTOPILOTCTL_BIN" --manifest "$SELLER_MANIFEST" --json data-market request-payment \
    --request-id "$REQUEST_ID" >"$RUN_DIR/request-payment.json"

  echo "waiting for seller to observe settled payment"
  wait_for_seller_payment_settled "$SELLER_MANIFEST" "$REQUEST_ID" "$RUN_DIR/seller-payment-settled.json" "$REQUEST_TIMEOUT_SECONDS" || {
    echo "seller never observed the paid settlement" >&2
    cat "$RUN_DIR/seller-payment-settled.json" >&2 || true
    exit 1
  }

  "$AUTOPILOTCTL_BIN" --manifest "$BUYER_MANIFEST" --json data-market buyer-status >"$RUN_DIR/buyer-after-payment.json"
fi

echo "staging local delivery bundle"
mkdir -p "$RUN_DIR/delivery-bundle/payload"
cp -R "$RUN_DIR/source-dataset/." "$RUN_DIR/delivery-bundle/payload/"
cp "$RUN_DIR/package/packaging-manifest.json" "$RUN_DIR/delivery-bundle/packaging-manifest.json"
cp "$RUN_DIR/package/packaging-summary.json" "$RUN_DIR/delivery-bundle/packaging-summary.json"
BUNDLE_SIZE_BYTES="$(
python3 - "$RUN_DIR/delivery-bundle/payload" <<'PY'
import pathlib, sys
root = pathlib.Path(sys.argv[1])
total = 0
for path in root.rglob("*"):
    if path.is_file():
        total += path.stat().st_size
print(total)
PY
)"
python3 - "$RUN_DIR/package/packaging-summary.json" "$RUN_DIR/delivery-args.json" "$RUN_DIR/delivery-bundle" "$BUNDLE_SIZE_BYTES" <<'PY'
import json
import pathlib
import sys

summary = json.loads(pathlib.Path(sys.argv[1]).read_text())
output = pathlib.Path(sys.argv[2])
bundle_root = pathlib.Path(sys.argv[3]).resolve()
bundle_size_bytes = int(sys.argv[4])
payload = {
    "delivery_ref": f"file://{bundle_root / 'payload'}",
    "delivery_digest": summary["content_digest"],
    "manifest_refs": [
        f"file://{bundle_root / 'packaging-manifest.json'}",
        f"file://{bundle_root / 'packaging-summary.json'}",
    ],
    "bundle_size_bytes": bundle_size_bytes,
    "expires_in_hours": 24,
}
output.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
PY

echo "issuing delivery"
"$AUTOPILOTCTL_BIN" --manifest "$SELLER_MANIFEST" --json data-market prepare-delivery --request-id "$REQUEST_ID" --file "$RUN_DIR/delivery-args.json" >"$RUN_DIR/prepare-delivery.json"
"$AUTOPILOTCTL_BIN" --manifest "$SELLER_MANIFEST" --json data-market issue-delivery --request-id "$REQUEST_ID" >"$RUN_DIR/issue-delivery.json"

echo "waiting for buyer result"
BUYER_RESULT_WAIT_SECONDS="$REQUEST_TIMEOUT_SECONDS"
if [[ -n "$RELAY_URLS_CSV" ]]; then
  BUYER_RESULT_WAIT_SECONDS="$LIVE_INGEST_WAIT_SECONDS"
fi
if ! wait_for_buyer_result "$BUYER_MANIFEST" "$REQUEST_ID" "$RUN_DIR/buyer-result.json" "$BUYER_RESULT_WAIT_SECONDS"; then
  if [[ "$REQUIRE_LIVE_INGEST" == "true" ]]; then
    echo "buyer never observed a live delivery result from configured relays" >&2
    cat "$RUN_DIR/buyer-result.json" >&2 || true
    exit 1
  elif [[ -n "$RELAY_URLS_CSV" ]]; then
    echo "buyer did not ingest live from public relays; importing result by event id"
    "$AUTOPILOTCTL_BIN" --manifest "$SELLER_MANIFEST" --json data-market seller-status >"$RUN_DIR/seller-after-delivery.json"
    RESULT_EVENT_ID="$(json_field "$RUN_DIR/seller-after-delivery.json" payload.seller.latest_incoming_request.delivery_result_event_id)"
    if [[ -z "$RESULT_EVENT_ID" ]]; then
      echo "seller delivery result event id was not available for buyer import" >&2
      cat "$RUN_DIR/seller-after-delivery.json" >&2 || true
      exit 1
    fi
    import_buyer_response_from_relays "$BUYER_MANIFEST" "$RESULT_EVENT_ID" "$RUN_DIR/buyer-import-response.json" "${CONFIGURED_RELAY_URLS[@]}"
    wait_for_buyer_result "$BUYER_MANIFEST" "$REQUEST_ID" "$RUN_DIR/buyer-result.json" 30 || {
      echo "buyer never observed the delivery result" >&2
      cat "$RUN_DIR/buyer-result.json" >&2 || true
      exit 1
    }
  else
    echo "buyer never observed the delivery result" >&2
    cat "$RUN_DIR/buyer-result.json" >&2 || true
    exit 1
  fi
fi

echo "consuming delivered data locally"
"$AUTOPILOTCTL_BIN" --manifest "$BUYER_MANIFEST" --json data-market consume-delivery \
  --request-id "$REQUEST_ID" \
  --grant-id "$GRANT_ID" \
  --output-dir "$RUN_DIR/consumed-dataset" \
  --refresh-market \
  --overwrite >"$RUN_DIR/consume-delivery.json"

echo "verifying consumed payload matches source"
python3 - "$RUN_DIR/source-dataset" "$RUN_DIR/consumed-dataset/payload" "$RUN_DIR/consume-delivery.json" "$RUN_DIR/publish-asset.json" "$RUN_DIR/publish-grant.json" "$RUN_DIR/buyer-result.json" "$RUN_DIR/issue-delivery.json" "$RUN_DIR/seller-request-ready.json" "$RUN_DIR/summary.json" "$NORMALIZED_RELAY_URLS_CSV" "$RUN_DIR/seller-import-request.json" "$RUN_DIR/buyer-import-response.json" "$REQUIRE_LIVE_INGEST" "$RUN_DIR/seller-payment-settled.json" "$RUN_DIR/buyer-after-payment.json" "$PRICE_SATS" <<'PY'
import filecmp
import json
import pathlib
import sys

source_root = pathlib.Path(sys.argv[1])
consumed_root = pathlib.Path(sys.argv[2])
consume_json = pathlib.Path(sys.argv[3])
publish_asset_json = pathlib.Path(sys.argv[4])
publish_grant_json = pathlib.Path(sys.argv[5])
buyer_result_json = pathlib.Path(sys.argv[6])
issue_delivery_json = pathlib.Path(sys.argv[7])
seller_request_json = pathlib.Path(sys.argv[8])
summary_path = pathlib.Path(sys.argv[9])
configured_relay_urls = [relay for relay in sys.argv[10].split(",") if relay]
seller_import_path = pathlib.Path(sys.argv[11])
buyer_import_path = pathlib.Path(sys.argv[12])
seller_payment_path = pathlib.Path(sys.argv[14])
buyer_payment_path = pathlib.Path(sys.argv[15])
price_sats = int(sys.argv[16])

if not consumed_root.exists():
    raise SystemExit("consumed payload directory is missing")

def compare_dirs(left: pathlib.Path, right: pathlib.Path):
    cmp = filecmp.dircmp(left, right)
    if cmp.left_only or cmp.right_only or cmp.funny_files:
        raise SystemExit(
            f"directory mismatch left_only={cmp.left_only} right_only={cmp.right_only} funny={cmp.funny_files}"
        )
    _, mismatch, errors = filecmp.cmpfiles(left, right, cmp.common_files, shallow=False)
    if mismatch or errors:
        raise SystemExit(f"file mismatch mismatch={mismatch} errors={errors}")
    for subdir in cmp.common_dirs:
        compare_dirs(left / subdir, right / subdir)

compare_dirs(source_root, consumed_root)

consume_payload = json.loads(consume_json.read_text())
publish_asset_payload = json.loads(publish_asset_json.read_text())
publish_grant_payload = json.loads(publish_grant_json.read_text())
buyer_result_payload = json.loads(buyer_result_json.read_text())
issue_delivery_payload = json.loads(issue_delivery_json.read_text())
seller_request_payload = json.loads(seller_request_json.read_text())
latest_request = buyer_result_payload["payload"]["buyer"]["latest_request"]
seller_latest = seller_request_payload["payload"]["seller"]["latest_incoming_request"]
seller_import = json.loads(seller_import_path.read_text()) if seller_import_path.exists() else None
buyer_import = json.loads(buyer_import_path.read_text()) if buyer_import_path.exists() else None
seller_payment = json.loads(seller_payment_path.read_text()) if seller_payment_path.exists() else None
buyer_payment = json.loads(buyer_payment_path.read_text()) if buyer_payment_path.exists() else None
seller_payment_latest = (
    seller_payment.get("payload", {}).get("seller", {}).get("latest_incoming_request")
    if seller_payment else None
)
buyer_payment_latest = (
    buyer_payment.get("payload", {}).get("buyer", {}).get("latest_request")
    if buyer_payment else None
)

summary = {
    "price_sats": price_sats,
    "asset_id": publish_asset_payload["payload"]["seller"]["draft"]["last_published_asset_id"],
    "grant_id": publish_grant_payload["payload"]["seller"]["draft"]["last_published_grant_id"],
    "request_id": latest_request["request_id"],
    "payment_feedback_event_id": (
        buyer_payment_latest.get("last_feedback_event_id")
        if buyer_payment_latest else latest_request.get("last_feedback_event_id")
    ),
    "buyer_payment_pointer": (
        buyer_payment_latest.get("last_payment_pointer")
        if buyer_payment_latest else latest_request.get("last_payment_pointer")
    ),
    "seller_payment_pointer": (
        ((seller_payment_latest or {}).get("payment") or {}).get("payment_pointer")
    ),
    "result_event_id": latest_request["last_result_event_id"],
    "delivery_bundle_id": issue_delivery_payload["payload"]["seller"]["latest_incoming_request"]["delivery"]["bundle_id"],
    "consumed_payload_path": consume_payload["consumed"]["payload_output_path"],
    "copied_manifest_paths": consume_payload["consumed"]["copied_manifest_paths"],
    "configured_relay_urls": configured_relay_urls,
    "request_kind": seller_import["relay_fetch"]["kind"] if seller_import else 5960,
    "request_source_relay_url": (
        seller_import["relay_fetch"]["relay_url"]
        if seller_import else (
            seller_latest.get("source_relay_url")
            or next(
                (
                    row.get("source_relay_url")
                    for row in seller_request_payload["payload"]["seller"].get("incoming_requests", [])
                    if row.get("request_id") == latest_request["request_id"]
                ),
                None,
            )
        )
    ),
    "seller_request_ingest_mode": "relay_import" if seller_import else "live_relay",
    "result_kind": buyer_import["relay_fetch"]["kind"] if buyer_import else 6960,
    "buyer_result_ingest_mode": "relay_import" if buyer_import else "live_relay",
    "result_relay_urls": (
        latest_request.get("last_result_relay_urls")
        or next(
            (
                observation.get("last_result_relay_urls")
                for observation in latest_request.get("provider_observations", [])
                if observation.get("last_result_event_id") == latest_request["last_result_event_id"]
            ),
            [],
        )
        or []
    ),
    "required_live_ingest": sys.argv[13].lower() == "true",
}
summary_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
print(json.dumps(summary, indent=2, sort_keys=True))
PY

echo
echo "headless data-market E2E completed"
echo "run dir: $RUN_DIR"
