#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

RUN_DIR="${OPENAGENTS_HEADLESS_RUN_DIR:-$ROOT_DIR/target/headless-compute-smoke}"
PROVIDER_HOME="${OPENAGENTS_HEADLESS_PROVIDER_HOME:-$RUN_DIR/provider}"
PROVIDER_BACKEND="${OPENAGENTS_HEADLESS_PROVIDER_BACKEND:-auto}"
MAX_REQUESTS="${OPENAGENTS_HEADLESS_MAX_REQUESTS:-1}"

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

echo "building autopilot-headless-compute"
cargo build -p autopilot-desktop --bin autopilot-headless-compute

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
if ! "$BIN" buyer \
  --relay "$RELAY_URL" \
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
