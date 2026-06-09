#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/_lib_receipts.sh"

spell=""
app_bin="${CAST_APP_BIN:-}"
prev_txs_file="${CAST_PREV_TXS_FILE:-}"
private_inputs_file="${CAST_PRIVATE_INPUTS_FILE:-}"
change_address="${CAST_CHANGE_ADDRESS:-}"
fee_rate="${CAST_FEE_RATE:-2}"
chain="${CAST_CHAIN:-bitcoin}"
beamed_from="${CAST_BEAMED_FROM:-}"
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
    --change-address)
        change_address="${2:-}"
        shift 2
        ;;
    --fee-rate)
        fee_rate="${2:-}"
        shift 2
        ;;
    --chain)
        chain="${2:-}"
        shift 2
        ;;
    --beamed-from)
        beamed_from="${2:-}"
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
        printf 'Usage: %s --spell <spell.yaml> [--mock] [--output-file <path>] [--receipt-file <path>] [--app-bin <wasm>] [--prev-txs-file <path>] [--private-inputs-file <path>] [--change-address <addr>] [--fee-rate <sat/vb>] [--chain <bitcoin|cardano>] [--beamed-from <yaml/json-map>]\n' "$0"
        exit 0
        ;;
    *)
        printf 'Unknown argument: %s\n' "$1" >&2
        exit 1
        ;;
    esac
done

if [[ -z "$spell" || -z "$app_bin" || -z "$prev_txs_file" || -z "$change_address" ]]; then
    printf 'Missing required inputs (spell, app bin, prev txs file, change address).\n' >&2
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

spell_for_prove="$spell"
if [[ "$mock" -eq 1 ]]; then
    spell_for_prove="$(mktemp)"
    if grep -qE '^mock:[[:space:]]*(true|false)[[:space:]]*$' "$spell"; then
        sed -E 's/^mock:[[:space:]]*(true|false)[[:space:]]*$/mock: true/' "$spell" > "$spell_for_prove"
    else
        cat "$spell" > "$spell_for_prove"
        printf '\nmock: true\n' >> "$spell_for_prove"
    fi
fi

prev_txs_lines=()
while IFS= read -r line || [[ -n "$line" ]]; do
    line="${line//$'\r'/}"
    if [[ "$line" =~ ^[[:space:]]*$ ]]; then
        continue
    fi
    prev_txs_lines+=("$line")
done < "$prev_txs_file"
if [[ "${#prev_txs_lines[@]}" -eq 0 ]]; then
    printf 'prev_txs file is empty: %s\n' "$prev_txs_file" >&2
    exit 1
fi
if [[ "${#prev_txs_lines[@]}" -eq 1 && "${prev_txs_lines[0]}" == *,* ]]; then
    IFS=',' read -r -a prev_txs_items <<<"${prev_txs_lines[0]}"
else
    prev_txs_items=("${prev_txs_lines[@]}")
fi

prev_txs_values=()
for item in "${prev_txs_items[@]}"; do
    trimmed="$(printf '%s' "$item" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')"
    if [[ -n "$trimmed" ]]; then
        prev_txs_values+=("$trimmed")
    fi
done
if [[ "${#prev_txs_values[@]}" -eq 0 ]]; then
    printf 'prev_txs file has no usable entries: %s\n' "$prev_txs_file" >&2
    exit 1
fi
prev_txs_joined="$(printf '%s,' "${prev_txs_values[@]}")"
prev_txs_joined="${prev_txs_joined%,}"
prev_txs_hash="$(printf '%s' "$prev_txs_joined" | shasum -a 256 | awk '{print $1}')"
spell_hash="$(cast_file_sha256 "$spell_for_prove")"
app_bin_hash="$(cast_file_sha256 "$app_bin")"
private_inputs_hash=""
if [[ -n "$private_inputs_file" ]]; then
    private_inputs_hash="$(cast_file_sha256 "$private_inputs_file")"
fi

cmd=(charms spell prove --spell "$spell_for_prove" --app-bins "$app_bin" --change-address "$change_address" --fee-rate "$fee_rate" --chain "$chain")
for prev_tx in "${prev_txs_values[@]}"; do
    cmd+=(--prev-txs "$prev_tx")
done
if [[ -n "$private_inputs_file" ]]; then
    cmd+=(--private-inputs "$private_inputs_file")
fi
if [[ -n "$beamed_from" ]]; then
    cmd+=(--beamed-from "$beamed_from")
fi
if [[ "$mock" -eq 1 ]]; then
    cmd+=(--mock)
fi

command_output_file="$(mktemp)"
command_stderr_file="$(mktemp)"
if ! "${cmd[@]}" >"$command_output_file" 2>"$command_stderr_file"; then
    command_log_file="$(mktemp)"
    cat "$command_stderr_file" "$command_output_file" >"$command_log_file"
    cat "$command_stderr_file" >&2
    cat "$command_output_file" >&2
    cast_print_spell_failure_hints "$command_log_file"
    exit 1
fi
if [[ -s "$command_stderr_file" ]]; then
    cat "$command_stderr_file" >&2
fi

cat "$command_output_file" > "$output_file"

if ! jq empty "$output_file" >/dev/null 2>&1; then
    printf 'prove output is not valid JSON: %s\n' "$output_file" >&2
    exit 1
fi

result_json="$(jq -n \
    --arg spell "$spell" \
    --arg spell_for_prove "$spell_for_prove" \
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
    --arg change_address "$change_address" \
    --arg fee_rate "$fee_rate" \
    --arg chain "$chain" \
    --arg beamed_from "$beamed_from" \
    --argjson mock "$mock" \
    '{
      ok: true,
      operation: "spell_prove",
      network: $network,
      mock: ($mock == 1),
      chain: $chain,
      spell: $spell,
      effective_spell: $spell_for_prove,
      app_bin: $app_bin,
      prev_txs_file: $prev_txs_file,
      prev_txs_hash: $prev_txs_hash,
      output_file: $output_file,
      change_address: $change_address,
      fee_rate: $fee_rate,
      beamed_from: $beamed_from,
      input_pointers: {
        spell_file: $spell,
        effective_spell_file: $spell_for_prove,
        app_bin_file: $app_bin,
        prev_txs_file: $prev_txs_file,
        private_inputs_file: $private_inputs_file,
        beamed_from: $beamed_from
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
