#!/usr/bin/env bash
set -euo pipefail

# Deterministic local setup for Maestro on macOS
# - Boots iOS Simulator (if not running)
# - Starts oa-bridge on BRIDGE_PORT with a known token
# - Starts Metro on METRO_PORT with auto-connect env
# - Writes scripts/maestro.env and prints quick run commands

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

: "${DEVICE:=iPhone 16}"
: "${BRIDGE_PORT:=8788}"
: "${BRIDGE_TOKEN:=test-maestro-token}"
: "${METRO_PORT:=8083}"
: "${BRIDGE_HOST:=127.0.0.1:${BRIDGE_PORT}}"

log() { echo "[prepare] $*"; }

# 1) Boot iOS Simulator
if ! xcrun simctl list devices booted | grep -q "$DEVICE"; then
  log "Booting Simulator: $DEVICE"
  open -a Simulator || true
  xcrun simctl boot "$DEVICE" || true
  # wait a bit for boot
  sleep 5
fi

# 2) Start oa-bridge if not already listening
if ! lsof -iTCP:${BRIDGE_PORT} -sTCP:LISTEN >/dev/null 2>&1; then
  log "Starting oa-bridge on 0.0.0.0:${BRIDGE_PORT}"
  nohup env OPENAGENTS_BRIDGE_TOKEN="${BRIDGE_TOKEN}" cargo run -q -p oa-bridge -- --bind 0.0.0.0:${BRIDGE_PORT} >/tmp/oa-bridge.out 2>&1 &
  sleep 1
fi

# 3) Start Metro on specified port
if ! lsof -iTCP:${METRO_PORT} -sTCP:LISTEN >/dev/null 2>&1; then
  log "Starting Metro on port ${METRO_PORT}"
  ( cd expo && nohup env EXPO_PUBLIC_ENV=development EXPO_PUBLIC_AUTO_CONNECT=1 EXPO_PUBLIC_BRIDGE_HOST="${BRIDGE_HOST}" EXPO_PUBLIC_BRIDGE_TOKEN="${BRIDGE_TOKEN}" bunx expo start --port ${METRO_PORT} >/tmp/metro.out 2>&1 & )
  sleep 3
fi

# 4) Write Maestro env file
mkdir -p scripts
cat > scripts/maestro.env <<ENV
BRIDGE_HOST=${BRIDGE_HOST}
BRIDGE_TOKEN=${BRIDGE_TOKEN}
EXP_URL=exp://localhost:${METRO_PORT}
ENV

log "Env written to scripts/maestro.env"
EXP_URL="exp://localhost:${METRO_PORT}"
# 5) Warm key routes in Simulator
log "Warming routes in Simulator"
xcrun simctl openurl booted "${EXP_URL}" || true
sleep 1
xcrun simctl openurl booted "${EXP_URL}/--/thread/new" || true
sleep 1
xcrun simctl openurl booted "${EXP_URL}/--/settings" || true
log "Run stable:   MAESTRO_ENV_FILE=scripts/maestro.env scripts/maestro-run-stable.sh"
log "Run all:      MAESTRO_ENV_FILE=scripts/maestro.env scripts/maestro-run-all.sh"
