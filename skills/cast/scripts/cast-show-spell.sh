#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/_lib_receipts.sh"

tx_hex=""
tx_file=""
chain="${CAST_CHAIN:-bitcoin}"
receipt_file=""

while [[ $# -gt 0 ]]; do
    case "$1" in
    --tx)
        tx_hex="${2:-}"
        shift 2
        ;;
    --tx-file)
        tx_file="${2:-}"
        shift 2
        ;;
    --chain)
        chain="${2:-}"
        shift 2
        ;;
    --receipt-file)
        receipt_file="${2:-}"
        shift 2
        ;;
    -h | --help)
        printf 'Usage: %s (--tx <hex> | --tx-file <path>) [--chain <bitcoin|testnet4|...>] [--receipt-file <path>]\n' "$0"
        exit 0
        ;;
    *)
        printf 'Unknown argument: %s\n' "$1" >&2
        exit 1
        ;;
    esac
done

if [[ -n "$tx_hex" && -n "$tx_file" ]]; then
    printf 'Use either --tx or --tx-file, not both.\n' >&2
    exit 1
fi
if [[ -z "$tx_hex" && -z "$tx_file" ]]; then
    printf 'Missing tx input. Provide --tx or --tx-file.\n' >&2
    exit 1
fi

if ! command -v charms >/dev/null 2>&1; then
    printf 'Missing required command: charms\n' >&2
    exit 1
fi
if ! command -v jq >/dev/null 2>&1; then
    printf 'Missing required command: jq\n' >&2
    exit 1
fi

if [[ -n "$tx_file" ]]; then
    if [[ ! -f "$tx_file" ]]; then
        printf 'tx file not found: %s\n' "$tx_file" >&2
        exit 1
    fi
    tx_hex="$(tr -d '\r\n' < "$tx_file")"
fi

if [[ -z "$tx_hex" ]]; then
    printf 'Transaction hex is empty.\n' >&2
    exit 1
fi

spell_json="$(charms tx show-spell --chain "$chain" --tx "$tx_hex" --json)"

result_json="$(jq -n \
    --arg operation "show_spell" \
    --arg chain "$chain" \
    --arg tx_file "$tx_file" \
    --arg tx_hex "$tx_hex" \
    --arg tx_hex_sha256 "$(printf '%s' "$tx_hex" | shasum -a 256 | awk '{print $1}')" \
    --arg spell_decode_summary_sha256 "$(printf '%s' "$spell_json" | shasum -a 256 | awk '{print $1}')" \
    --argjson spell_decode_summary "$spell_json" \
    '{ok: true, operation: $operation, chain: $chain, tx_file: $tx_file, tx_hex: $tx_hex, spell_decode_summary: $spell_decode_summary, input_pointers: {tx_file: $tx_file}, input_hashes: {tx_hex_sha256: $tx_hex_sha256}, output_hashes: {spell_decode_summary_sha256: $spell_decode_summary_sha256}}')"

cast_write_receipt "show_spell" "$result_json" "$receipt_file"
