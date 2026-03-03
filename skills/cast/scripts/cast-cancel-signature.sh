#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/_lib_receipts.sh"

spell=""
cancel_utxo=""
message_override=""
xprv_file="${CAST_CANCEL_XPRV_FILE:-}"
derivation_path="${CAST_CANCEL_DERIVATION_PATH:-}"
receipt_file=""

while [[ $# -gt 0 ]]; do
    case "$1" in
    --spell)
        spell="${2:-}"
        shift 2
        ;;
    --cancel-utxo)
        cancel_utxo="${2:-}"
        shift 2
        ;;
    --message)
        message_override="${2:-}"
        shift 2
        ;;
    --xprv-file)
        xprv_file="${2:-}"
        shift 2
        ;;
    --path)
        derivation_path="${2:-}"
        shift 2
        ;;
    --receipt-file)
        receipt_file="${2:-}"
        shift 2
        ;;
    -h | --help)
        printf 'Usage: %s --spell <spell.yaml> --cancel-utxo <txid:vout> --xprv-file <path> --path <derivation-path> [--message "<utxo_id outputs_hash>"] [--receipt-file <path>]\n' "$0"
        exit 0
        ;;
    *)
        printf 'Unknown argument: %s\n' "$1" >&2
        exit 1
        ;;
    esac
done

if [[ -z "$spell" || -z "$cancel_utxo" || -z "$xprv_file" || -z "$derivation_path" ]]; then
    printf 'Missing required arguments.\n' >&2
    exit 1
fi

if ! command -v cancel-msg >/dev/null 2>&1; then
    printf 'Missing required command: cancel-msg\n' >&2
    exit 1
fi
if ! command -v jq >/dev/null 2>&1; then
    printf 'Missing required command: jq\n' >&2
    exit 1
fi

if [[ ! -f "$spell" ]]; then
    printf 'Spell file not found: %s\n' "$spell" >&2
    exit 1
fi
if [[ ! -f "$xprv_file" ]]; then
    printf 'xprv file not found: %s\n' "$xprv_file" >&2
    exit 1
fi

xprv="$(tr -d '\r\n' < "$xprv_file")"
if [[ -z "$xprv" ]]; then
    printf 'xprv file is empty: %s\n' "$xprv_file" >&2
    exit 1
fi

if [[ -n "$message_override" ]]; then
    message="$message_override"
else
    message_file="$(mktemp)"
    message_err_file="$(mktemp)"
    if ! cancel-msg message "$spell" "$cancel_utxo" >"$message_file" 2>"$message_err_file"; then
        cat "$message_err_file" >&2
        printf 'Failed to derive cancellation message from spell.\n' >&2
        printf 'If your cancel-msg build does not support Charms v11 spell format, pass --message "<utxo_id outputs_hash>" explicitly.\n' >&2
        exit 1
    fi
    message="$(tr -d '\r\n' < "$message_file")"
    if [[ -z "$message" ]]; then
        printf 'Failed to generate cancellation message.\n' >&2
        exit 1
    fi
fi

read -r message_utxo outputs_hash trailing <<<"$message"
if [[ -z "${message_utxo:-}" || -z "${outputs_hash:-}" || -n "${trailing:-}" ]]; then
    printf 'Cancellation message must match "{utxo_id} {outputs_hash}": %s\n' "$message" >&2
    exit 1
fi
if [[ "$message_utxo" != "$cancel_utxo" ]]; then
    printf 'Cancellation message utxo mismatch. Expected %s, got %s.\n' "$cancel_utxo" "$message_utxo" >&2
    exit 1
fi
if ! [[ "$outputs_hash" =~ ^[0-9a-fA-F]{64}$ ]]; then
    printf 'Cancellation outputs_hash must be 32-byte hex: %s\n' "$outputs_hash" >&2
    exit 1
fi

sign_output_file="$(mktemp)"
if ! cancel-msg sign --xprv "$xprv" --path "$derivation_path" "$message" >"$sign_output_file" 2>&1; then
    cat "$sign_output_file" >&2
    printf 'Failed to generate cancellation signature.\n' >&2
    exit 1
fi

signature="$(awk '/^Signature \(hex\): /{print $3}' "$sign_output_file" | tail -n 1)"
if [[ -z "$signature" ]]; then
    signature="$(grep -Eo '[0-9a-fA-F]{130}' "$sign_output_file" | tail -n 1 || true)"
fi
if [[ -z "$signature" ]]; then
    printf 'Failed to generate cancellation signature.\n' >&2
    exit 1
fi
if ! [[ "$signature" =~ ^[0-9a-fA-F]{130}$ ]]; then
    printf 'Cancellation signature must be 65-byte compact signature hex (130 chars).\n' >&2
    printf 'Raw cancel-msg output:\n' >&2
    cat "$sign_output_file" >&2
    exit 1
fi

result_json="$(jq -n \
    --arg spell "$spell" \
    --arg spell_hash "$(cast_file_sha256 "$spell")" \
    --arg cancel_utxo "$cancel_utxo" \
    --arg message "$message" \
    --arg outputs_hash "$outputs_hash" \
    --arg signature "$signature" \
    --arg signature_hash "$(printf '%s' "$signature" | shasum -a 256 | awk '{print $1}')" \
    --arg derivation_path "$derivation_path" \
    --arg xprv_file "$xprv_file" \
    '{ok: true, operation: "cancel_signature", spell: $spell, cancel_utxo: $cancel_utxo, message: $message, outputs_hash: $outputs_hash, signature: $signature, signature_hash: $signature_hash, derivation_path: $derivation_path, input_pointers: {spell_file: $spell, xprv_file: $xprv_file}, input_hashes: {spell_sha256: $spell_hash}}')"

cast_write_receipt "cancel_signature" "$result_json" "$receipt_file"
