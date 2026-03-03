#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPTS_DIR="$ROOT_DIR/scripts"
ASSETS_DIR="$ROOT_DIR/assets"
source "$SCRIPTS_DIR/_lib_receipts.sh"

require_command() {
    local name="$1"
    if ! command -v "$name" >/dev/null 2>&1; then
        printf 'Missing required command: %s\n' "$name" >&2
        exit 1
    fi
}

require_command bash
require_command envsubst
require_command jq

for script in "$SCRIPTS_DIR"/*.sh; do
    bash -n "$script"
done

export CAST_APP_IDENTITY="b/0000000000000000000000000000000000000000000000000000000000000000/a471d3fcc436ae7cbc0e0c82a68cdc8e003ee21ef819e1acf834e11c43ce47d8"
export CAST_ASSET_APP_IDENTITY="t/asset_tag/asset_vk"
export CAST_APP_INDEX="0"
export CAST_ASSET_APP_INDEX="1"
export CAST_TAKER_FEE_BPS="50"
export CAST_MATCHER_FEE_BPS="2000"
export CAST_FEE_ADDRESS="bc1qfeeaddress0000000000000000000000000000000"
export CAST_FEE_DEST="0014ffffffffffffffffffffffffffffffffffffffff"
export CAST_MIN_VALUE_SATS="10000"
export CAST_OPERATOR_SIGNATURE="deadbeef"
export CAST_MAKER_ASSET_UTXO="txid000:0"
export CAST_FUNDING_UTXO="txid005:2"
export CAST_ORDER_ASSET_QUANTITY="100000000"
export CAST_ORDER_SCROLLS_ADDRESS="bc1qscrolls000000000000000000000000000000000"
export CAST_ORDER_SCROLLS_DEST="0014aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
export CAST_MAKER_BTC_ADDRESS="bc1qmaker0000000000000000000000000000000000"
export CAST_PRICE_NUMERATOR="1"
export CAST_PRICE_DENOMINATOR="400000"
export CAST_ORDER_AMOUNT_SATS="25000"
export CAST_CANCEL_INPUT_INDEX="0"
export CAST_CANCEL_SIGNATURE_HEX="cafebabe"
export CAST_CANCEL_ORDER_UTXO="txid001:0"
export CAST_CANCEL_ORDER_ASSET_QUANTITY="50000000"
export CAST_EXISTING_ORDER_MAKER_ADDRESS="$CAST_MAKER_BTC_ADDRESS"
export CAST_EXISTING_EXEC_TYPE="all_or_none"
export CAST_EXISTING_PRICE_NUMERATOR="1"
export CAST_EXISTING_PRICE_DENOMINATOR="500000"
export CAST_EXISTING_ORDER_AMOUNT_SATS="10000"
export CAST_ADDITIONAL_ASSET_UTXO="txid002:1"
export CAST_ADDITIONAL_ASSET_QUANTITY="49000000"
export CAST_REPLACEMENT_SCROLLS_ADDRESS="bc1qreplacement00000000000000000000000000000"
export CAST_REPLACEMENT_TOTAL_ASSET_QUANTITY="99000000"
export CAST_REPLACEMENT_MAKER_ADDRESS="$CAST_MAKER_BTC_ADDRESS"
export CAST_REPLACEMENT_PRICE_NUMERATOR="1"
export CAST_REPLACEMENT_PRICE_DENOMINATOR="400000"
export CAST_REPLACEMENT_AMOUNT_SATS="24750"
export CAST_REPLACEMENT_SCROLLS_DEST="0014bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
export CAST_ORDER_UTXO="txid003:0"
export CAST_ORDER_MAKER_ADDRESS="$CAST_MAKER_BTC_ADDRESS"
export CAST_ORDER_MAKER_DEST="0014cccccccccccccccccccccccccccccccccccccccc"
export CAST_TAKER_FUNDING_UTXO="txid004:3"
export CAST_TAKER_RECEIVE_ADDRESS="bc1qtaker0000000000000000000000000000000000"
export CAST_TAKER_RECEIVE_DEST="0014dddddddddddddddddddddddddddddddddddddddd"
export CAST_FILL_ASSET_QUANTITY="30000000"
export CAST_REMAINDER_ASSET_QUANTITY="69000000"
export CAST_REMAINDER_AMOUNT_SATS="17250"
export CAST_MAKER_PAYOUT_COIN="7800"
export CAST_TOTAL_FEE_COIN="1000"

render_dir="$(mktemp -d)"
for template in "$ASSETS_DIR"/*.template.yaml; do
    out="$render_dir/$(basename "${template%.template.yaml}").yaml"
    envsubst < "$template" > "$out"
    if rg -n '\${[A-Z0-9_]+}' "$out" >/dev/null 2>&1; then
        printf 'Unresolved template variable in rendered file: %s\n' "$out" >&2
        exit 1
    fi
done

rendered_files_json="$(find "$render_dir" -maxdepth 1 -type f -name '*.yaml' -print | LC_ALL=C sort | jq -Rsc 'split("\n") | map(select(length > 0))')"
receipt_json="$(jq -n \
    --arg root_dir "$ROOT_DIR" \
    --arg scripts_dir "$SCRIPTS_DIR" \
    --arg assets_dir "$ASSETS_DIR" \
    --arg render_dir "$render_dir" \
    --argjson rendered_files "$rendered_files_json" \
    '{ok: true, operation: "smoke", scripts_dir: $scripts_dir, assets_dir: $assets_dir, render_dir: $render_dir, rendered_files: $rendered_files, notes: "syntax+template smoke only; no prove/sign/broadcast executed"}')"
cast_write_receipt "smoke" "$receipt_json" "${CAST_SMOKE_RECEIPT_FILE:-}" >/dev/null

printf 'CAST smoke check passed. Rendered templates in %s\n' "$render_dir"
printf 'No prove/sign/broadcast operations were executed.\n'
