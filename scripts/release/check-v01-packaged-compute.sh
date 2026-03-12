#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

RUN_DIR="${OPENAGENTS_PACKAGED_RUN_DIR:-$ROOT_DIR/target/packaged-compute-smoke}"
APP_HOME="${OPENAGENTS_PACKAGED_APP_HOME:-$RUN_DIR/app-home}"
APP_LOG_DIR="${OPENAGENTS_PACKAGED_APP_LOG_DIR:-$RUN_DIR/app-logs}"
BUYER_HOME="${OPENAGENTS_PACKAGED_BUYER_HOME:-$RUN_DIR/buyer-home}"
BUYER_IDENTITY_PATH="${OPENAGENTS_PACKAGED_BUYER_IDENTITY_PATH:-$BUYER_HOME/.openagents/pylon/identity.mnemonic}"
BUYER_STORAGE_DIR="${OPENAGENTS_PACKAGED_BUYER_STORAGE_DIR:-$BUYER_HOME/.openagents/pylon/spark}"
APP_SETTINGS_PATH="${APP_HOME}/.openagents/autopilot-settings-v1.conf"
APP_IDENTITY_PATH="${APP_HOME}/.openagents/pylon/identity.mnemonic"
APP_BUNDLE_PATH="${OPENAGENTS_PACKAGED_APP_BUNDLE:-$ROOT_DIR/target/release/bundle/osx/Autopilot.app}"
APP_EXECUTABLE="${APP_BUNDLE_PATH}/Contents/MacOS/autopilot-desktop"
APP_BRIDGE_BINARY="${APP_BUNDLE_PATH}/Contents/MacOS/foundation-bridge"
AUTOPILOTCTL_BIN="${OPENAGENTS_PACKAGED_AUTOPILOTCTL_BIN:-$ROOT_DIR/target/release/autopilotctl}"
HEADLESS_BIN="${OPENAGENTS_PACKAGED_HEADLESS_BIN:-$ROOT_DIR/target/release/autopilot-headless-compute}"
SPARK_BIN="${OPENAGENTS_PACKAGED_SPARK_BIN:-$ROOT_DIR/target/release/spark-wallet-cli}"
FUNDER_HOME="${OPENAGENTS_PACKAGED_FUNDER_HOME:-$HOME}"
FUNDER_IDENTITY_PATH="${OPENAGENTS_PACKAGED_FUNDER_IDENTITY_PATH:-}"
BUYER_FUNDING_SATS="${OPENAGENTS_PACKAGED_BUYER_FUNDING_SATS:-50}"
BUDGET_SATS="${OPENAGENTS_PACKAGED_BUDGET_SATS:-2}"
BUYER_TIMEOUT_SECONDS="${OPENAGENTS_PACKAGED_BUYER_TIMEOUT_SECONDS:-90}"
BUYER_INTERVAL_SECONDS="${OPENAGENTS_PACKAGED_BUYER_INTERVAL_SECONDS:-8}"
APP_START_TIMEOUT_SECONDS="${OPENAGENTS_PACKAGED_APP_START_TIMEOUT_SECONDS:-90}"
PROVIDER_ONLINE_TIMEOUT_MS="${OPENAGENTS_PACKAGED_PROVIDER_ONLINE_TIMEOUT_MS:-120000}"
APPLE_FM_READY_TIMEOUT_MS="${OPENAGENTS_PACKAGED_APPLE_FM_READY_TIMEOUT_MS:-120000}"
SKIP_BUILD="${OPENAGENTS_PACKAGED_SKIP_BUILD:-0}"

APP_PID=""
BUYER_PID=""
RELAY_PID=""

log() {
  echo "[check-v01-packaged-compute] $*"
}

die() {
  echo "[check-v01-packaged-compute] ERROR: $*" >&2
  exit 1
}

require_command() {
  local cmd="$1"
  command -v "$cmd" >/dev/null 2>&1 || die "Missing required command: $cmd"
}

ensure_cargo_bundle() {
  if command -v cargo-bundle >/dev/null 2>&1; then
    echo "cargo bundle"
    return
  fi

  local install_root="${ROOT_DIR}/target/release-tools"
  local bundle_bin="${install_root}/bin/cargo-bundle"
  if [[ ! -x "$bundle_bin" ]]; then
    log "Installing cargo-bundle into ${install_root}"
    cargo install cargo-bundle --locked --root "$install_root"
  fi
  [[ -x "$bundle_bin" ]] || die "cargo-bundle was not found at ${bundle_bin}"
  echo "$bundle_bin"
}

resolve_default_spark_api_key() {
  python3 - <<'PY'
import pathlib, re, sys
text = pathlib.Path("apps/autopilot-desktop/src/spark_wallet.rs").read_text()
match = re.search(r'DEFAULT_OPENAGENTS_SPARK_API_KEY: &str = "([^"]+)";', text)
if not match:
    raise SystemExit("failed to locate DEFAULT_OPENAGENTS_SPARK_API_KEY in spark_wallet.rs")
print(match.group(1))
PY
}

json_field() {
  local file="$1"
  local field="$2"
  python3 - "$file" "$field" <<'PY'
import json, pathlib, sys
value = json.loads(pathlib.Path(sys.argv[1]).read_text())
node = value
for key in sys.argv[2].split("."):
    if not key:
        continue
    node = node[key]
if isinstance(node, bool):
    print("true" if node else "false")
else:
    print(node)
PY
}

status_total_sats() {
  local file="$1"
  python3 - "$file" <<'PY'
import json
import pathlib
import sys

payload = json.loads(pathlib.Path(sys.argv[1]).read_text())
print(payload.get("balance", {}).get("totalSats", 0))
PY
}

find_free_port() {
  python3 - <<'PY'
import socket
s = socket.socket()
s.bind(("127.0.0.1", 0))
print(s.getsockname()[1])
s.close()
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

wait_for_jsonl_event() {
  local file="$1"
  local expression="$2"
  local timeout_seconds="$3"
  python3 - "$file" "$expression" "$timeout_seconds" <<'PY'
import json
import pathlib
import sys
import time

path = pathlib.Path(sys.argv[1])
expression = sys.argv[2]
timeout_seconds = int(sys.argv[3])
deadline = time.time() + timeout_seconds

def matches(entry):
    source = entry.get("source")
    domain_event = entry.get("domain_event")
    line = entry.get("line", "")
    if expression == "provider.result_published":
        return domain_event == expression
    if expression == "provider.payment_requested":
        return domain_event == expression
    if expression == "provider.settlement_confirmed":
        return domain_event == expression
    if expression == "mission_control.accepted":
        return source == "mission_control" and "[accepted]" in line
    if expression == "mission_control.running":
        return source == "mission_control" and "[running]" in line
    if expression == "mission_control.delivered":
        return source == "mission_control" and "[delivered]" in line
    raise SystemExit(f"unsupported expression: {expression}")

while time.time() < deadline:
    if path.exists():
        for raw in path.read_text().splitlines():
            raw = raw.strip()
            if not raw:
                continue
            try:
                entry = json.loads(raw)
            except json.JSONDecodeError:
                continue
            if matches(entry):
                sys.exit(0)
    time.sleep(1)

raise SystemExit(f"timed out waiting for {expression} in {path}")
PY
}

capture_buyer_wallet_status() {
  local output_path="$1"
  OPENAGENTS_SPARK_API_KEY="$OPENAGENTS_SPARK_API_KEY" \
  "$SPARK_BIN" \
    --identity-path "$BUYER_IDENTITY_PATH" \
    --storage-dir "$BUYER_STORAGE_DIR" \
    status >"$output_path"
}

wait_for_buyer_wallet_balance() {
  local minimum_sats="$1"
  local timeout_seconds="$2"
  local output_path="$3"
  local deadline=$((SECONDS + timeout_seconds))
  while (( SECONDS < deadline )); do
    capture_buyer_wallet_status "$output_path"
    if [[ "$(status_total_sats "$output_path")" -ge "$minimum_sats" ]]; then
      return 0
    fi
    sleep 2
  done
  return 1
}

cleanup() {
  set +e
  if [[ -n "$BUYER_PID" ]]; then
    kill "$BUYER_PID" 2>/dev/null || true
    wait "$BUYER_PID" 2>/dev/null || true
  fi
  if [[ -n "$APP_PID" ]]; then
    if [[ -f "${APP_LOG_DIR}/desktop-control.json" ]]; then
      "$AUTOPILOTCTL_BIN" --manifest "${APP_LOG_DIR}/desktop-control.json" provider offline --wait --timeout-ms 15000 >/dev/null 2>&1 || true
    fi
    kill "$APP_PID" 2>/dev/null || true
    wait "$APP_PID" 2>/dev/null || true
  fi
  if [[ -n "$RELAY_PID" ]]; then
    kill "$RELAY_PID" 2>/dev/null || true
    wait "$RELAY_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

require_command cargo
require_command python3

export OPENAGENTS_SPARK_API_KEY="${OPENAGENTS_SPARK_API_KEY:-$(resolve_default_spark_api_key)}"

if [[ "$SKIP_BUILD" != "1" ]]; then
  CARGO_BUNDLE_BIN="$(ensure_cargo_bundle)"
  log "Building release binaries"
  cargo build -p autopilot-desktop --release --bin autopilotctl --bin autopilot-headless-compute --bin spark-wallet-cli
  log "Building Foundation Models bridge"
  ./swift/foundation-bridge/build.sh
  log "Bundling Autopilot.app"
  if [[ "$CARGO_BUNDLE_BIN" == "cargo bundle" ]]; then
    (
      cd apps/autopilot-desktop
      cargo bundle --release --bin autopilot-desktop --format osx
    )
  else
    (
      cd apps/autopilot-desktop
      "$CARGO_BUNDLE_BIN" --release --bin autopilot-desktop --format osx
    )
  fi
  mkdir -p "${APP_BUNDLE_PATH}/Contents/MacOS"
  cp bin/foundation-bridge "$APP_BRIDGE_BINARY"
  chmod +x "$APP_BRIDGE_BINARY"
fi

[[ -x "$APP_EXECUTABLE" ]] || die "Missing packaged app executable at ${APP_EXECUTABLE}"
[[ -x "$AUTOPILOTCTL_BIN" ]] || die "Missing autopilotctl binary at ${AUTOPILOTCTL_BIN}"
[[ -x "$HEADLESS_BIN" ]] || die "Missing headless compute binary at ${HEADLESS_BIN}"
[[ -x "$SPARK_BIN" ]] || die "Missing spark-wallet-cli binary at ${SPARK_BIN}"

rm -rf "$RUN_DIR"
mkdir -p "${APP_HOME}/.openagents" "$APP_LOG_DIR" "${BUYER_HOME}/.openagents/pylon"

PORT="$(find_free_port)"
RELAY_URL="ws://127.0.0.1:${PORT}"

cat >"$APP_SETTINGS_PATH" <<EOF
schema_version=2
primary_relay_url=${RELAY_URL}
backup_relay_urls=
identity_path=${APP_IDENTITY_PATH}
wallet_default_send_sats=1000
provider_max_queue_depth=1
reconnect_required=false
EOF

log "Starting deterministic local relay on ${RELAY_URL}"
"$HEADLESS_BIN" relay --listen "127.0.0.1:${PORT}" >"${RUN_DIR}/relay.log" 2>&1 &
RELAY_PID=$!
sleep 2

log "Launching packaged app executable ${APP_EXECUTABLE}"
HOME="$APP_HOME" \
OPENAGENTS_AUTOPILOT_LOG_DIR="$APP_LOG_DIR" \
OPENAGENTS_SPARK_API_KEY="$OPENAGENTS_SPARK_API_KEY" \
"$APP_EXECUTABLE" >"${RUN_DIR}/packaged-app.stdout.log" 2>"${RUN_DIR}/packaged-app.stderr.log" &
APP_PID=$!

MANIFEST_PATH="${APP_LOG_DIR}/desktop-control.json"
wait_for_file "$MANIFEST_PATH" "$APP_START_TIMEOUT_SECONDS" || die "Timed out waiting for desktop control manifest ${MANIFEST_PATH}"
wait_for_file "$APP_IDENTITY_PATH" "$APP_START_TIMEOUT_SECONDS" || die "Timed out waiting for packaged app identity ${APP_IDENTITY_PATH}"

SESSION_LOG_PATH="$(json_field "$MANIFEST_PATH" latest_session_log_path)"
LATEST_LOG_PATH="${APP_LOG_DIR}/latest.jsonl"

log "Refreshing Apple FM inside bundled app"
"$AUTOPILOTCTL_BIN" --manifest "$MANIFEST_PATH" apple-fm refresh --wait --timeout-ms "$APPLE_FM_READY_TIMEOUT_MS" >"${RUN_DIR}/apple-fm-refresh.json"

log "Refreshing wallet inside bundled app"
"$AUTOPILOTCTL_BIN" --manifest "$MANIFEST_PATH" wallet refresh >"${RUN_DIR}/wallet-refresh.json"

log "Deriving packaged provider identity"
"$HEADLESS_BIN" identity --identity-path "$APP_IDENTITY_PATH" >"${RUN_DIR}/provider-identity.json"
PROVIDER_PUBKEY="$(json_field "${RUN_DIR}/provider-identity.json" publicKeyHex)"
PROVIDER_NPUB="$(json_field "${RUN_DIR}/provider-identity.json" npub)"

log "Preparing isolated buyer identity"
"$HEADLESS_BIN" identity --identity-path "$BUYER_IDENTITY_PATH" >"${RUN_DIR}/buyer-identity.json"
BUYER_PUBKEY="$(json_field "${RUN_DIR}/buyer-identity.json" publicKeyHex)"
capture_buyer_wallet_status "${RUN_DIR}/buyer-status-before.json"

log "Funding isolated buyer wallet with ${BUYER_FUNDING_SATS} sats"
"$SPARK_BIN" \
  --identity-path "$BUYER_IDENTITY_PATH" \
  --storage-dir "$BUYER_STORAGE_DIR" \
  bolt11-invoice "$BUYER_FUNDING_SATS" \
  --description "Packaged app buyer funding" >"${RUN_DIR}/buyer-funding-invoice.json"
BUYER_FUNDING_INVOICE="$(json_field "${RUN_DIR}/buyer-funding-invoice.json" invoice)"

if [[ -n "$FUNDER_IDENTITY_PATH" ]]; then
  HOME="$FUNDER_HOME" \
  OPENAGENTS_SPARK_API_KEY="$OPENAGENTS_SPARK_API_KEY" \
  "$SPARK_BIN" \
  --identity-path "$FUNDER_IDENTITY_PATH" \
  pay-invoice "$BUYER_FUNDING_INVOICE" >"${RUN_DIR}/buyer-funding-payment.json"
else
  HOME="$FUNDER_HOME" \
  OPENAGENTS_SPARK_API_KEY="$OPENAGENTS_SPARK_API_KEY" \
  "$SPARK_BIN" \
  pay-invoice "$BUYER_FUNDING_INVOICE" >"${RUN_DIR}/buyer-funding-payment.json"
fi

wait_for_buyer_wallet_balance "$BUYER_FUNDING_SATS" 120 "${RUN_DIR}/buyer-status-funded.json" \
  || die "Buyer wallet funding did not settle to ${BUYER_FUNDING_SATS} sats"

log "Bringing packaged provider online through desktop control"
"$AUTOPILOTCTL_BIN" --manifest "$MANIFEST_PATH" --json provider online --wait --timeout-ms "$PROVIDER_ONLINE_TIMEOUT_MS" >"${RUN_DIR}/provider-online.json"

log "Starting controlled headless buyer targeting packaged provider ${PROVIDER_NPUB}"
OPENAGENTS_SPARK_API_KEY="$OPENAGENTS_SPARK_API_KEY" \
"$HEADLESS_BIN" buyer \
  --relay "$RELAY_URL" \
  --identity-path "$BUYER_IDENTITY_PATH" \
  --budget-sats "$BUDGET_SATS" \
  --timeout-seconds "$BUYER_TIMEOUT_SECONDS" \
  --interval-seconds "$BUYER_INTERVAL_SECONDS" \
  --target-provider-pubkey "$PROVIDER_PUBKEY" \
  --max-settled-requests 1 \
  --fail-fast >"${RUN_DIR}/buyer.log" 2>&1 &
BUYER_PID=$!

if ! wait "$BUYER_PID"; then
  echo "--- relay.log ---"
  cat "${RUN_DIR}/relay.log"
  echo "--- packaged-app.stderr.log ---"
  cat "${RUN_DIR}/packaged-app.stderr.log"
  echo "--- buyer.log ---"
  cat "${RUN_DIR}/buyer.log"
  die "Headless buyer failed"
fi
BUYER_PID=""

log "Asserting bundled app file logs captured the full provider lifecycle"
wait_for_jsonl_event "$LATEST_LOG_PATH" "mission_control.accepted" 60
wait_for_jsonl_event "$LATEST_LOG_PATH" "mission_control.running" 60
wait_for_jsonl_event "$LATEST_LOG_PATH" "mission_control.delivered" 60
wait_for_jsonl_event "$LATEST_LOG_PATH" "provider.result_published" 60
wait_for_jsonl_event "$LATEST_LOG_PATH" "provider.payment_requested" 60
wait_for_jsonl_event "$LATEST_LOG_PATH" "provider.settlement_confirmed" 60
wait_for_jsonl_event "$SESSION_LOG_PATH" "provider.result_published" 60
wait_for_jsonl_event "$SESSION_LOG_PATH" "provider.payment_requested" 60
wait_for_jsonl_event "$SESSION_LOG_PATH" "provider.settlement_confirmed" 60

log "Capturing final control snapshot"
"$AUTOPILOTCTL_BIN" --manifest "$MANIFEST_PATH" --json status >"${RUN_DIR}/final-status.json"

python3 - "$RUN_DIR" "$RELAY_URL" "$PROVIDER_PUBKEY" "$BUYER_PUBKEY" "$LATEST_LOG_PATH" "$SESSION_LOG_PATH" <<'PY'
import json
import pathlib
import sys

run_dir = pathlib.Path(sys.argv[1])
relay_url = sys.argv[2]
provider_pubkey = sys.argv[3]
buyer_pubkey = sys.argv[4]
latest_log = pathlib.Path(sys.argv[5])
session_log = pathlib.Path(sys.argv[6])

provider_online = json.loads((run_dir / "provider-online.json").read_text())
final_status = json.loads((run_dir / "final-status.json").read_text())

buyer_log = (run_dir / "buyer.log").read_text().splitlines()

summary = {
    "relayUrl": relay_url,
    "providerPubkey": provider_pubkey,
    "buyerPubkey": buyer_pubkey,
    "bundle": {
        "providerOnlineMessage": provider_online.get("response", {}).get("message"),
        "finalProviderOnline": final_status.get("snapshot", {}).get("provider", {}).get("online"),
        "finalWalletBalanceSats": final_status.get("snapshot", {}).get("wallet", {}).get("balance_sats"),
        "latestJsonl": str(latest_log),
        "sessionJsonl": str(session_log),
    },
    "buyer": {
        "settledLine": next((line for line in buyer_log if "buyer settled request_id=" in line), None),
        "paymentSettledLine": next((line for line in buyer_log if "buyer payment settled" in line), None),
    },
}

(run_dir / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
(run_dir / "summary.txt").write_text(
    "\n".join([
        f"relay={relay_url}",
        f"provider_pubkey={provider_pubkey}",
        f"buyer_pubkey={buyer_pubkey}",
        f"latest_jsonl={latest_log}",
        f"session_jsonl={session_log}",
        f"buyer_settled={summary['buyer']['settledLine'] or 'missing'}",
        f"buyer_payment_settled={summary['buyer']['paymentSettledLine'] or 'missing'}",
    ]) + "\n"
)
PY

log "Packaged compute verification succeeded"
log "Artifacts: ${RUN_DIR}"
