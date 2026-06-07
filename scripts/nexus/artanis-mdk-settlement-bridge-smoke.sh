#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

ARTANIS_PROOF_DOC="${ARTANIS_PROOF_DOC:-docs/reports/nexus/2026-06-07-pylon-v02-live-artanis-shc-bootstrap-proof.md}"
PYLON_ACCEPTED_WORK_PROOF="${PYLON_ACCEPTED_WORK_PROOF:-docs/reports/nexus/pylon-v022-shc-nosource-proof-20260607183407.json}"
PYLON_PACKAGE="${PYLON_PACKAGE:-@openagentsinc/pylon@0.2.2}"
PYLON_VERSION="${PYLON_VERSION:-0.2.2}"
AMOUNT_SATS="${AMOUNT_SATS:-21}"
MDK_PAYER_HOME="${MDK_PAYER_HOME:-}"
MDK_PAYER_PORT="${MDK_PAYER_PORT:-3462}"
TIMESTAMP="${TIMESTAMP:-$(date -u +%Y%m%d%H%M%S)}"
RAW_ARTIFACT_DIR="${RAW_ARTIFACT_DIR:-${ROOT_DIR}/target/private/artanis-mdk-settlement-bridge-${TIMESTAMP}}"
OUTPUT_PATH="${OUTPUT_PATH:-docs/reports/nexus/artanis-mdk-settlement-bridge-smoke-${TIMESTAMP}.json}"

if [[ -z "$MDK_PAYER_HOME" ]]; then
  echo "ERROR: set MDK_PAYER_HOME to the funded local MDK agent-wallet HOME." >&2
  exit 2
fi

for path in "$ARTANIS_PROOF_DOC" "$PYLON_ACCEPTED_WORK_PROOF"; do
  if [[ ! -f "$path" ]]; then
    echo "ERROR: missing proof input: $path" >&2
    exit 2
  fi
done

mkdir -p "$RAW_ARTIFACT_DIR" "$(dirname "$OUTPUT_PATH")"

PLAN_PATH="$RAW_ARTIFACT_DIR/settlement-plan.json"
python3 - "$ARTANIS_PROOF_DOC" "$PYLON_ACCEPTED_WORK_PROOF" "$TIMESTAMP" "$AMOUNT_SATS" "$PLAN_PATH" <<'PY'
import hashlib
import json
import pathlib
import re
import sys

artanis_doc_path = pathlib.Path(sys.argv[1])
accepted_path = pathlib.Path(sys.argv[2])
timestamp = sys.argv[3]
amount_sats = int(sys.argv[4])
plan_path = pathlib.Path(sys.argv[5])

artanis_doc = artanis_doc_path.read_text()
accepted = json.loads(accepted_path.read_text())


def markdown_value(text: str, label: str):
    pattern = re.compile(rf"\|\s*{re.escape(label)}\s*\|\s*([^|]+?)\s*\|")
    match = pattern.search(text)
    return match.group(1).strip().strip("`") if match else None


window = ((accepted.get("observed_run") or {}).get("windows") or [{}])[0]
artanis_run_id = markdown_value(artanis_doc, "SHC run id")
training_run_id = (accepted.get("observed_run") or {}).get("training_run_id")
window_id = window.get("window_id")
assignment_material = "|".join(
    value or ""
    for value in [
        "artanis-mdk-settlement-bridge-v1",
        artanis_run_id,
        training_run_id,
        window_id,
        timestamp,
    ]
)
assignment_digest = hashlib.sha256(assignment_material.encode()).hexdigest()
assignment_id = f"artanis-mdk-bridge-{assignment_digest[:24]}"
settlement_intent_id = f"settlement-{assignment_digest[:32]}"
receipt_id = f"receipt-{assignment_digest[:32]}"
description = (
    "OpenAgents Artanis paid-work settlement "
    f"assignment={assignment_id} "
    f"artanis={artanis_run_id} "
    f"training={training_run_id} "
    f"window={window_id}"
)

plan = {
    "assignment_id": assignment_id,
    "settlement_intent_id": settlement_intent_id,
    "receipt_id": receipt_id,
    "amount_sats": amount_sats,
    "description": description,
    "artanis_run_id": artanis_run_id,
    "training_run_id": training_run_id,
    "window_id": window_id,
    "accepted_contributions": window.get("accepted_contributions"),
    "closeout_status": window.get("closeout_status"),
}
plan_path.write_text(json.dumps(plan, indent=2) + "\n")
print(description)
PY

DESCRIPTION="$(jq -r '.description' "$PLAN_PATH")"
RECEIVER_HOME="$RAW_ARTIFACT_DIR/receiver-pylon-home"

HOME="$RAW_ARTIFACT_DIR/npm-home" \
NPM_CONFIG_CACHE="$RAW_ARTIFACT_DIR/npm-cache" \
OPENAGENTS_DISABLE_TELEMETRY=1 \
npm exec --yes --package "$PYLON_PACKAGE" -- pylon \
  --version "$PYLON_VERSION" \
  --install-root "$RAW_ARTIFACT_DIR/install" \
  --pylon-home "$RECEIVER_HOME" \
  --no-launch \
  --no-updates \
  --json >"$RAW_ARTIFACT_DIR/pylon-bootstrap.json"

PYLON_BIN="$(jq -r '.binaries.pylon // empty' "$RAW_ARTIFACT_DIR/pylon-bootstrap.json")"
if [[ -z "$PYLON_BIN" || ! -x "$PYLON_BIN" ]]; then
  echo "ERROR: pylon bootstrap did not return an executable pylon binary." >&2
  jq '.' "$RAW_ARTIFACT_DIR/pylon-bootstrap.json" >&2
  exit 1
fi

OPENAGENTS_PYLON_HOME="$RECEIVER_HOME" "$PYLON_BIN" wallet status --json >"$RAW_ARTIFACT_DIR/receiver-status-before.json"
OPENAGENTS_PYLON_HOME="$RECEIVER_HOME" "$PYLON_BIN" wallet balance --json >"$RAW_ARTIFACT_DIR/receiver-balance-before.json"
OPENAGENTS_PYLON_HOME="$RECEIVER_HOME" "$PYLON_BIN" wallet invoice "$AMOUNT_SATS" --description "$DESCRIPTION" --json >"$RAW_ARTIFACT_DIR/receiver-invoice.json"

INVOICE="$(jq -r '.invoice.payment_request // .invoice.bolt11 // .payment_request // .bolt11 // .invoice // empty' "$RAW_ARTIFACT_DIR/receiver-invoice.json")"
if [[ -z "$INVOICE" || "$INVOICE" == "null" ]]; then
  echo "ERROR: failed to extract BOLT11 invoice from receiver-invoice.json" >&2
  jq 'keys' "$RAW_ARTIFACT_DIR/receiver-invoice.json" >&2
  exit 1
fi

HOME="$MDK_PAYER_HOME" MDK_WALLET_PORT="$MDK_PAYER_PORT" \
  npx @moneydevkit/agent-wallet@latest balance >"$RAW_ARTIFACT_DIR/payer-balance-before.json"
HOME="$MDK_PAYER_HOME" MDK_WALLET_PORT="$MDK_PAYER_PORT" \
  npx @moneydevkit/agent-wallet@latest send "$INVOICE" >"$RAW_ARTIFACT_DIR/payer-send.json"
HOME="$MDK_PAYER_HOME" MDK_WALLET_PORT="$MDK_PAYER_PORT" \
  npx @moneydevkit/agent-wallet@latest balance >"$RAW_ARTIFACT_DIR/payer-balance-after.json"

for _ in $(seq 1 20); do
  OPENAGENTS_PYLON_HOME="$RECEIVER_HOME" "$PYLON_BIN" wallet balance --json >"$RAW_ARTIFACT_DIR/receiver-balance-after.json" || true
  OPENAGENTS_PYLON_HOME="$RECEIVER_HOME" "$PYLON_BIN" wallet history --limit 10 --json >"$RAW_ARTIFACT_DIR/receiver-history-after.json" || true
  RECEIVER_TOTAL="$(jq -r '.total_sats // .balance.total_sats // .balance_sats // 0' "$RAW_ARTIFACT_DIR/receiver-balance-after.json" 2>/dev/null || echo 0)"
  RECEIVER_PAYMENT_COUNT="$(jq -r '(.payments // .recent_payments // []) | length' "$RAW_ARTIFACT_DIR/receiver-history-after.json" 2>/dev/null || echo 0)"
  if [[ "$RECEIVER_TOTAL" != "0" || "$RECEIVER_PAYMENT_COUNT" != "0" ]]; then
    break
  fi
  sleep 3
done

python3 - "$RAW_ARTIFACT_DIR" "$OUTPUT_PATH" <<'PY'
import datetime as dt
import hashlib
import json
import pathlib
import sys

raw_dir = pathlib.Path(sys.argv[1])
output_path = pathlib.Path(sys.argv[2])


def load(name: str):
    return json.loads((raw_dir / name).read_text())


def digest(value):
    if value is None:
        return None
    if not isinstance(value, str):
        value = json.dumps(value, sort_keys=True)
    return "sha256:" + hashlib.sha256(value.encode()).hexdigest()


plan = load("settlement-plan.json")
bootstrap = load("pylon-bootstrap.json")
receiver_status = load("receiver-status-before.json")
receiver_before = load("receiver-balance-before.json")
receiver_after = load("receiver-balance-after.json")
receiver_history = load("receiver-history-after.json")
invoice_payload = load("receiver-invoice.json")
payer_before = load("payer-balance-before.json")
payer_after = load("payer-balance-after.json")
payer_send = load("payer-send.json")

receiver_payments = receiver_history.get("payments") or receiver_history.get("recent_payments") or []
invoice = invoice_payload.get("invoice", {}) if isinstance(invoice_payload.get("invoice"), dict) else invoice_payload
payment_request = (
    invoice.get("payment_request")
    or invoice.get("bolt11")
    or invoice_payload.get("payment_request")
    or invoice_payload.get("bolt11")
    or invoice_payload.get("invoice")
)
payment_id = payer_send.get("payment_id") or payer_send.get("paymentId")
payment_hash = payer_send.get("payment_hash") or payer_send.get("paymentHash")
preimage = payer_send.get("preimage")
payer_delta = None
if payer_before.get("balance_sats") is not None and payer_after.get("balance_sats") is not None:
    payer_delta = payer_after["balance_sats"] - payer_before["balance_sats"]

receiver_total_after = (
    receiver_after.get("total_sats")
    or (receiver_after.get("balance") or {}).get("total_sats")
    or receiver_after.get("balance_sats")
)
status = "completed"
if not ((payer_delta or 0) < 0 and len(receiver_payments) >= 1 and receiver_total_after):
    status = "blocked"

receipt = {
    "schema_version": "openagents.artanis_mdk_settlement_bridge_smoke.v1",
    "generated_at": dt.datetime.now(dt.timezone.utc).isoformat(),
    "status": status,
    "assignment": {
        "assignment_id": plan["assignment_id"],
        "settlement_intent_id": plan["settlement_intent_id"],
        "receipt_id": plan["receipt_id"],
        "artanis_run_id": plan["artanis_run_id"],
        "training_run_id": plan["training_run_id"],
        "window_id": plan["window_id"],
        "accepted_contributions": plan["accepted_contributions"],
        "closeout_status": plan["closeout_status"],
    },
    "public_pylon_install": {
        "package": "@openagentsinc/pylon@0.2.2",
        "version": bootstrap.get("version"),
        "tag_name": bootstrap.get("tagName"),
        "install_method": bootstrap.get("installMethod"),
        "cached": bootstrap.get("cached"),
        "target": bootstrap.get("target"),
        "binary_path_contains_release_version": "pylon-v0.2.2"
        in ((bootstrap.get("binaries") or {}).get("pylon") or ""),
    },
    "receiver_wallet": {
        "runtime_kind": (receiver_status.get("runtime") or {}).get("runtime_kind"),
        "local_daemon_port": (receiver_status.get("runtime") or {}).get("local_daemon_port"),
        "balance_before_sats": receiver_before.get("total_sats")
        or (receiver_before.get("balance") or {}).get("total_sats")
        or receiver_before.get("balance_sats"),
        "balance_after_sats": receiver_total_after,
        "payment_count_after": len(receiver_payments),
    },
    "payer_wallet": {
        "balance_before_sats": payer_before.get("balance_sats"),
        "balance_after_sats": payer_after.get("balance_sats"),
        "balance_delta_sats": payer_delta,
    },
    "payment": {
        "amount_sats": plan["amount_sats"],
        "description_digest": digest(plan["description"]),
        "invoice_digest": digest(payment_request),
        "payment_id_digest": digest(payment_id),
        "payment_hash_digest": digest(payment_hash),
        "preimage_observed_but_not_disclosed": bool(preimage),
    },
    "redaction": {
        "raw_material_location": "ignored private artifact directory",
        "raw_material_not_committed": [
            "invoice",
            "payment_id",
            "payment_hash",
            "preimage",
            "mnemonic",
            "wallet_config",
            "access_token",
        ],
    },
}
output_path.write_text(json.dumps(receipt, indent=2) + "\n")
print(json.dumps(receipt, indent=2))
PY
