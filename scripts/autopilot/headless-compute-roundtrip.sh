#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

RUN_DIR="${OPENAGENTS_HEADLESS_RUN_DIR:-$ROOT_DIR/target/headless-compute-roundtrip}"
PROVIDER_HOME="${OPENAGENTS_HEADLESS_PROVIDER_HOME:-$RUN_DIR/provider}"
PROVIDER_IDENTITY_PATH="$PROVIDER_HOME/identity.mnemonic"
BUYER_HOME="${OPENAGENTS_HEADLESS_BUYER_HOME:-$HOME}"
FORWARD_COUNT="${OPENAGENTS_HEADLESS_FORWARD_COUNT:-6}"
REQUESTED_REVERSE_COUNT="${OPENAGENTS_HEADLESS_REVERSE_COUNT:-3}"
INTERVAL_SECONDS="${OPENAGENTS_HEADLESS_INTERVAL_SECONDS:-8}"
TIMEOUT_SECONDS="${OPENAGENTS_HEADLESS_TIMEOUT_SECONDS:-75}"
PROVIDER_BACKEND="${OPENAGENTS_HEADLESS_PROVIDER_BACKEND:-canned}"
BUDGET_SATS="${OPENAGENTS_HEADLESS_BUDGET_SATS:-2}"
SPARK_NETWORK="${OPENAGENTS_SPARK_NETWORK:-mainnet}"

case "$SPARK_NETWORK" in
  mainnet|regtest) ;;
  *)
    echo "OPENAGENTS_SPARK_NETWORK=${SPARK_NETWORK} is unsupported for this script; spark-wallet-cli only supports mainnet or regtest here" >&2
    exit 1
    ;;
esac

if (( FORWARD_COUNT < 1 )); then
  echo "OPENAGENTS_HEADLESS_FORWARD_COUNT must be at least 1" >&2
  exit 1
fi

if (( REQUESTED_REVERSE_COUNT < 1 )); then
  echo "OPENAGENTS_HEADLESS_REVERSE_COUNT must be at least 1" >&2
  exit 1
fi

rm -rf "$RUN_DIR"
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
HEADLESS_BIN="$ROOT_DIR/target/debug/autopilot-headless-compute"
SPARK_BIN="$ROOT_DIR/target/debug/spark-wallet-cli"

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

echo "building headless compute + spark wallet binaries"
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

echo "starting local relay on ${RELAY_URL}"
"$HEADLESS_BIN" relay --listen "127.0.0.1:${PORT}" >"$RUN_DIR/relay.log" 2>&1 &
RELAY_PID=$!
sleep 2

json_field() {
  local file="$1"
  local field="$2"
  python3 - "$file" "$field" <<'PY'
import json, pathlib, sys
value = json.loads(pathlib.Path(sys.argv[1]).read_text())
node = value
for key in sys.argv[2].split("."):
    node = node[key]
print(node)
PY
}

wallet_status() {
  local output_path="$1"
  shift
  HOME="$BUYER_HOME" \
  "$SPARK_BIN" \
    --network "$SPARK_NETWORK" \
    "$@" \
    status >"$output_path"
}

run_phase() {
  local phase="$1"
  local provider_identity_path="$2"
  local buyer_identity_path="$3"
  local target_provider_pubkey="$4"
  local settled_count="$5"
  local provider_log="$RUN_DIR/${phase}-provider.log"
  local buyer_log="$RUN_DIR/${phase}-buyer.log"

  local provider_cmd=(
    "$HEADLESS_BIN" provider
    --relay "$RELAY_URL"
    --backend "$PROVIDER_BACKEND"
    --max-settled-jobs "$settled_count"
  )
  if [[ -n "$provider_identity_path" ]]; then
    provider_cmd+=(--identity-path "$provider_identity_path")
  fi

  local buyer_cmd=(
    "$HEADLESS_BIN" buyer
    --relay "$RELAY_URL"
    --budget-sats "$BUDGET_SATS"
    --timeout-seconds "$TIMEOUT_SECONDS"
    --interval-seconds "$INTERVAL_SECONDS"
    --target-provider-pubkey "$target_provider_pubkey"
    --max-settled-requests "$settled_count"
    --fail-fast
  )
  if [[ -n "$buyer_identity_path" ]]; then
    buyer_cmd+=(--identity-path "$buyer_identity_path")
  fi

  echo "starting ${phase} provider"
  "${provider_cmd[@]}" >"$provider_log" 2>&1 &
  PROVIDER_PID=$!
  sleep 5

  echo "starting ${phase} buyer"
  if ! HOME="$BUYER_HOME" "${buyer_cmd[@]}" >"$buyer_log" 2>&1; then
    echo
    echo "${phase} buyer failed; logs follow"
    echo "--- ${phase}-provider.log ---"
    cat "$provider_log"
    echo "--- ${phase}-buyer.log ---"
    cat "$buyer_log"
    exit 1
  fi

  wait "$PROVIDER_PID"
  PROVIDER_PID=""
}

echo "creating default + provider identities"
HOME="$BUYER_HOME" "$HEADLESS_BIN" identity >"$RUN_DIR/default-identity.json"
"$HEADLESS_BIN" identity --identity-path "$PROVIDER_IDENTITY_PATH" >"$RUN_DIR/provider-identity.json"

DEFAULT_PUBKEY="$(json_field "$RUN_DIR/default-identity.json" publicKeyHex)"
DEFAULT_NPUB="$(json_field "$RUN_DIR/default-identity.json" npub)"
PROVIDER_PUBKEY="$(json_field "$RUN_DIR/provider-identity.json" publicKeyHex)"
PROVIDER_NPUB="$(json_field "$RUN_DIR/provider-identity.json" npub)"

echo "capturing initial wallet status"
wallet_status "$RUN_DIR/default-status-initial.json"
wallet_status "$RUN_DIR/provider-status-initial.json" --identity-path "$PROVIDER_IDENTITY_PATH"

DEFAULT_INITIAL_BALANCE="$(json_field "$RUN_DIR/default-status-initial.json" balance.totalSats)"
MIN_FORWARD_BALANCE=$((FORWARD_COUNT * BUDGET_SATS))
if (( DEFAULT_INITIAL_BALANCE < MIN_FORWARD_BALANCE )); then
  echo "default buyer wallet in HOME=${BUYER_HOME} only has ${DEFAULT_INITIAL_BALANCE} sats on ${SPARK_NETWORK}; requires at least ${MIN_FORWARD_BALANCE} sats to attempt ${FORWARD_COUNT} forward jobs at ${BUDGET_SATS} sats/job" >&2
  echo "fund that wallet first or point OPENAGENTS_HEADLESS_BUYER_HOME at a funded Spark home" >&2
  exit 1
fi

echo "phase 1: default wallet pays provider wallet ${FORWARD_COUNT} times"
run_phase "forward" "$PROVIDER_IDENTITY_PATH" "" "$PROVIDER_PUBKEY" "$FORWARD_COUNT"

echo "capturing post-forward wallet status"
wallet_status "$RUN_DIR/default-status-after-forward.json"
wallet_status "$RUN_DIR/provider-status-after-forward.json" --identity-path "$PROVIDER_IDENTITY_PATH"

DEFAULT_AFTER_FORWARD_BALANCE="$(json_field "$RUN_DIR/default-status-after-forward.json" balance.totalSats)"
PROVIDER_AFTER_FORWARD_BALANCE="$(json_field "$RUN_DIR/provider-status-after-forward.json" balance.totalSats)"

FORWARD_SPEND_TOTAL=$((DEFAULT_INITIAL_BALANCE - DEFAULT_AFTER_FORWARD_BALANCE))
if (( FORWARD_SPEND_TOTAL <= 0 )); then
  echo "forward phase did not reduce the buyer balance; cannot estimate reverse send cost" >&2
  exit 1
fi

ESTIMATED_SEND_COST=$(((FORWARD_SPEND_TOTAL + FORWARD_COUNT - 1) / FORWARD_COUNT))
if (( ESTIMATED_SEND_COST < BUDGET_SATS )); then
  ESTIMATED_SEND_COST="$BUDGET_SATS"
fi

AFFORDABLE_REVERSE_COUNT=$((PROVIDER_AFTER_FORWARD_BALANCE / ESTIMATED_SEND_COST))
if (( AFFORDABLE_REVERSE_COUNT < 1 )); then
  echo "provider wallet has ${PROVIDER_AFTER_FORWARD_BALANCE} sats after forward phase, which is below the estimated ${ESTIMATED_SEND_COST}-sat send cost" >&2
  exit 1
fi

REVERSE_COUNT="$REQUESTED_REVERSE_COUNT"
if (( REVERSE_COUNT > AFFORDABLE_REVERSE_COUNT )); then
  echo "reducing reverse phase from ${REVERSE_COUNT} to ${AFFORDABLE_REVERSE_COUNT} request(s) based on post-forward provider balance ${PROVIDER_AFTER_FORWARD_BALANCE} sats and estimated send cost ${ESTIMATED_SEND_COST} sats/job"
  REVERSE_COUNT="$AFFORDABLE_REVERSE_COUNT"
fi

echo "phase 2: provider wallet pays default wallet ${REVERSE_COUNT} times"
run_phase "reverse" "" "$PROVIDER_IDENTITY_PATH" "$DEFAULT_PUBKEY" "$REVERSE_COUNT"

echo "capturing final wallet status"
wallet_status "$RUN_DIR/default-status-final.json"
wallet_status "$RUN_DIR/provider-status-final.json" --identity-path "$PROVIDER_IDENTITY_PATH"

python3 - "$RUN_DIR" "$RELAY_URL" "$FORWARD_COUNT" "$REQUESTED_REVERSE_COUNT" "$REVERSE_COUNT" "$BUDGET_SATS" "$ESTIMATED_SEND_COST" "$DEFAULT_NPUB" "$PROVIDER_NPUB" <<'PY'
import json
import pathlib
import re
import sys

run_dir = pathlib.Path(sys.argv[1])
relay_url = sys.argv[2]
forward_count = int(sys.argv[3])
requested_reverse_count = int(sys.argv[4])
reverse_count = int(sys.argv[5])
budget_sats = int(sys.argv[6])
estimated_send_cost = int(sys.argv[7])
default_npub = sys.argv[8]
provider_npub = sys.argv[9]

phase_specs = {
    "forward": {
        "payer": "default",
        "provider": "secondary",
    },
    "reverse": {
        "payer": "secondary",
        "provider": "default",
    },
}

def read_json(name: str):
    return json.loads((run_dir / name).read_text())

def extract_all(pattern: str, text: str):
    return [match.groupdict() for match in re.finditer(pattern, text, re.MULTILINE)]

summary = {
    "relayUrl": relay_url,
    "participants": {
        "default": {
            "npub": default_npub,
            "initialTotalSats": read_json("default-status-initial.json")["balance"]["totalSats"],
            "afterForwardTotalSats": read_json("default-status-after-forward.json")["balance"]["totalSats"],
            "finalTotalSats": read_json("default-status-final.json")["balance"]["totalSats"],
        },
        "secondary": {
            "npub": provider_npub,
            "initialTotalSats": read_json("provider-status-initial.json")["balance"]["totalSats"],
            "afterForwardTotalSats": read_json("provider-status-after-forward.json")["balance"]["totalSats"],
            "finalTotalSats": read_json("provider-status-final.json")["balance"]["totalSats"],
        },
    },
    "phases": {},
}

for phase, spec in phase_specs.items():
    buyer_log = (run_dir / f"{phase}-buyer.log").read_text()
    provider_log = (run_dir / f"{phase}-provider.log").read_text()
    buyer_settled = extract_all(
        r"buyer settled request_id=(?P<request_id>[0-9a-f]+) provider=(?P<provider>[0-9a-f]+) result=(?P<result>.+)",
        buyer_log,
    )
    buyer_payments = extract_all(
        r"buyer payment settled request_id=(?P<request_id>[0-9a-f]+) payment_id=(?P<payment_id>[-0-9a-f]+) amount_sats=(?P<amount_sats>\d+)",
        buyer_log,
    )
    provider_settlements = extract_all(
        r"provider settlement confirmed request_id=(?P<request_id>[0-9a-f]+) success_feedback_id=(?P<success_feedback_id>[0-9a-f]+) balance_before=(?P<balance_before>\d+) balance_after=(?P<balance_after>\d+)",
        provider_log,
    )
    provider_invoices = extract_all(
        r"provider queued payment-required feedback request_id=(?P<request_id>[0-9a-f]+) event_id=(?P<event_id>[0-9a-f]+) amount_sats=(?P<amount_sats>\d+)",
        provider_log,
    )

    summary["phases"][phase] = {
        "payer": spec["payer"],
        "provider": spec["provider"],
        "expectedPayments": forward_count if phase == "forward" else reverse_count,
        "buyerSettledCount": len(buyer_settled),
        "providerSettledCount": len(provider_settlements),
        "invoiceCount": len(provider_invoices),
        "paymentCount": len(buyer_payments),
        "requestIds": [entry["request_id"] for entry in buyer_settled],
        "paymentIds": [entry["payment_id"] for entry in buyer_payments],
        "results": [entry["result"].strip() for entry in buyer_settled],
        "providerBalanceAfterEachSettlement": [int(entry["balance_after"]) for entry in provider_settlements],
    }

expected_forward = summary["phases"]["forward"]["expectedPayments"]
expected_reverse = summary["phases"]["reverse"]["expectedPayments"]
if summary["phases"]["forward"]["buyerSettledCount"] != expected_forward:
    raise SystemExit("forward phase did not settle the expected number of buyer requests")
if summary["phases"]["forward"]["providerSettledCount"] != expected_forward:
    raise SystemExit("forward phase did not settle the expected number of provider payouts")
if summary["phases"]["reverse"]["buyerSettledCount"] != expected_reverse:
    raise SystemExit("reverse phase did not settle the expected number of buyer requests")
if summary["phases"]["reverse"]["providerSettledCount"] != expected_reverse:
    raise SystemExit("reverse phase did not settle the expected number of provider payouts")

summary["budgetSats"] = budget_sats
summary["estimatedSendCostSats"] = estimated_send_cost
summary["forwardTransferredSats"] = forward_count * budget_sats
summary["reverseTransferredSats"] = reverse_count * budget_sats
summary["requestedReverseCount"] = requested_reverse_count
summary["executedReverseCount"] = reverse_count
summary["netDeltaSats"] = {
    "default": summary["participants"]["default"]["finalTotalSats"] - summary["participants"]["default"]["initialTotalSats"],
    "secondary": summary["participants"]["secondary"]["finalTotalSats"] - summary["participants"]["secondary"]["initialTotalSats"],
}

(run_dir / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")

lines = [
    "Headless compute roundtrip summary",
    f"default npub:   {default_npub}",
    f"secondary npub: {provider_npub}",
    f"budget per job: {budget_sats} sats",
    f"estimated send cost: {estimated_send_cost} sats",
    f"forward jobs:   {forward_count}",
    f"reverse jobs:   {reverse_count} executed (requested {requested_reverse_count})",
    f"default wallet: {summary['participants']['default']['initialTotalSats']} -> {summary['participants']['default']['afterForwardTotalSats']} -> {summary['participants']['default']['finalTotalSats']} sats",
    f"secondary wallet: {summary['participants']['secondary']['initialTotalSats']} -> {summary['participants']['secondary']['afterForwardTotalSats']} -> {summary['participants']['secondary']['finalTotalSats']} sats",
    f"net delta default: {summary['netDeltaSats']['default']} sats",
    f"net delta secondary: {summary['netDeltaSats']['secondary']} sats",
    f"forward request ids: {', '.join(summary['phases']['forward']['requestIds'])}",
    f"reverse request ids: {', '.join(summary['phases']['reverse']['requestIds'])}",
]
(run_dir / "summary.txt").write_text("\n".join(lines) + "\n")
print("\n".join(lines))
PY

echo
echo "roundtrip logs and summaries:"
echo "relay log:    $RUN_DIR/relay.log"
echo "forward provider log: $RUN_DIR/forward-provider.log"
echo "forward buyer log:    $RUN_DIR/forward-buyer.log"
echo "reverse provider log: $RUN_DIR/reverse-provider.log"
echo "reverse buyer log:    $RUN_DIR/reverse-buyer.log"
echo "summary json: $RUN_DIR/summary.json"
