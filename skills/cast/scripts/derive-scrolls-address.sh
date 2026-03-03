#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/_lib_receipts.sh"

funding_utxo=""
output_index=""
scrolls_base_url=""
receipt_file=""

while [[ $# -gt 0 ]]; do
    case "$1" in
    --funding-utxo)
        funding_utxo="${2:-}"
        shift 2
        ;;
    --output-index)
        output_index="${2:-}"
        shift 2
        ;;
    --scrolls-base-url)
        scrolls_base_url="${2:-}"
        shift 2
        ;;
    --receipt-file)
        receipt_file="${2:-}"
        shift 2
        ;;
    -h | --help)
        printf 'Usage: %s --funding-utxo <txid:vout> --output-index <n> --scrolls-base-url <url> [--receipt-file <path>]\n' "$0"
        exit 0
        ;;
    *)
        printf 'Unknown argument: %s\n' "$1" >&2
        exit 1
        ;;
    esac
done

if [[ -z "$funding_utxo" || -z "$output_index" || -z "$scrolls_base_url" ]]; then
    printf 'Missing required arguments.\n' >&2
    printf 'Usage: %s --funding-utxo <txid:vout> --output-index <n> --scrolls-base-url <url> [--receipt-file <path>]\n' "$0" >&2
    exit 1
fi
if ! [[ "$output_index" =~ ^[0-9]+$ ]]; then
    printf 'output-index must be a non-negative integer: %s\n' "$output_index" >&2
    exit 1
fi

if ! command -v scrolls-nonce >/dev/null 2>&1; then
    printf 'Missing required command: scrolls-nonce\n' >&2
    exit 1
fi
if ! command -v curl >/dev/null 2>&1; then
    printf 'Missing required command: curl\n' >&2
    exit 1
fi
if ! command -v jq >/dev/null 2>&1; then
    printf 'Missing required command: jq\n' >&2
    exit 1
fi

nonce="$(scrolls-nonce "$funding_utxo" "$output_index" | tr -d '[:space:]')"
if [[ -z "$nonce" ]]; then
    printf 'Failed to derive nonce from funding utxo and output index.\n' >&2
    exit 1
fi

response="$(curl -fsS "${scrolls_base_url%/}/address/${nonce}")"
address="$(printf '%s' "$response" | jq -er 'if type == "string" then . elif .address then .address else error("missing address") end' 2>/dev/null || true)"
if [[ -z "$address" ]]; then
    # fallback for plain text responses
    address="$(printf '%s' "$response" | tr -d '"[:space:]')"
fi
if [[ -z "$address" ]]; then
    printf 'Failed to parse Scrolls address from response: %s\n' "$response" >&2
    exit 1
fi

result_json="$(jq -n \
    --arg funding_utxo_id "$funding_utxo" \
    --argjson output_index "$output_index" \
    --arg nonce "$nonce" \
    --arg scrolls_address "$address" \
    --arg scrolls_base_url "$scrolls_base_url" \
    --arg response_sha256 "$(printf '%s' "$response" | shasum -a 256 | awk '{print $1}')" \
    '{ok: true, operation: "derive_scrolls_address", funding_utxo_id: $funding_utxo_id, output_index: $output_index, nonce: $nonce, scrolls_address: $scrolls_address, input_pointers: {scrolls_base_url: $scrolls_base_url}, output_hashes: {scrolls_response_sha256: $response_sha256}}')"

cast_write_receipt "derive_scrolls_address" "$result_json" "$receipt_file"
