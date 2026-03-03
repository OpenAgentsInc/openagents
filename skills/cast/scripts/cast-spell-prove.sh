#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/_lib_receipts.sh"

spell=""
app_bin="${CAST_APP_BIN:-}"
prev_txs_file="${CAST_PREV_TXS_FILE:-}"
private_inputs_file="${CAST_PRIVATE_INPUTS_FILE:-}"
funding_utxo="${CAST_FUNDING_UTXO:-}"
funding_utxo_value="${CAST_FUNDING_UTXO_VALUE:-}"
change_address="${CAST_CHANGE_ADDRESS:-}"
fee_rate="${CAST_FEE_RATE:-2}"
mock=0
output_file=""
receipt_file=""

while [[ $# -gt 0 ]]; do
    case "$1" in
    --spell)
        spell="${2:-}"
        shift 2
        ;;
    --app-bin)
        app_bin="${2:-}"
        shift 2
        ;;
    --prev-txs-file)
        prev_txs_file="${2:-}"
        shift 2
        ;;
    --private-inputs-file)
        private_inputs_file="${2:-}"
        shift 2
        ;;
    --funding-utxo)
        funding_utxo="${2:-}"
        shift 2
        ;;
    --funding-utxo-value)
        funding_utxo_value="${2:-}"
        shift 2
        ;;
    --change-address)
        change_address="${2:-}"
        shift 2
        ;;
    --fee-rate)
        fee_rate="${2:-}"
        shift 2
        ;;
    --output-file)
        output_file="${2:-}"
        shift 2
        ;;
    --receipt-file)
        receipt_file="${2:-}"
        shift 2
        ;;
    --mock)
        mock=1
        shift
        ;;
    -h | --help)
        printf 'Usage: %s --spell <spell.yaml> [--mock] [--output-file <path>] [--receipt-file <path>] [--app-bin <wasm>] [--prev-txs-file <path>] [--funding-utxo <txid:vout>] [--funding-utxo-value <sats>] [--change-address <addr>] [--fee-rate <sat/vb>]\n' "$0"
        exit 0
        ;;
    *)
        printf 'Unknown argument: %s\n' "$1" >&2
        exit 1
        ;;
    esac
done

if [[ -z "$spell" || -z "$app_bin" || -z "$prev_txs_file" ]]; then
    printf 'Missing required inputs (spell, app bin, prev txs file).\n' >&2
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

if [[ ! -f "$spell" ]]; then
    printf 'Spell file not found: %s\n' "$spell" >&2
    exit 1
fi
if [[ ! -f "$app_bin" ]]; then
    printf 'App bin not found: %s\n' "$app_bin" >&2
    exit 1
fi
if [[ ! -f "$prev_txs_file" ]]; then
    printf 'prev_txs file not found: %s\n' "$prev_txs_file" >&2
    exit 1
fi
if [[ -n "$private_inputs_file" && ! -f "$private_inputs_file" ]]; then
    printf 'private inputs file not found: %s\n' "$private_inputs_file" >&2
    exit 1
fi
if ! [[ "$fee_rate" =~ ^[0-9]+([.][0-9]+)?$ ]]; then
    printf 'fee-rate must be numeric (sat/vb): %s\n' "$fee_rate" >&2
    exit 1
fi

if ! cast_verify_app_bin_hash "$app_bin"; then
    exit 1
fi

if [[ "$mock" -eq 0 ]]; then
    if [[ -z "$funding_utxo" || -z "$funding_utxo_value" || -z "$change_address" ]]; then
        printf 'Real prove requires funding_utxo, funding_utxo_value, and change_address.\n' >&2
        exit 1
    fi
    if ! [[ "$funding_utxo_value" =~ ^[0-9]+$ ]]; then
        printf 'funding-utxo-value must be an integer satoshi value: %s\n' "$funding_utxo_value" >&2
        exit 1
    fi
fi

if [[ -z "$output_file" ]]; then
    run_dir="$(cast_run_dir)"
    mkdir -p "${run_dir}/proofs"
    if [[ "$mock" -eq 1 ]]; then
        output_file="${run_dir}/proofs/mock_prove.json"
    else
        output_file="${run_dir}/proofs/prove.json"
    fi
else
    mkdir -p "$(dirname "$output_file")"
fi

prev_txs="$(tr -d '\n' < "$prev_txs_file")"
if [[ -z "$prev_txs" ]]; then
    printf 'prev_txs file is empty: %s\n' "$prev_txs_file" >&2
    exit 1
fi
prev_txs_hash="$(printf '%s' "$prev_txs" | shasum -a 256 | awk '{print $1}')"
spell_hash="$(cast_file_sha256 "$spell")"
app_bin_hash="$(cast_file_sha256 "$app_bin")"
private_inputs_hash=""
if [[ -n "$private_inputs_file" ]]; then
    private_inputs_hash="$(cast_file_sha256 "$private_inputs_file")"
fi

cmd=(charms spell prove --spell "$spell" --app-bins "$app_bin" --prev-txs "$prev_txs" --fee-rate "$fee_rate")
if [[ -n "$private_inputs_file" ]]; then
    cmd+=(--private-inputs "$private_inputs_file")
fi
if [[ "$mock" -eq 1 ]]; then
    cmd+=(--mock)
else
    cmd+=(--funding-utxo "$funding_utxo" --funding-utxo-value "$funding_utxo_value" --change-address "$change_address")
fi

command_output_file="$(mktemp)"
if ! "${cmd[@]}" >"$command_output_file" 2>&1; then
    cat "$command_output_file" >&2
    exit 1
fi

cat "$command_output_file" > "$output_file"

if ! jq empty "$output_file" >/dev/null 2>&1; then
    printf 'prove output is not valid JSON: %s\n' "$output_file" >&2
    exit 1
fi

result_json="$(jq -n \
    --arg spell "$spell" \
    --arg spell_hash "$spell_hash" \
    --arg network "${CAST_NETWORK:-mainnet}" \
    --arg app_bin "$app_bin" \
    --arg app_bin_hash "$app_bin_hash" \
    --arg prev_txs_file "$prev_txs_file" \
    --arg prev_txs_hash "$prev_txs_hash" \
    --arg private_inputs_file "$private_inputs_file" \
    --arg private_inputs_hash "$private_inputs_hash" \
    --arg output_file "$output_file" \
    --arg output_file_hash "$(cast_file_sha256 "$output_file")" \
    --arg funding_utxo "$funding_utxo" \
    --arg funding_utxo_value "$funding_utxo_value" \
    --arg change_address "$change_address" \
    --arg fee_rate "$fee_rate" \
    --argjson mock "$mock" \
    '{
      ok: true,
      operation: "spell_prove",
      network: $network,
      mock: ($mock == 1),
      spell: $spell,
      app_bin: $app_bin,
      prev_txs_file: $prev_txs_file,
      prev_txs_hash: $prev_txs_hash,
      output_file: $output_file,
      funding_utxo: $funding_utxo,
      funding_utxo_value: $funding_utxo_value,
      change_address: $change_address,
      fee_rate: $fee_rate,
      input_pointers: {
        spell_file: $spell,
        app_bin_file: $app_bin,
        prev_txs_file: $prev_txs_file,
        private_inputs_file: $private_inputs_file
      },
      input_hashes: {
        spell_sha256: $spell_hash,
        app_bin_sha256: $app_bin_hash,
        prev_txs_sha256: $prev_txs_hash,
        private_inputs_sha256: $private_inputs_hash
      },
      output_pointers: {
        prove_json_file: $output_file
      },
      output_hashes: {
        prove_json_sha256: $output_file_hash
      }
    }')"

cast_write_receipt "spell_prove" "$result_json" "$receipt_file"
