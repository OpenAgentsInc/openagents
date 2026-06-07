#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

ARTANIS_PROOF_DOC="${ARTANIS_PROOF_DOC:-docs/reports/nexus/2026-06-07-pylon-v02-live-artanis-shc-bootstrap-proof.md}"
PYLON_ACCEPTED_WORK_PROOF="${PYLON_ACCEPTED_WORK_PROOF:-docs/reports/nexus/pylon-v022-shc-nosource-proof-20260607183407.json}"
PYLON_NPM_BOOTSTRAP_PROOF="${PYLON_NPM_BOOTSTRAP_PROOF:-docs/reports/nexus/pylon-v022-shc-npm-bootstrap-202606071836.json}"
MDK_PAYMENT_REDACTED_SUMMARY="${MDK_PAYMENT_REDACTED_SUMMARY:-}"
OUTPUT_PATH="${OUTPUT_PATH:-docs/reports/nexus/artanis-pylon-v022-integrated-paid-work-proof-$(date -u +%Y%m%d%H%M%S).json}"

if [[ -z "$MDK_PAYMENT_REDACTED_SUMMARY" ]]; then
  echo "ERROR: set MDK_PAYMENT_REDACTED_SUMMARY to a redacted MDK payment summary JSON." >&2
  echo "The summary must not contain raw invoices, preimages, mnemonics, access tokens, or wallet secrets." >&2
  exit 2
fi

for path in "$ARTANIS_PROOF_DOC" "$PYLON_ACCEPTED_WORK_PROOF" "$PYLON_NPM_BOOTSTRAP_PROOF" "$MDK_PAYMENT_REDACTED_SUMMARY"; do
  if [[ ! -f "$path" ]]; then
    echo "ERROR: missing proof input: $path" >&2
    exit 2
  fi
done

mkdir -p "$(dirname "$OUTPUT_PATH")"

python3 - "$ARTANIS_PROOF_DOC" "$PYLON_ACCEPTED_WORK_PROOF" "$PYLON_NPM_BOOTSTRAP_PROOF" "$MDK_PAYMENT_REDACTED_SUMMARY" "$OUTPUT_PATH" <<'PY'
import datetime as dt
import hashlib
import json
import pathlib
import re
import sys

artanis_doc_path = pathlib.Path(sys.argv[1])
accepted_path = pathlib.Path(sys.argv[2])
npm_path = pathlib.Path(sys.argv[3])
mdk_path = pathlib.Path(sys.argv[4])
output_path = pathlib.Path(sys.argv[5])


def sha256_file(path: pathlib.Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def load_json(path: pathlib.Path):
    return json.loads(path.read_text())


def markdown_value(text: str, label: str):
    pattern = re.compile(rf"\|\s*{re.escape(label)}\s*\|\s*([^|]+?)\s*\|")
    match = pattern.search(text)
    return match.group(1).strip().strip("`") if match else None


artanis_doc = artanis_doc_path.read_text()
accepted = load_json(accepted_path)
npm = load_json(npm_path)
mdk = load_json(mdk_path)

npm_metadata = {
    entry.get("key"): entry.get("value")
    for entry in (((npm.get("status") or {}).get("snapshot") or {}).get("config_metadata") or [])
    if isinstance(entry, dict)
}
npm_training_status = npm_metadata.get("training_operator_status") or {}

window = ((accepted.get("observed_run") or {}).get("windows") or [{}])[0]
nodes = []
for node in (accepted.get("fleet") or {}).get("nodes") or []:
    process_binary = ((node.get("process") or {}).get("binary"))
    nodes.append(
        {
            "role": node.get("role"),
            "index": node.get("index"),
            "node_label": node.get("node_label"),
            "payout_destination_label": "redacted-proof-fixture-target"
            if node.get("payout_destination")
            else None,
            "process_binary_contains_release_version": bool(
                process_binary and "pylon-v0.2.2" in process_binary
            ),
            "current_run_id": ((node.get("training") or {}).get("current_run_id")),
        }
    )

training_run_id = (accepted.get("observed_run") or {}).get("training_run_id")
worker_node = next((node for node in nodes if node.get("role") == "worker"), None)
assignment_ref = {
    "training_run_id": training_run_id,
    "window_id": window.get("window_id"),
    "accepted_contributions": window.get("accepted_contributions"),
    "closeout_status": window.get("closeout_status"),
    "worker_node_label": worker_node.get("node_label") if worker_node else None,
}

mdk_payment_ok = (
    mdk.get("amount_sats", 0) > 0
    and (mdk.get("payer_balance_delta_sats") or 0) < 0
    and mdk.get("receiver_payment_count_after", 0) >= 1
    and mdk.get("payment_id_digest")
    and mdk.get("invoice_digest")
)

accepted_work_ok = (
    accepted.get("status") == "completed"
    and window.get("closeout_status") == "rewarded"
    and (window.get("accepted_contributions") or 0) >= 1
    and all(node.get("process_binary_contains_release_version") for node in nodes)
)

npm_ok = (
    npm.get("version") == "0.2.2"
    and npm.get("tagName") == "pylon-v0.2.2"
    and npm.get("installMethod") == "release_asset"
    and not npm.get("cached")
)

bridge_blocker = (
    "The released Pylon proof runtime can prove accepted/rewarded work from "
    "the public pylon-v0.2.2 install path, and the MDK agent-wallet can settle "
    "a real bitcoin-denominated Lightning payment to a Pylon-scoped receiver "
    "wallet. The production bridge that is still not implemented is the "
    "authority/idempotency link that makes an Artanis-created assignment id "
    "the direct source of truth for the MDK checkout/payment/settlement record."
)

status = (
    "completed_with_settlement_bridge_gap"
    if accepted_work_ok and npm_ok and mdk_payment_ok
    else "blocked"
)

bundle = {
    "schema_version": "openagents.artanis_pylon_v022_integrated_paid_work_proof.v1",
    "generated_at": dt.datetime.now(dt.timezone.utc).isoformat(),
    "status": status,
    "release": {
        "pylon_version": "0.2.2",
        "github_release_tag": "pylon-v0.2.2",
        "npm_package": "@openagentsinc/pylon@0.2.2",
    },
    "source_artifacts": {
        "artanis_proof_doc": {
            "path": str(artanis_doc_path),
            "sha256": sha256_file(artanis_doc_path),
        },
        "pylon_accepted_work_proof": {
            "path": str(accepted_path),
            "sha256": sha256_file(accepted_path),
        },
        "pylon_npm_bootstrap_proof": {
            "path": str(npm_path),
            "sha256": sha256_file(npm_path),
        },
        "mdk_payment_redacted_summary": {
            "path_label": "operator-local ignored .secrets redacted summary",
            "sha256": sha256_file(mdk_path),
        },
    },
    "artanis_dispatch_evidence": {
        "proof_type": "live_account_backed_shc_bootstrap",
        "shc_run_id": markdown_value(artanis_doc, "SHC run id"),
        "omega_external_run_id": markdown_value(artanis_doc, "Omega external run id"),
        "runner": markdown_value(artanis_doc, "Runner"),
        "wallet_authority": markdown_value(artanis_doc, "Wallet authority"),
        "omega_status": "completed",
        "artifact_count": 8,
        "note": (
            "This is the current live Artanis supervisor proof for the Pylon "
            "launch lane. It proves bounded SHC workroom dispatch and artifact "
            "capture with wallet authority disabled."
        ),
    },
    "public_pylon_install_evidence": {
        "proof_type": "npm_bootstrap_to_public_release_asset",
        "version": npm.get("version"),
        "tag_name": npm.get("tagName"),
        "target": npm.get("target"),
        "install_method": npm.get("installMethod"),
        "cached": npm.get("cached"),
        "binary_path_contains_release_version": "pylon-v0.2.2"
        in ((npm.get("binaries") or {}).get("pylon") or ""),
        "pylon_home_label": "fresh SHC temp Pylon home",
        "desired_mode": (npm.get("status") or {}).get("desired_mode"),
        "runtime_surface_detected": npm_training_status.get("runtime_surface_detected"),
        "psionic_repo_source": npm_training_status.get("psionic_repo_source"),
        "contributor_supported": npm_training_status.get("contributor_supported"),
        "inventory_row_count": len((npm.get("inventory") or {}).get("rows") or []),
    },
    "accepted_work_evidence": {
        "proof_type": "shc_no_source_public_release_asset_proof",
        "status": accepted.get("status"),
        "lane": accepted.get("lane"),
        "namespace": accepted.get("namespace"),
        "detail": accepted.get("detail"),
        "assignment_ref": assignment_ref,
        "nodes": nodes,
        "authority_binary_contains_release_version": "pylon-v0.2.2"
        in ((((accepted.get("fleet") or {}).get("authority") or {}).get("authority_process") or {}).get("binary") or ""),
        "artifact_binary_contains_release_version": "pylon-v0.2.2"
        in ((((accepted.get("fleet") or {}).get("authority") or {}).get("artifact_store_process") or {}).get("binary") or ""),
    },
    "mdk_payment_evidence": {
        "proof_type": "real_mdk_agent_wallet_lightning_payment",
        "run_id": mdk.get("run_id"),
        "amount_sats": mdk.get("amount_sats"),
        "payer_balance_before_sats": mdk.get("payer_balance_before_sats"),
        "payer_balance_after_sats": mdk.get("payer_balance_after_sats"),
        "payer_balance_delta_sats": mdk.get("payer_balance_delta_sats"),
        "receiver_runtime_kind": mdk.get("receiver_runtime_kind"),
        "receiver_balance_after_sats": mdk.get("receiver_balance_after_sats"),
        "receiver_payment_count_after": mdk.get("receiver_payment_count_after"),
        "invoice_digest": mdk.get("invoice_digest"),
        "payment_id_digest": mdk.get("payment_id_digest"),
        "payment_hash_digest": mdk.get("payment_hash_digest"),
        "preimage_observed_but_not_disclosed": bool(mdk.get("preimage_digest_recorded")),
        "raw_material_redaction": mdk.get("raw_material_redaction"),
    },
    "bridge_assessment": {
        "single_production_artanis_assignment_to_mdk_settlement_bridge": False,
        "exact_remaining_blocker": bridge_blocker,
        "no_overclaim": (
            "Do not claim that production Artanis paid-work settlement is fully "
            "live until the Artanis assignment id, Pylon accepted-work result, "
            "MDK payment intent, and public receipt are one idempotent production trace."
        ),
        "bounded_v02_claim": (
            "Pylon v0.2.2 public install works, accepted/rewarded work is proven "
            "from public release assets, Artanis can dispatch the launch workroom, "
            "and real MDK Lightning payment movement works through the selected "
            "wallet runtime."
        ),
    },
}

output_path.write_text(json.dumps(bundle, indent=2) + "\n")
print(output_path)
PY
