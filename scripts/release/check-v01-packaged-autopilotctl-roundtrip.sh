#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

RUN_DIR="${OPENAGENTS_AUTOPILOTCTL_RUN_DIR:-$ROOT_DIR/target/packaged-autopilotctl-roundtrip}"
BUNDLE_HOME="${OPENAGENTS_AUTOPILOTCTL_BUNDLE_HOME:-$RUN_DIR/bundle-home}"
BUNDLE_LOG_DIR="${OPENAGENTS_AUTOPILOTCTL_BUNDLE_LOG_DIR:-$RUN_DIR/bundle-logs}"
RUNTIME_HOME="${OPENAGENTS_AUTOPILOTCTL_RUNTIME_HOME:-$RUN_DIR/runtime-home}"
RUNTIME_LOG_DIR="${OPENAGENTS_AUTOPILOTCTL_RUNTIME_LOG_DIR:-$RUN_DIR/runtime-logs}"
SEED_HOME="${OPENAGENTS_AUTOPILOTCTL_SEED_HOME:-$RUN_DIR/seed-home}"
BUNDLE_IDENTITY_PATH="${OPENAGENTS_AUTOPILOTCTL_BUNDLE_IDENTITY_PATH:-$BUNDLE_HOME/.openagents/pylon/identity.mnemonic}"
BUNDLE_STORAGE_DIR="${OPENAGENTS_AUTOPILOTCTL_BUNDLE_STORAGE_DIR:-$BUNDLE_HOME/.openagents/pylon/spark}"
RUNTIME_IDENTITY_PATH="${OPENAGENTS_AUTOPILOTCTL_RUNTIME_IDENTITY_PATH:-$RUNTIME_HOME/.openagents/pylon/identity.mnemonic}"
RUNTIME_STORAGE_DIR="${OPENAGENTS_AUTOPILOTCTL_RUNTIME_STORAGE_DIR:-$RUNTIME_HOME/.openagents/pylon/spark}"
SEED_IDENTITY_PATH="${OPENAGENTS_AUTOPILOTCTL_SEED_IDENTITY_PATH:-$SEED_HOME/.openagents/pylon/identity.mnemonic}"
BUNDLE_SETTINGS_PATH="${BUNDLE_HOME}/.openagents/autopilot-settings-v1.conf"
RUNTIME_SETTINGS_PATH="${RUNTIME_HOME}/.openagents/autopilot-settings-v1.conf"
APP_BUNDLE_PATH="${OPENAGENTS_AUTOPILOTCTL_APP_BUNDLE:-$ROOT_DIR/target/release/bundle/osx/Autopilot.app}"
BUNDLE_EXECUTABLE="${APP_BUNDLE_PATH}/Contents/MacOS/autopilot-desktop"
RUNTIME_EXECUTABLE="${OPENAGENTS_AUTOPILOTCTL_RUNTIME_EXECUTABLE:-$ROOT_DIR/target/release/autopilot-desktop}"
BRIDGE_APP="${APP_BUNDLE_PATH}/Contents/Helpers/FoundationBridge.app"
AUTOPILOTCTL_BIN="${OPENAGENTS_AUTOPILOTCTL_BIN:-$ROOT_DIR/target/release/autopilotctl}"
HEADLESS_BIN="${OPENAGENTS_AUTOPILOTCTL_HEADLESS_BIN:-$ROOT_DIR/target/release/autopilot-headless-compute}"
SPARK_BIN="${OPENAGENTS_AUTOPILOTCTL_SPARK_BIN:-$ROOT_DIR/target/release/spark-wallet-cli}"
FUNDER_HOME="${OPENAGENTS_AUTOPILOTCTL_FUNDER_HOME:-$HOME}"
FUNDER_IDENTITY_PATH="${OPENAGENTS_AUTOPILOTCTL_FUNDER_IDENTITY_PATH:-}"
FUND_SATS="${OPENAGENTS_AUTOPILOTCTL_FUND_SATS:-100}"
BUDGET_SATS="${OPENAGENTS_AUTOPILOTCTL_BUDGET_SATS:-2}"
SPARK_NETWORK="${OPENAGENTS_SPARK_NETWORK:-mainnet}"
MIN_SETTLED_BUYER_SATS="${OPENAGENTS_AUTOPILOTCTL_MIN_SETTLED_BUYER_SATS:-$(( BUDGET_SATS * 2 ))}"
APP_START_TIMEOUT_SECONDS="${OPENAGENTS_AUTOPILOTCTL_APP_START_TIMEOUT_SECONDS:-90}"
WAIT_TIMEOUT_MS="${OPENAGENTS_AUTOPILOTCTL_WAIT_TIMEOUT_MS:-120000}"
BUY_TIMEOUT_MS="${OPENAGENTS_AUTOPILOTCTL_BUY_TIMEOUT_MS:-240000}"
SKIP_BUILD="${OPENAGENTS_AUTOPILOTCTL_SKIP_BUILD:-0}"
SKIP_TEST_GATES="${OPENAGENTS_AUTOPILOTCTL_SKIP_TEST_GATES:-0}"

BRIDGE_STARTED_BY_SCRIPT=0
RELAY_PID=""
BUNDLE_PID=""
RUNTIME_PID=""

case "$SPARK_NETWORK" in
  mainnet|regtest) ;;
  *)
    echo "[check-v01-packaged-autopilotctl-roundtrip] ERROR: OPENAGENTS_SPARK_NETWORK=${SPARK_NETWORK} is unsupported for this script; spark-wallet-cli only supports mainnet or regtest here" >&2
    exit 1
    ;;
esac

log() {
  echo "[check-v01-packaged-autopilotctl-roundtrip] $*"
}

die() {
  echo "[check-v01-packaged-autopilotctl-roundtrip] ERROR: $*" >&2
  exit 1
}

require_command() {
  local cmd="$1"
  command -v "$cmd" >/dev/null 2>&1 || die "Missing required command: $cmd"
}

run_release_regression_gates() {
  log "Running deterministic autopilot release gates"
  cargo test -p autopilot-desktop --lib \
    app_state::tests::restart_preserves_earnings_scoreboard_from_persisted_receipts \
    -- --exact
  cargo test -p autopilot-desktop --lib \
    spark_wallet::tests::startup_convergence_refresh_due_after_interval \
    -- --exact
  cargo test -p autopilot-desktop --lib \
    spark_wallet::tests::startup_convergence_status_reports_reconciling_until_followups_finish \
    -- --exact
  cargo test -p autopilot-desktop --lib \
    input::reducers::jobs::tests::auto_accept_policy_skips_expired_targeted_request_even_if_target_matches \
    -- --exact
  cargo test -p autopilot-desktop --lib \
    app_state::tests::network_requests_buyer_payment_watchdog_stays_active_after_seller_settlement_feedback \
    -- --exact
  cargo test -p autopilot-desktop --lib \
    nip90_compute_flow::tests::buyer_snapshot_distinguishes_seller_settlement_from_local_wallet_confirmation \
    -- --exact
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
import pathlib, re
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
import json, pathlib, re, sys
text = pathlib.Path(sys.argv[1]).read_text()
text = re.sub(r"\x1b\[[0-9;]*m", "", text)
decoder = json.JSONDecoder()
value = None
for index, ch in enumerate(text):
    if ch not in "{[":
        continue
    try:
        value, _ = decoder.raw_decode(text[index:])
        break
    except json.JSONDecodeError:
        continue
if value is None:
    raise SystemExit(f"failed to decode JSON payload from {sys.argv[1]}")
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
import json, pathlib, re, sys
text = pathlib.Path(sys.argv[1]).read_text()
text = re.sub(r"\x1b\[[0-9;]*m", "", text)
decoder = json.JSONDecoder()
payload = None
for index, ch in enumerate(text):
    if ch not in "{[":
        continue
    try:
        payload, _ = decoder.raw_decode(text[index:])
        break
    except json.JSONDecodeError:
        continue
if payload is None:
    raise SystemExit(f"failed to decode JSON payload from {sys.argv[1]}")
print(payload.get("balance", {}).get("totalSats", 0))
PY
}

json_tail_contains_content() {
  local expected_content="$1"
  local raw_output="$2"
  python3 - "$expected_content" "$raw_output" <<'PY'
import json, re, sys

expected = sys.argv[1]
text = sys.argv[2]
text = re.sub(r"\x1b\[[0-9;]*m", "", text)
decoder = json.JSONDecoder()
payload = None
for index, ch in enumerate(text):
    if ch not in "{[":
        continue
    try:
        payload, _ = decoder.raw_decode(text[index:])
        break
    except json.JSONDecodeError:
        continue
if payload is None:
    sys.exit(1)
if isinstance(payload, dict):
    messages = payload.get("messages", [])
else:
    messages = payload
for entry in messages:
    if isinstance(entry, dict) and entry.get("content") == expected:
        sys.exit(0)
sys.exit(1)
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

wait_for_http_health() {
  local timeout_seconds="$1"
  local deadline=$((SECONDS + timeout_seconds))
  while (( SECONDS < deadline )); do
    if curl -sf http://127.0.0.1:11435/health >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  return 1
}

write_settings() {
  local path="$1"
  local relay_url="$2"
  local identity_path="$3"
  mkdir -p "$(dirname "$path")"
  cat >"$path" <<EOF
schema_version=2
primary_relay_url=${relay_url}
backup_relay_urls=
identity_path=${identity_path}
wallet_default_send_sats=1000
provider_max_queue_depth=1
reconnect_required=false
EOF
}

capture_wallet_status() {
  local identity_path="$1"
  local storage_dir="$2"
  local output_path="$3"
  OPENAGENTS_SPARK_API_KEY="$OPENAGENTS_SPARK_API_KEY" \
  "$SPARK_BIN" \
    --network "$SPARK_NETWORK" \
    --identity-path "$identity_path" \
    --storage-dir "$storage_dir" \
    status >"$output_path"
}

capture_funder_wallet_status() {
  local output_path="$1"
  if [[ -n "$FUNDER_IDENTITY_PATH" ]]; then
    HOME="$FUNDER_HOME" \
    OPENAGENTS_SPARK_API_KEY="$OPENAGENTS_SPARK_API_KEY" \
    "$SPARK_BIN" \
      --network "$SPARK_NETWORK" \
      --identity-path "$FUNDER_IDENTITY_PATH" \
      status >"$output_path"
  else
    HOME="$FUNDER_HOME" \
    OPENAGENTS_SPARK_API_KEY="$OPENAGENTS_SPARK_API_KEY" \
    "$SPARK_BIN" \
      --network "$SPARK_NETWORK" \
      status >"$output_path"
  fi
}

wait_for_wallet_balance() {
  local identity_path="$1"
  local storage_dir="$2"
  local minimum_sats="$3"
  local timeout_seconds="$4"
  local output_path="$5"
  local deadline=$((SECONDS + timeout_seconds))
  while (( SECONDS < deadline )); do
    capture_wallet_status "$identity_path" "$storage_dir" "$output_path"
    if [[ "$(status_total_sats "$output_path")" -ge "$minimum_sats" ]]; then
      return 0
    fi
    sleep 2
  done
  return 1
}

wait_for_manifest_wallet_balance() {
  local manifest="$1"
  local minimum_sats="$2"
  local timeout_seconds="$3"
  local output_path="$4"
  local deadline=$((SECONDS + timeout_seconds))
  local next_refresh_at=$SECONDS
  while (( SECONDS < deadline )); do
    if (( SECONDS >= next_refresh_at )); then
      "$AUTOPILOTCTL_BIN" --manifest "$manifest" wallet refresh >/dev/null
      next_refresh_at=$((SECONDS + 5))
    fi
    "$AUTOPILOTCTL_BIN" --manifest "$manifest" --json status >"$output_path"
    local balance_sats
    balance_sats="$(json_field "$output_path" snapshot.wallet.balance_sats)"
    local wallet_error
    wallet_error="$(python3 - "$output_path" <<'PY'
import json, pathlib, re, sys
text = pathlib.Path(sys.argv[1]).read_text()
text = re.sub(r"\x1b\[[0-9;]*m", "", text)
payload = json.loads(text)
value = payload.get("snapshot", {}).get("wallet", {}).get("last_error")
print("" if value is None else str(value))
PY
)"
    if [[ "$balance_sats" -ge "$minimum_sats" && -z "$wallet_error" ]]; then
      return 0
    fi
    sleep 2
  done
  return 1
}

wait_for_manifest_wallet_balance_sync() {
  local manifest="$1"
  local identity_path="$2"
  local storage_dir="$3"
  local timeout_seconds="$4"
  local manifest_output_path="$5"
  local wallet_status_path="$6"

  capture_wallet_status "$identity_path" "$storage_dir" "$wallet_status_path"
  local expected_sats
  expected_sats="$(status_total_sats "$wallet_status_path")"
  wait_for_manifest_wallet_balance \
    "$manifest" \
    "$expected_sats" \
    "$timeout_seconds" \
    "$manifest_output_path"
}

manifest_wallet_balance() {
  local manifest="$1"
  local output_path="$2"
  "$AUTOPILOTCTL_BIN" --manifest "$manifest" --json status >"$output_path"
  json_field "$output_path" snapshot.wallet.balance_sats
}

wait_for_manifest_wallet_balance_increase() {
  local manifest="$1"
  local previous_sats="$2"
  local timeout_seconds="$3"
  local output_path="$4"
  local deadline=$((SECONDS + timeout_seconds))
  local next_refresh_at=$SECONDS
  while (( SECONDS < deadline )); do
    if (( SECONDS >= next_refresh_at )); then
      "$AUTOPILOTCTL_BIN" --manifest "$manifest" wallet refresh >/dev/null
      next_refresh_at=$((SECONDS + 10))
    fi
    "$AUTOPILOTCTL_BIN" --manifest "$manifest" --json status >"$output_path"
    local balance_sats
    balance_sats="$(json_field "$output_path" snapshot.wallet.balance_sats)"
    local wallet_error
    wallet_error="$(python3 - "$output_path" <<'PY'
import json, pathlib, re, sys
text = pathlib.Path(sys.argv[1]).read_text()
text = re.sub(r"\x1b\[[0-9;]*m", "", text)
payload = json.loads(text)
value = payload.get("snapshot", {}).get("wallet", {}).get("last_error")
print("" if value is None else str(value))
PY
)"
    if [[ "$balance_sats" -gt "$previous_sats" && -z "$wallet_error" ]]; then
      return 0
    fi
    sleep 2
  done
  return 1
}

pay_invoice_from_funder() {
  local invoice="$1"
  local output_path="$2"
  if [[ -n "$FUNDER_IDENTITY_PATH" ]]; then
    HOME="$FUNDER_HOME" \
    OPENAGENTS_SPARK_API_KEY="$OPENAGENTS_SPARK_API_KEY" \
    "$SPARK_BIN" \
      --network "$SPARK_NETWORK" \
      --identity-path "$FUNDER_IDENTITY_PATH" \
      pay-invoice "$invoice" >"$output_path"
  else
    HOME="$FUNDER_HOME" \
    OPENAGENTS_SPARK_API_KEY="$OPENAGENTS_SPARK_API_KEY" \
    "$SPARK_BIN" \
      --network "$SPARK_NETWORK" \
      pay-invoice "$invoice" >"$output_path"
  fi
}

fund_wallet() {
  local label="$1"
  local identity_path="$2"
  local storage_dir="$3"
  local invoice_path="$4"
  local payment_path="$5"
  local status_path="$6"
  log "Funding ${label} wallet with ${FUND_SATS} sats"
  OPENAGENTS_SPARK_API_KEY="$OPENAGENTS_SPARK_API_KEY" \
  "$SPARK_BIN" \
    --network "$SPARK_NETWORK" \
    --identity-path "$identity_path" \
    --storage-dir "$storage_dir" \
    bolt11-invoice "$FUND_SATS" \
    --description "Autopilotctl roundtrip ${label} funding" >"$invoice_path"
  local invoice
  invoice="$(json_field "$invoice_path" invoice)"
  pay_invoice_from_funder "$invoice" "$payment_path"
  wait_for_wallet_balance "$identity_path" "$storage_dir" "$FUND_SATS" 120 "$status_path" \
    || die "${label} wallet funding did not settle to ${FUND_SATS} sats"
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
    domain_event = entry.get("domain_event")
    line = entry.get("line", "")
    if expression in {"provider.result_published", "provider.payment_requested", "provider.settlement_confirmed"}:
        return domain_event == expression
    if expression == "mission_control.accepted":
        return entry.get("source") == "mission_control" and "[accepted]" in line
    if expression == "mission_control.running":
        return entry.get("source") == "mission_control" and "[running]" in line
    if expression == "mission_control.delivered":
        return entry.get("source") == "mission_control" and "[delivered]" in line
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

wait_for_chat_message() {
  local manifest="$1"
  local expected_content="$2"
  local timeout_seconds="$3"
  local deadline=$((SECONDS + timeout_seconds))
  while (( SECONDS < deadline )); do
    local output
    output="$("$AUTOPILOTCTL_BIN" --manifest "$manifest" --json chat tail --limit 20)"
    if json_tail_contains_content "$expected_content" "$output"; then
      return 0
    fi
    sleep 1
  done
  return 1
}

assert_selected_target() {
  local manifest="$1"
  local expected_pubkey="$2"
  local output_path="$3"
  "$AUTOPILOTCTL_BIN" --manifest "$manifest" --json buy-mode target >"$output_path"
  local selected
  selected="$(json_field "$output_path" selected_peer_pubkey)"
  [[ "$selected" == "$expected_pubkey" ]] \
    || die "Expected target ${expected_pubkey} but control plane selected ${selected}"
}

run_buy_cycle() {
  local buyer_label="$1"
  local buyer_manifest="$2"
  local seller_label="$3"
  local seller_manifest="$4"
  local seller_identity_path="$5"
  local seller_storage_dir="$6"
  local seller_latest_log="$7"
  local seller_session_log="$8"
  local seller_expect_settlement="$9"
  local cycle_slug="${10}"

  local seller_before_path="${RUN_DIR}/${cycle_slug}-${seller_label}-before-status.json"
  local seller_before_wallet_path="${RUN_DIR}/${cycle_slug}-${seller_label}-before-wallet-status.json"
  "$AUTOPILOTCTL_BIN" --manifest "$seller_manifest" wallet refresh >/dev/null
  wait_for_manifest_wallet_balance_sync \
    "$seller_manifest" \
    "$seller_identity_path" \
    "$seller_storage_dir" \
    120 \
    "$seller_before_path" \
    "$seller_before_wallet_path" \
    || die "${seller_label} app wallet did not converge before ${cycle_slug}"
  local seller_after_path="${RUN_DIR}/${cycle_slug}-${seller_label}-after-status.json"
  local seller_before_sats
  seller_before_sats="$(manifest_wallet_balance "$seller_manifest" "$seller_before_path")"

  log "Starting buy mode on ${buyer_label}"
  "$AUTOPILOTCTL_BIN" --manifest "$buyer_manifest" buy-mode start --approved-budget-sats "$BUDGET_SATS" > /dev/null
  "$AUTOPILOTCTL_BIN" --manifest "$buyer_manifest" wait buy-mode-in-flight --timeout-ms "$WAIT_TIMEOUT_MS" > /dev/null
  "$AUTOPILOTCTL_BIN" --manifest "$buyer_manifest" buy-mode stop --wait --timeout-ms "$WAIT_TIMEOUT_MS" > /dev/null
  "$AUTOPILOTCTL_BIN" --manifest "$buyer_manifest" wait buy-mode-paid --timeout-ms "$BUY_TIMEOUT_MS" > /dev/null
  wait_for_jsonl_event "$seller_latest_log" "provider.payment_requested" 120
  wait_for_jsonl_event "$seller_latest_log" "provider.settlement_confirmed" "$seller_expect_settlement"
  wait_for_jsonl_event "$seller_session_log" "provider.settlement_confirmed" "$seller_expect_settlement"
  wait_for_manifest_wallet_balance_increase \
    "$seller_manifest" \
    "$seller_before_sats" \
    "$seller_expect_settlement" \
    "$seller_after_path" \
    || die "${seller_label} wallet balance did not increase after settlement-confirmed for ${cycle_slug} (before=${seller_before_sats})"
}

cleanup() {
  set +e
  if [[ -n "$BUNDLE_PID" && -f "${BUNDLE_LOG_DIR}/desktop-control.json" ]]; then
    "$AUTOPILOTCTL_BIN" --manifest "${BUNDLE_LOG_DIR}/desktop-control.json" provider offline --timeout-ms 15000 >/dev/null 2>&1 || true
  fi
  if [[ -n "$RUNTIME_PID" && -f "${RUNTIME_LOG_DIR}/desktop-control.json" ]]; then
    "$AUTOPILOTCTL_BIN" --manifest "${RUNTIME_LOG_DIR}/desktop-control.json" provider offline --timeout-ms 15000 >/dev/null 2>&1 || true
  fi
  if [[ -n "$BUNDLE_PID" ]]; then
    kill "$BUNDLE_PID" 2>/dev/null || true
    wait "$BUNDLE_PID" 2>/dev/null || true
  fi
  if [[ -n "$RUNTIME_PID" ]]; then
    kill "$RUNTIME_PID" 2>/dev/null || true
    wait "$RUNTIME_PID" 2>/dev/null || true
  fi
  if [[ -n "$RELAY_PID" ]]; then
    kill "$RELAY_PID" 2>/dev/null || true
    wait "$RELAY_PID" 2>/dev/null || true
  fi
  if [[ "$BRIDGE_STARTED_BY_SCRIPT" == "1" ]]; then
    curl -sf -X POST http://127.0.0.1:11435/control/shutdown >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

require_command cargo
require_command curl
require_command python3

if [[ "$SKIP_TEST_GATES" != "1" ]]; then
  run_release_regression_gates
fi

if [[ -f "$ROOT_DIR/.env.local" ]]; then
  # shellcheck disable=SC1091
  set -a
  source "$ROOT_DIR/.env.local"
  set +a
fi

export OPENAGENTS_SPARK_API_KEY="${OPENAGENTS_SPARK_API_KEY:-$(resolve_default_spark_api_key)}"

if [[ "$SKIP_BUILD" != "1" ]]; then
  CARGO_BUNDLE_BIN="$(ensure_cargo_bundle)"
  log "Building release binaries"
  cargo build -p autopilot-desktop --release --bin autopilot-desktop --bin autopilotctl --bin autopilot-headless-compute --bin spark-wallet-cli
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
  mkdir -p "${APP_BUNDLE_PATH}/Contents/Helpers"
  rm -rf "$BRIDGE_APP"
  cp -R bin/FoundationBridge.app "$BRIDGE_APP"
fi

[[ -x "$BUNDLE_EXECUTABLE" ]] || die "Missing bundled app executable at ${BUNDLE_EXECUTABLE}"
[[ -x "$RUNTIME_EXECUTABLE" ]] || die "Missing runtime executable at ${RUNTIME_EXECUTABLE}"
[[ -x "$AUTOPILOTCTL_BIN" ]] || die "Missing autopilotctl binary at ${AUTOPILOTCTL_BIN}"
[[ -x "$HEADLESS_BIN" ]] || die "Missing headless binary at ${HEADLESS_BIN}"
[[ -x "$SPARK_BIN" ]] || die "Missing spark-wallet-cli binary at ${SPARK_BIN}"

rm -rf "$RUN_DIR"
mkdir -p \
  "${BUNDLE_HOME}/.openagents/pylon" \
  "${RUNTIME_HOME}/.openagents/pylon" \
  "${SEED_HOME}/.openagents/pylon" \
  "$BUNDLE_LOG_DIR" \
  "$RUNTIME_LOG_DIR"

capture_funder_wallet_status "${RUN_DIR}/funder-status-before.json"
FUNDER_BALANCE="$(status_total_sats "${RUN_DIR}/funder-status-before.json")"
REQUIRED_FUNDER_SATS=$((FUND_SATS * 2))
if (( FUNDER_BALANCE < REQUIRED_FUNDER_SATS )); then
  die "Funder wallet in HOME=${FUNDER_HOME} only has ${FUNDER_BALANCE} sats on ${SPARK_NETWORK}; requires at least ${REQUIRED_FUNDER_SATS} sats to seed the bundle and runtime wallets. Set OPENAGENTS_AUTOPILOTCTL_FUNDER_HOME and optionally OPENAGENTS_AUTOPILOTCTL_FUNDER_IDENTITY_PATH to a funded Spark wallet."
fi

if ! curl -sf http://127.0.0.1:11435/health >/dev/null 2>&1; then
  log "Starting Foundation Models bridge"
  open -n -g "$BRIDGE_APP" --args 11435 >/dev/null 2>&1
  BRIDGE_STARTED_BY_SCRIPT=1
fi
wait_for_http_health 30 || die "Apple FM bridge did not become healthy on 127.0.0.1:11435"

PORT="$(find_free_port)"
RELAY_URL="ws://127.0.0.1:${PORT}"
write_settings "$BUNDLE_SETTINGS_PATH" "$RELAY_URL" "$BUNDLE_IDENTITY_PATH"
write_settings "$RUNTIME_SETTINGS_PATH" "$RELAY_URL" "$RUNTIME_IDENTITY_PATH"

log "Starting deterministic local relay on ${RELAY_URL}"
"$HEADLESS_BIN" relay --listen "127.0.0.1:${PORT}" >"${RUN_DIR}/relay.log" 2>&1 &
RELAY_PID=$!
sleep 2

log "Seeding managed NIP-28 main channel"
"$HEADLESS_BIN" seed-nip28-main --relay "$RELAY_URL" --identity-path "$SEED_IDENTITY_PATH" >"${RUN_DIR}/seed.json"
CHANNEL_ID="$(json_field "${RUN_DIR}/seed.json" channelId)"

log "Launching bundled app"
HOME="$BUNDLE_HOME" \
OPENAGENTS_AUTOPILOT_LOG_DIR="$BUNDLE_LOG_DIR" \
OPENAGENTS_SPARK_API_KEY="$OPENAGENTS_SPARK_API_KEY" \
OA_DEFAULT_NIP28_RELAY_URL="$RELAY_URL" \
OA_DEFAULT_NIP28_CHANNEL_ID="$CHANNEL_ID" \
"$BUNDLE_EXECUTABLE" >"${RUN_DIR}/bundle.stdout.log" 2>"${RUN_DIR}/bundle.stderr.log" &
BUNDLE_PID=$!

log "Launching second runtime app"
HOME="$RUNTIME_HOME" \
OPENAGENTS_AUTOPILOT_LOG_DIR="$RUNTIME_LOG_DIR" \
OPENAGENTS_SPARK_API_KEY="$OPENAGENTS_SPARK_API_KEY" \
OA_DEFAULT_NIP28_RELAY_URL="$RELAY_URL" \
OA_DEFAULT_NIP28_CHANNEL_ID="$CHANNEL_ID" \
"$RUNTIME_EXECUTABLE" >"${RUN_DIR}/runtime.stdout.log" 2>"${RUN_DIR}/runtime.stderr.log" &
RUNTIME_PID=$!

BUNDLE_MANIFEST="${BUNDLE_LOG_DIR}/desktop-control.json"
RUNTIME_MANIFEST="${RUNTIME_LOG_DIR}/desktop-control.json"
wait_for_file "$BUNDLE_MANIFEST" "$APP_START_TIMEOUT_SECONDS" || die "Timed out waiting for bundled manifest"
wait_for_file "$RUNTIME_MANIFEST" "$APP_START_TIMEOUT_SECONDS" || die "Timed out waiting for runtime manifest"
wait_for_file "$BUNDLE_IDENTITY_PATH" "$APP_START_TIMEOUT_SECONDS" || die "Timed out waiting for bundled identity"
wait_for_file "$RUNTIME_IDENTITY_PATH" "$APP_START_TIMEOUT_SECONDS" || die "Timed out waiting for runtime identity"

BUNDLE_SESSION_LOG="$(json_field "$BUNDLE_MANIFEST" latest_session_log_path)"
RUNTIME_SESSION_LOG="$(json_field "$RUNTIME_MANIFEST" latest_session_log_path)"
BUNDLE_LATEST_LOG="${BUNDLE_LOG_DIR}/latest.jsonl"
RUNTIME_LATEST_LOG="${RUNTIME_LOG_DIR}/latest.jsonl"

log "Refreshing Apple FM and wallet in both apps"
"$AUTOPILOTCTL_BIN" --manifest "$BUNDLE_MANIFEST" apple-fm refresh --wait --timeout-ms "$WAIT_TIMEOUT_MS" >"${RUN_DIR}/bundle-apple-fm.json"
"$AUTOPILOTCTL_BIN" --manifest "$RUNTIME_MANIFEST" apple-fm refresh --wait --timeout-ms "$WAIT_TIMEOUT_MS" >"${RUN_DIR}/runtime-apple-fm.json"
"$AUTOPILOTCTL_BIN" --manifest "$BUNDLE_MANIFEST" wallet refresh >"${RUN_DIR}/bundle-wallet-refresh.json"
"$AUTOPILOTCTL_BIN" --manifest "$RUNTIME_MANIFEST" wallet refresh >"${RUN_DIR}/runtime-wallet-refresh.json"

log "Capturing app identities"
"$HEADLESS_BIN" identity --identity-path "$BUNDLE_IDENTITY_PATH" >"${RUN_DIR}/bundle-identity.json"
"$HEADLESS_BIN" identity --identity-path "$RUNTIME_IDENTITY_PATH" >"${RUN_DIR}/runtime-identity.json"
BUNDLE_PUBKEY="$(json_field "${RUN_DIR}/bundle-identity.json" publicKeyHex)"
RUNTIME_PUBKEY="$(json_field "${RUN_DIR}/runtime-identity.json" publicKeyHex)"

fund_wallet "bundle" "$BUNDLE_IDENTITY_PATH" "$BUNDLE_STORAGE_DIR" "${RUN_DIR}/bundle-funding-invoice.json" "${RUN_DIR}/bundle-funding-payment.json" "${RUN_DIR}/bundle-status-funded.json"
fund_wallet "runtime" "$RUNTIME_IDENTITY_PATH" "$RUNTIME_STORAGE_DIR" "${RUN_DIR}/runtime-funding-invoice.json" "${RUN_DIR}/runtime-funding-payment.json" "${RUN_DIR}/runtime-status-funded.json"
"$AUTOPILOTCTL_BIN" --manifest "$BUNDLE_MANIFEST" wallet refresh >"${RUN_DIR}/bundle-wallet-refresh-funded.json"
"$AUTOPILOTCTL_BIN" --manifest "$RUNTIME_MANIFEST" wallet refresh >"${RUN_DIR}/runtime-wallet-refresh-funded.json"
wait_for_manifest_wallet_balance_sync \
  "$BUNDLE_MANIFEST" \
  "$BUNDLE_IDENTITY_PATH" \
  "$BUNDLE_STORAGE_DIR" \
  120 \
  "${RUN_DIR}/bundle-status-funded-manifest.json" \
  "${RUN_DIR}/bundle-status-funded-wallet.json" \
  || die "Bundled app did not converge to its funded wallet balance after funding"
wait_for_manifest_wallet_balance_sync \
  "$RUNTIME_MANIFEST" \
  "$RUNTIME_IDENTITY_PATH" \
  "$RUNTIME_STORAGE_DIR" \
  120 \
  "${RUN_DIR}/runtime-status-funded-manifest.json" \
  "${RUN_DIR}/runtime-status-funded-wallet.json" \
  || die "Runtime app did not converge to its funded wallet balance after funding"

log "Selecting the managed main channel in both apps"
"$AUTOPILOTCTL_BIN" --manifest "$BUNDLE_MANIFEST" chat main >"${RUN_DIR}/bundle-chat-main.json"
"$AUTOPILOTCTL_BIN" --manifest "$RUNTIME_MANIFEST" chat main >"${RUN_DIR}/runtime-chat-main.json"
"$AUTOPILOTCTL_BIN" --manifest "$BUNDLE_MANIFEST" wait nip28-ready --timeout-ms "$WAIT_TIMEOUT_MS" > /dev/null
"$AUTOPILOTCTL_BIN" --manifest "$RUNTIME_MANIFEST" wait nip28-ready --timeout-ms "$WAIT_TIMEOUT_MS" > /dev/null

log "Bringing both apps online as providers"
"$AUTOPILOTCTL_BIN" --manifest "$BUNDLE_MANIFEST" provider online --wait --timeout-ms "$WAIT_TIMEOUT_MS" >"${RUN_DIR}/bundle-provider-online.json"
"$AUTOPILOTCTL_BIN" --manifest "$RUNTIME_MANIFEST" provider online --wait --timeout-ms "$WAIT_TIMEOUT_MS" >"${RUN_DIR}/runtime-provider-online.json"

log "Waiting for targeted buy-mode roster selection"
"$AUTOPILOTCTL_BIN" --manifest "$BUNDLE_MANIFEST" wait buy-mode-target-ready --timeout-ms "$WAIT_TIMEOUT_MS" > /dev/null
"$AUTOPILOTCTL_BIN" --manifest "$RUNTIME_MANIFEST" wait buy-mode-target-ready --timeout-ms "$WAIT_TIMEOUT_MS" > /dev/null
assert_selected_target "$BUNDLE_MANIFEST" "$RUNTIME_PUBKEY" "${RUN_DIR}/bundle-target.json"
assert_selected_target "$RUNTIME_MANIFEST" "$BUNDLE_PUBKEY" "${RUN_DIR}/runtime-target.json"

BUNDLE_TO_RUNTIME_MESSAGE="bundle->runtime $(date +%s)"
RUNTIME_TO_BUNDLE_MESSAGE="runtime->bundle $(date +%s)"

log "Sending chat message from bundled app to runtime app"
"$AUTOPILOTCTL_BIN" --manifest "$BUNDLE_MANIFEST" --json chat send "$BUNDLE_TO_RUNTIME_MESSAGE" --wait --timeout-ms "$WAIT_TIMEOUT_MS" >"${RUN_DIR}/bundle-chat-send.json"
wait_for_chat_message "$RUNTIME_MANIFEST" "$BUNDLE_TO_RUNTIME_MESSAGE" 60 \
  || die "Runtime app did not observe bundled chat message"

log "Sending chat message from runtime app to bundled app"
"$AUTOPILOTCTL_BIN" --manifest "$RUNTIME_MANIFEST" --json chat send "$RUNTIME_TO_BUNDLE_MESSAGE" --wait --timeout-ms "$WAIT_TIMEOUT_MS" >"${RUN_DIR}/runtime-chat-send.json"
wait_for_chat_message "$BUNDLE_MANIFEST" "$RUNTIME_TO_BUNDLE_MESSAGE" 60 \
  || die "Bundled app did not observe runtime chat message"

log "Running runtime buyer -> bundled seller payment flow"
run_buy_cycle "runtime" "$RUNTIME_MANIFEST" "bundle" "$BUNDLE_MANIFEST" "$BUNDLE_IDENTITY_PATH" "$BUNDLE_STORAGE_DIR" "$BUNDLE_LATEST_LOG" "$BUNDLE_SESSION_LOG" 180 "runtime-buys-bundle"

log "Running bundled buyer -> runtime seller payment flow"
run_buy_cycle "bundle" "$BUNDLE_MANIFEST" "runtime" "$RUNTIME_MANIFEST" "$RUNTIME_IDENTITY_PATH" "$RUNTIME_STORAGE_DIR" "$RUNTIME_LATEST_LOG" "$RUNTIME_SESSION_LOG" 180 "bundle-buys-runtime"

log "Capturing final control snapshots"
"$AUTOPILOTCTL_BIN" --manifest "$BUNDLE_MANIFEST" --json status >"${RUN_DIR}/bundle-final-status.json"
"$AUTOPILOTCTL_BIN" --manifest "$RUNTIME_MANIFEST" --json status >"${RUN_DIR}/runtime-final-status.json"

python3 - "$RUN_DIR" "$RELAY_URL" "$CHANNEL_ID" "$BUNDLE_PUBKEY" "$RUNTIME_PUBKEY" "$BUNDLE_SESSION_LOG" "$RUNTIME_SESSION_LOG" "$BUNDLE_MANIFEST" "$RUNTIME_MANIFEST" <<'PY'
import json
import pathlib
import sys

run_dir = pathlib.Path(sys.argv[1])
relay_url = sys.argv[2]
channel_id = sys.argv[3]
bundle_pubkey = sys.argv[4]
runtime_pubkey = sys.argv[5]
bundle_session_log = sys.argv[6]
runtime_session_log = sys.argv[7]
bundle_manifest = sys.argv[8]
runtime_manifest = sys.argv[9]
run_dir_path = pathlib.Path(run_dir)

def load_balance(path: pathlib.Path):
    if not path.exists():
        return None
    payload = json.loads(path.read_text())
    return payload.get("snapshot", {}).get("wallet", {}).get("balance_sats")

def assert_split_shell_status(label: str, snapshot: dict):
    session = snapshot.get("session", {})
    shell_mode = session.get("shell_mode")
    dev_mode_enabled = session.get("dev_mode_enabled")
    if shell_mode != "hotbar":
        raise SystemExit(f"{label} shell_mode changed unexpectedly: {shell_mode!r}")
    if dev_mode_enabled is not False:
        raise SystemExit(f"{label} dev_mode_enabled changed unexpectedly: {dev_mode_enabled!r}")

runtime_buys_bundle_before = load_balance(run_dir_path / "runtime-buys-bundle-bundle-before-status.json")
runtime_buys_bundle_after = load_balance(run_dir_path / "runtime-buys-bundle-bundle-after-status.json")
bundle_buys_runtime_before = load_balance(run_dir_path / "bundle-buys-runtime-runtime-before-status.json")
bundle_buys_runtime_after = load_balance(run_dir_path / "bundle-buys-runtime-runtime-after-status.json")
bundle_final_snapshot = json.loads((run_dir / "bundle-final-status.json").read_text()).get("snapshot", {})
runtime_final_snapshot = json.loads((run_dir / "runtime-final-status.json").read_text()).get("snapshot", {})
assert_split_shell_status("bundle", bundle_final_snapshot)
assert_split_shell_status("runtime", runtime_final_snapshot)

summary = {
    "relayUrl": relay_url,
    "channelId": channel_id,
    "participants": {
        "bundle": {
            "pubkey": bundle_pubkey,
            "manifest": bundle_manifest,
            "sessionLog": bundle_session_log,
        },
        "runtime": {
            "pubkey": runtime_pubkey,
            "manifest": runtime_manifest,
            "sessionLog": runtime_session_log,
        },
    },
    "chat": {
        "bundleToRuntime": json.loads((run_dir / "bundle-chat-send.json").read_text()).get("response", {}).get("message"),
        "runtimeToBundle": json.loads((run_dir / "runtime-chat-send.json").read_text()).get("response", {}).get("message"),
    },
    "paymentCycles": {
        "runtimeBuysBundle": {
            "seller": "bundle",
            "balanceBeforeSats": runtime_buys_bundle_before,
            "balanceAfterSats": runtime_buys_bundle_after,
            "balanceDeltaSats": None if runtime_buys_bundle_before is None or runtime_buys_bundle_after is None else runtime_buys_bundle_after - runtime_buys_bundle_before,
        },
        "bundleBuysRuntime": {
            "seller": "runtime",
            "balanceBeforeSats": bundle_buys_runtime_before,
            "balanceAfterSats": bundle_buys_runtime_after,
            "balanceDeltaSats": None if bundle_buys_runtime_before is None or bundle_buys_runtime_after is None else bundle_buys_runtime_after - bundle_buys_runtime_before,
        },
    },
    "finalStatus": {
        "bundle": bundle_final_snapshot,
        "runtime": runtime_final_snapshot,
    },
}

(run_dir / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
PY

log "Roundtrip complete. Summary written to ${RUN_DIR}/summary.json"
