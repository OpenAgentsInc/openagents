#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

RUN_DIR="${OPENAGENTS_HEADLESS_RUN_DIR:-$ROOT_DIR/target/headless-compute-smoke}"
PROVIDER_HOME="${OPENAGENTS_HEADLESS_PROVIDER_HOME:-$RUN_DIR/provider}"
PROVIDER_BACKEND="${OPENAGENTS_HEADLESS_PROVIDER_BACKEND:-auto}"
MAX_REQUESTS="${OPENAGENTS_HEADLESS_MAX_REQUESTS:-1}"
BUYER_HOME="${OPENAGENTS_HEADLESS_BUYER_HOME:-$HOME}"
BUDGET_SATS="${OPENAGENTS_HEADLESS_BUDGET_SATS:-2}"
SPARK_NETWORK="${OPENAGENTS_SPARK_NETWORK:-mainnet}"
SPARK_BIN="$ROOT_DIR/target/debug/spark-wallet-cli"

case "$SPARK_NETWORK" in
  mainnet|regtest) ;;
  *)
    echo "OPENAGENTS_SPARK_NETWORK=${SPARK_NETWORK} is unsupported for this script; spark-wallet-cli only supports mainnet or regtest here" >&2
    exit 1
    ;;
esac

mkdir -p "$RUN_DIR" "$PROVIDER_HOME"

PORT="$(
python3 - <<'PY'
import socket
s = socket.socket()
s.bind(("127.0.0.1", 0))
print(s.getsockname()[1])
s.close()
PY
)"
RELAY_URL="ws://127.0.0.1:${PORT}"
BIN="$ROOT_DIR/target/debug/autopilot-headless-compute"

DEFAULT_SPARK_API_KEY="$(
python3 - <<'PY'
import pathlib, re, sys
text = pathlib.Path("apps/autopilot-desktop/src/spark_wallet.rs").read_text()
match = re.search(r'DEFAULT_OPENAGENTS_SPARK_API_KEY: &str = "([^"]+)";', text)
if not match:
    raise SystemExit("failed to locate default OPENAGENTS_SPARK_API_KEY fallback in spark_wallet.rs")
print(match.group(1))
PY
)"

export OPENAGENTS_SPARK_API_KEY="${OPENAGENTS_SPARK_API_KEY:-$DEFAULT_SPARK_API_KEY}"

echo "building autopilot-headless-compute + spark-wallet-cli"
cargo build -p autopilot-desktop --bin autopilot-headless-compute --bin spark-wallet-cli

cleanup() {
  set +e
  if [[ -n "${BUYER_PID:-}" ]]; then
    kill "$BUYER_PID" 2>/dev/null || true
  fi
  if [[ -n "${PROVIDER_PID:-}" ]]; then
    kill "$PROVIDER_PID" 2>/dev/null || true
  fi
  if [[ -n "${RELAY_PID:-}" ]]; then
    kill "$RELAY_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

echo "checking buyer wallet funding on ${SPARK_NETWORK}"
BUYER_STATUS="$(
  HOME="$BUYER_HOME" \
  "$SPARK_BIN" \
    --network "$SPARK_NETWORK" \
    status
)"
BUYER_BALANCE="$(
  python3 - "$BUYER_STATUS" <<'PY'
import json, sys
payload = json.loads(sys.argv[1])
print(payload.get("balance", {}).get("totalSats", 0))
PY
)"
if (( BUYER_BALANCE < BUDGET_SATS )); then
  echo "buyer wallet in HOME=${BUYER_HOME} only has ${BUYER_BALANCE} sats on ${SPARK_NETWORK}; requires at least ${BUDGET_SATS} sats for headless smoke" >&2
  echo "fund that wallet first or point OPENAGENTS_HEADLESS_BUYER_HOME at a funded Spark home" >&2
  exit 1
fi

echo "starting local relay on ${RELAY_URL}"
"$BIN" relay --listen "127.0.0.1:${PORT}" >"$RUN_DIR/relay.log" 2>&1 &
RELAY_PID=$!
sleep 2

echo "starting provider with fresh identity at ${PROVIDER_HOME}/identity.mnemonic"
"$BIN" provider \
  --relay "$RELAY_URL" \
  --identity-path "$PROVIDER_HOME/identity.mnemonic" \
  --backend "$PROVIDER_BACKEND" \
  --max-settled-jobs "$MAX_REQUESTS" \
  >"$RUN_DIR/provider.log" 2>&1 &
PROVIDER_PID=$!
sleep 5

echo "starting buyer against current default wallet"
if ! HOME="$BUYER_HOME" "$BIN" buyer \
  --relay "$RELAY_URL" \
  --budget-sats "$BUDGET_SATS" \
  --max-settled-requests "$MAX_REQUESTS" \
  --fail-fast \
  >"$RUN_DIR/buyer.log" 2>&1; then
  echo
  echo "buyer failed; relay/provider logs follow"
  echo "--- relay.log ---"
  cat "$RUN_DIR/relay.log"
  echo "--- provider.log ---"
  cat "$RUN_DIR/provider.log"
  echo "--- buyer.log ---"
  cat "$RUN_DIR/buyer.log"
  exit 1
fi

wait "$PROVIDER_PID"

echo
echo "headless smoke completed"
echo "relay log:    $RUN_DIR/relay.log"
echo "provider log: $RUN_DIR/provider.log"
echo "buyer log:    $RUN_DIR/buyer.log"
