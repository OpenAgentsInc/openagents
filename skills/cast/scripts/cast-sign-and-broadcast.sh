#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/_lib_receipts.sh"

tx_json=""
signed_output_file=""
scrolls_sign_request_file=""
scrolls_output_file=""
prev_txs_file="${CAST_PREV_TXS_FILE:-}"
dry_run=0
yes_broadcast=0
broadcast_url="${CAST_MEMPOOL_BROADCAST_URL:-}"
scrolls_base_url="${CAST_SCROLLS_BASE_URL:-}"
receipt_file=""

while [[ $# -gt 0 ]]; do
    case "$1" in
    --tx-json)
        tx_json="${2:-}"
        shift 2
        ;;
    --signed-output-file)
        signed_output_file="${2:-}"
        shift 2
        ;;
    --scrolls-sign-request)
        scrolls_sign_request_file="${2:-}"
        shift 2
        ;;
    --scrolls-output-file)
        scrolls_output_file="${2:-}"
        shift 2
        ;;
    --prev-txs-file)
        prev_txs_file="${2:-}"
        shift 2
        ;;
    --broadcast-url)
        broadcast_url="${2:-}"
        shift 2
        ;;
    --receipt-file)
        receipt_file="${2:-}"
        shift 2
        ;;
    --dry-run)
        dry_run=1
        shift
        ;;
    --yes-broadcast)
        yes_broadcast=1
        shift
        ;;
    -h | --help)
        printf 'Usage: %s --tx-json <prove_tx_json> [--signed-output-file <path>] [--scrolls-sign-request <path>] [--scrolls-output-file <path>] [--prev-txs-file <path>] [--broadcast-url <url>] [--receipt-file <path>] [--dry-run] [--yes-broadcast]\n' "$0"
        exit 0
        ;;
    *)
        printf 'Unknown argument: %s\n' "$1" >&2
        exit 1
        ;;
    esac
done

if [[ -z "$tx_json" ]]; then
    printf 'Missing required argument: --tx-json\n' >&2
    exit 1
fi

if ! command -v sign-txs >/dev/null 2>&1; then
    printf 'Missing required command: sign-txs\n' >&2
    exit 1
fi
if ! command -v jq >/dev/null 2>&1; then
    printf 'Missing required command: jq\n' >&2
    exit 1
fi
if ! command -v curl >/dev/null 2>&1; then
    printf 'Missing required command: curl\n' >&2
    exit 1
fi

if [[ ! -f "$tx_json" ]]; then
    printf 'tx json file not found: %s\n' "$tx_json" >&2
    exit 1
fi
if [[ -n "$prev_txs_file" && ! -f "$prev_txs_file" ]]; then
    printf 'prev_txs file not found: %s\n' "$prev_txs_file" >&2
    exit 1
fi
if [[ -n "$scrolls_sign_request_file" && ! -f "$scrolls_sign_request_file" ]]; then
    printf 'Scrolls sign request file not found: %s\n' "$scrolls_sign_request_file" >&2
    exit 1
fi

if [[ -z "$signed_output_file" ]]; then
    run_dir="$(cast_run_dir)"
    mkdir -p "${run_dir}/signed"
    base_name="$(basename "$tx_json" .json)"
    signed_output_file="${run_dir}/signed/${base_name}.signed.json"
fi
mkdir -p "$(dirname "$signed_output_file")"

load_prev_txs_values() {
    local path="$1"
    local lines=()
    while IFS= read -r line || [[ -n "$line" ]]; do
        line="${line//$'\r'/}"
        if [[ "$line" =~ ^[[:space:]]*$ ]]; then
            continue
        fi
        lines+=("$line")
    done < "$path"

    local items=()
    if [[ "${#lines[@]}" -eq 1 && "${lines[0]}" == *,* ]]; then
        IFS=',' read -r -a items <<<"${lines[0]}"
    else
        items=("${lines[@]}")
    fi

    local value trimmed
    for value in "${items[@]}"; do
        trimmed="$(printf '%s' "$value" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')"
        if [[ -n "$trimmed" ]]; then
            printf '%s\n' "$trimmed"
        fi
    done
}

sign_with_wallet_prev_txs() {
    local tx_json_file="$1"
    local prev_file="$2"
    local signed_file="$3"
    local wallet="${CAST_BITCOIN_WALLET:-}"

    if ! command -v bitcoin-cli >/dev/null 2>&1; then
        printf 'Fallback signing requires bitcoin-cli in PATH.\n' >&2
        return 1
    fi

    local prev_values_file
    prev_values_file="$(mktemp)"
    load_prev_txs_values "$prev_file" > "$prev_values_file"
    if [[ ! -s "$prev_values_file" ]]; then
        printf 'Fallback signing failed: prev_txs file has no usable entries: %s\n' "$prev_file" >&2
        return 1
    fi

    local prev_decoded_file
    prev_decoded_file="$(mktemp)"
    : > "$prev_decoded_file"
    while IFS= read -r prev_hex; do
        decoded_prev="$(bitcoin-cli decoderawtransaction "$prev_hex" 2>/dev/null || true)"
        if [[ -z "$decoded_prev" ]]; then
            printf 'Fallback signing failed: cannot decode prev tx hex entry.\n' >&2
            return 1
        fi
        printf '%s\n' "$decoded_prev" >> "$prev_decoded_file"
    done < "$prev_values_file"

    local tx_count
    tx_count="$(jq 'if type=="array" then length else 0 end' "$tx_json_file")"
    if [[ "$tx_count" -le 0 ]]; then
        printf 'Fallback signing failed: tx json must be a non-empty array.\n' >&2
        return 1
    fi

    local signed_lines_file
    signed_lines_file="$(mktemp)"
    : > "$signed_lines_file"

    local i tx_hex decoded_tx vins prevouts sign_result signed_hex
    for ((i = 0; i < tx_count; i++)); do
        tx_hex="$(jq -r ".[$i].bitcoin // empty" "$tx_json_file")"
        if [[ -z "$tx_hex" ]]; then
            printf 'Fallback signing failed: missing .bitcoin for transaction index %s\n' "$i" >&2
            return 1
        fi

        decoded_tx="$(bitcoin-cli decoderawtransaction "$tx_hex" 2>/dev/null || true)"
        if [[ -z "$decoded_tx" ]]; then
            printf 'Fallback signing failed: cannot decode tx index %s\n' "$i" >&2
            return 1
        fi
        vins="$(jq -c '.vin // [] | map(select((.txinwitness // null) == null) | {txid, vout})' <<<"$decoded_tx")"
        prevouts="$(jq -s --argjson vins "$vins" '
            [ $vins[] as $in
              | .[]
              | select(.txid == $in.txid)
              | .vout[]
              | select(.n == $in.vout)
              | {txid: $in.txid, vout: $in.vout, amount: .value, scriptPubKey: .scriptPubKey.hex}
            ]' "$prev_decoded_file")"

        if [[ -n "$wallet" ]]; then
            sign_result="$(bitcoin-cli -rpcwallet="$wallet" signrawtransactionwithwallet "$tx_hex" "$prevouts" 2>/dev/null || true)"
        else
            sign_result="$(bitcoin-cli signrawtransactionwithwallet "$tx_hex" "$prevouts" 2>/dev/null || true)"
        fi
        if [[ -z "$sign_result" ]]; then
            printf 'Fallback signing failed: signrawtransactionwithwallet returned no output for tx index %s\n' "$i" >&2
            return 1
        fi
        signed_hex="$(jq -r '.hex // empty' <<<"$sign_result")"
        if [[ -z "$signed_hex" ]]; then
            printf 'Fallback signing failed: missing signed hex for tx index %s\n' "$i" >&2
            return 1
        fi
        printf '%s\n' "$signed_hex" >> "$signed_lines_file"
    done

    jq -Rsc 'split("\n") | map(select(length > 0) | {bitcoin: .})' "$signed_lines_file" > "$signed_file"
    return 0
}

signer_mode="sign_txs"
sign_stderr_file="$(mktemp)"
if ! sign-txs "$tx_json" > "$signed_output_file" 2>"$sign_stderr_file"; then
    cat "$sign_stderr_file" >&2
    if [[ -n "$prev_txs_file" ]]; then
        printf 'sign-txs failed; attempting wallet fallback with prev_txs file: %s\n' "$prev_txs_file" >&2
        if ! sign_with_wallet_prev_txs "$tx_json" "$prev_txs_file" "$signed_output_file"; then
            exit 1
        fi
        signer_mode="wallet_prev_txs_fallback"
    else
        exit 1
    fi
fi

if ! jq empty "$signed_output_file" >/dev/null 2>&1; then
    printf 'signed output is not valid JSON: %s\n' "$signed_output_file" >&2
    exit 1
fi

if [[ -n "$scrolls_sign_request_file" ]]; then
    if [[ -z "$scrolls_base_url" ]]; then
        printf 'CAST_SCROLLS_BASE_URL is required when --scrolls-sign-request is used.\n' >&2
        exit 1
    fi
    if [[ -z "$scrolls_output_file" ]]; then
        scrolls_output_file="${signed_output_file%.json}.scrolls.json"
    fi
    mkdir -p "$(dirname "$scrolls_output_file")"
    curl -fsS -X POST "${scrolls_base_url%/}/sign" \
        -H 'Content-Type: application/json' \
        --data "@${scrolls_sign_request_file}" > "$scrolls_output_file"
fi

hex_file="$(mktemp)"
jq -r 'if type=="array" then .[] | .bitcoin // empty elif type=="object" then .bitcoin // empty else empty end' "$signed_output_file" | sed '/^$/d' > "$hex_file"
hex_count="$(wc -l < "$hex_file" | tr -d '[:space:]')"
if [[ "$hex_count" == "0" ]]; then
    printf 'No signed bitcoin tx hex values found in: %s\n' "$signed_output_file" >&2
    exit 1
fi

local_txids_file="$(mktemp)"
while IFS= read -r tx_hex; do
    local_txid=""
    if command -v bitcoin-cli >/dev/null 2>&1; then
        local_txid="$(bitcoin-cli decoderawtransaction "$tx_hex" 2>/dev/null | jq -r '.txid // empty' || true)"
    fi
    printf '%s\n' "$local_txid" >> "$local_txids_file"
done < "$hex_file"

broadcast_results_file=""
if [[ "$dry_run" -eq 0 ]]; then
    if [[ "$yes_broadcast" -ne 1 ]]; then
        printf 'Broadcast requires explicit confirmation: pass --yes-broadcast (or use --dry-run).\n' >&2
        exit 1
    fi
    if [[ -z "$broadcast_url" ]]; then
        printf 'Missing broadcast URL. Set CAST_MEMPOOL_BROADCAST_URL or pass --broadcast-url.\n' >&2
        exit 1
    fi

    broadcast_results_file="$(mktemp)"
    : > "$broadcast_results_file"
    while IFS= read -r tx_hex; do
        response="$(curl -fsS -X POST "$broadcast_url" --data "$tx_hex")"
        printf '%s\n' "$response" >> "$broadcast_results_file"
    done < "$hex_file"
fi

signed_tx_hex_json="$(jq -Rsc 'split("\n") | map(select(length > 0))' "$hex_file")"
local_txids_json="$(jq -Rsc 'split("\n") | map(select(length > 0))' "$local_txids_file")"
if [[ -n "$broadcast_results_file" ]]; then
    broadcast_txids_json="$(jq -Rsc 'split("\n") | map(select(length > 0))' "$broadcast_results_file")"
else
    broadcast_txids_json='[]'
fi
scrolls_output_hash=""
if [[ -n "$scrolls_output_file" && -f "$scrolls_output_file" ]]; then
    scrolls_output_hash="$(cast_file_sha256 "$scrolls_output_file")"
fi

result_json="$(jq -n \
    --arg tx_json "$tx_json" \
    --arg tx_json_hash "$(cast_file_sha256 "$tx_json")" \
    --arg network "${CAST_NETWORK:-mainnet}" \
    --arg signer_mode "$signer_mode" \
    --arg prev_txs_file "$prev_txs_file" \
    --arg prev_txs_hash "$(cast_file_sha256 "$prev_txs_file")" \
    --arg signed_output_file "$signed_output_file" \
    --arg signed_output_hash "$(cast_file_sha256 "$signed_output_file")" \
    --arg scrolls_sign_request_file "$scrolls_sign_request_file" \
    --arg scrolls_sign_request_hash "$(cast_file_sha256 "$scrolls_sign_request_file")" \
    --arg scrolls_output_file "$scrolls_output_file" \
    --arg scrolls_output_hash "$scrolls_output_hash" \
    --argjson dry_run "$dry_run" \
    --argjson signed_tx_hex "$signed_tx_hex_json" \
    --argjson local_txids "$local_txids_json" \
    --argjson broadcast_txids "$broadcast_txids_json" \
    '{
      ok: true,
      operation: "sign_and_broadcast",
      network: $network,
      dry_run: ($dry_run == 1),
      signer_mode: $signer_mode,
      tx_json: $tx_json,
      prev_txs_file: $prev_txs_file,
      signed_output_file: $signed_output_file,
      scrolls_sign_request_file: $scrolls_sign_request_file,
      scrolls_output_file: $scrolls_output_file,
      signed_tx_hex: $signed_tx_hex,
      local_txids: $local_txids,
      broadcast_txids: $broadcast_txids,
      input_pointers: {
        tx_json_file: $tx_json,
        prev_txs_file: $prev_txs_file,
        scrolls_sign_request_file: $scrolls_sign_request_file
      },
      input_hashes: {
        tx_json_sha256: $tx_json_hash,
        prev_txs_sha256: $prev_txs_hash,
        scrolls_sign_request_sha256: $scrolls_sign_request_hash
      },
      output_pointers: {
        signed_output_file: $signed_output_file,
        scrolls_output_file: $scrolls_output_file
      },
      output_hashes: {
        signed_output_sha256: $signed_output_hash,
        scrolls_output_sha256: $scrolls_output_hash
      }
    }')"

cast_write_receipt "sign_and_broadcast" "$result_json" "$receipt_file"
