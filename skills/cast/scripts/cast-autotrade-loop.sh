#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/_lib_receipts.sh"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

config_file=""
summary_file=""
stages_csv=""
run_root=""
interval_seconds=""
max_iterations=""
continue_on_error=""
once=0
broadcast=0
mock_prove=""

usage() {
    cat <<EOF
Usage: $0 [options]

Options:
  --config <file>            Source a shell env file before each run.
  --summary-file <path>      Write latest iteration summary JSON to a stable path.
  --stages <csv>             Stage list (default: check,prove,sign,inspect).
  --run-root <dir>           Root directory for per-iteration artifacts.
  --interval-seconds <n>     Sleep interval between loop iterations.
  --max-iterations <n>       0 means unlimited.
  --once                     Run one iteration and exit.
  --broadcast                Enable live broadcast in sign stage.
  --mock-prove               Force --mock prove mode (default).
  --real-prove               Disable --mock prove mode.
  --continue-on-error        Continue looping after stage errors.
  -h, --help                 Show this help.

Required env (for check/prove stages):
  CAST_AUTOTRADE_SPELL_FILE
  CAST_APP_BIN
  CAST_PREV_TXS_FILE
  CAST_CHANGE_ADDRESS

Optional env:
  CAST_AUTOTRADE_PRIVATE_INPUTS_FILE
  CAST_AUTOTRADE_TX_JSON             # when skipping prove or overriding prove output
  CAST_AUTOTRADE_TX_HEX              # when inspect stage should decode explicit tx
  CAST_CHAIN                         # defaults to bitcoin for inspect
  CAST_AUTOTRADE_MOCK_PROVE          # 1 (default) or 0
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
    --config)
        config_file="${2:-}"
        shift 2
        ;;
    --summary-file)
        summary_file="${2:-}"
        shift 2
        ;;
    --stages)
        stages_csv="${2:-}"
        shift 2
        ;;
    --run-root)
        run_root="${2:-}"
        shift 2
        ;;
    --interval-seconds)
        interval_seconds="${2:-}"
        shift 2
        ;;
    --max-iterations)
        max_iterations="${2:-}"
        shift 2
        ;;
    --once)
        once=1
        shift
        ;;
    --broadcast)
        broadcast=1
        shift
        ;;
    --mock-prove)
        mock_prove=1
        shift
        ;;
    --real-prove)
        mock_prove=0
        shift
        ;;
    --continue-on-error)
        continue_on_error=1
        shift
        ;;
    -h | --help)
        usage
        exit 0
        ;;
    *)
        printf 'Unknown argument: %s\n' "$1" >&2
        usage >&2
        exit 1
        ;;
    esac
done

source_config() {
    if [[ -z "$config_file" ]]; then
        return 0
    fi
    if [[ ! -f "$config_file" ]]; then
        printf 'config file not found: %s\n' "$config_file" >&2
        return 1
    fi
    # shellcheck source=/dev/null
    source "$config_file"
}

if ! source_config; then
    exit 1
fi

: "${stages_csv:=${CAST_AUTOTRADE_STAGES:-check,prove,sign,inspect}}"
: "${run_root:=${CAST_AUTOTRADE_RUN_ROOT:-run/autotrade}}"
: "${interval_seconds:=${CAST_AUTOTRADE_INTERVAL_SECONDS:-30}}"
: "${max_iterations:=${CAST_AUTOTRADE_MAX_ITERATIONS:-0}}"
: "${continue_on_error:=${CAST_AUTOTRADE_CONTINUE_ON_ERROR:-0}}"
: "${mock_prove:=${CAST_AUTOTRADE_MOCK_PROVE:-1}}"

if ! [[ "$interval_seconds" =~ ^[0-9]+$ ]]; then
    printf 'interval-seconds must be a non-negative integer: %s\n' "$interval_seconds" >&2
    exit 1
fi
if ! [[ "$max_iterations" =~ ^[0-9]+$ ]]; then
    printf 'max-iterations must be a non-negative integer: %s\n' "$max_iterations" >&2
    exit 1
fi
if ! [[ "$continue_on_error" =~ ^[01]$ ]]; then
    printf 'continue-on-error must be 0 or 1: %s\n' "$continue_on_error" >&2
    exit 1
fi
if ! [[ "$mock_prove" =~ ^[01]$ ]]; then
    printf 'mock-prove must be 0 or 1: %s\n' "$mock_prove" >&2
    exit 1
fi

IFS=',' read -r -a stages_raw <<<"$stages_csv"
stages=()
for stage in "${stages_raw[@]}"; do
    trimmed="$(printf '%s' "$stage" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')"
    if [[ -n "$trimmed" ]]; then
        case "$trimmed" in
        check | prove | sign | inspect)
            stages+=("$trimmed")
            ;;
        *)
            printf 'Unsupported stage in --stages: %s\n' "$trimmed" >&2
            exit 1
            ;;
        esac
    fi
done
if [[ "${#stages[@]}" -eq 0 ]]; then
    printf 'No stages configured.\n' >&2
    exit 1
fi

require_env_for_prove() {
    local key="$1"
    local value="${!key:-}"
    if [[ -z "$value" ]]; then
        printf 'Missing required env: %s\n' "$key" >&2
        return 1
    fi
    return 0
}

run_iteration() {
    local iteration="$1"
    local ts
    local iteration_dir
    local summary_receipt
    local overall_ok=1
    local failed_stage=""
    local failed_message=""
    local prove_output=""
    local tx_json=""
    local inspect_tx_hex=""
    local stage_details_json='[]'

    if ! source_config; then
        return 1
    fi

    ts="$(date -u +%Y%m%dT%H%M%SZ)"
    iteration_dir="${run_root}/${ts}-${iteration}"
    mkdir -p "$iteration_dir/inputs" "$iteration_dir/rendered" "$iteration_dir/proofs" \
        "$iteration_dir/signed" "$iteration_dir/receipts"

    export CAST_RUN_DIR="$iteration_dir"
    summary_receipt="${iteration_dir}/receipts/autotrade_loop.json"
    prove_output="${iteration_dir}/proofs/prove.json"

    for stage in "${stages[@]}"; do
        local stage_receipt="${iteration_dir}/receipts/${stage}.json"
        local stage_ok=1
        local stage_error=""
        local stage_started
        local stage_finished
        stage_started="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

        case "$stage" in
        check)
            if ! require_env_for_prove "CAST_AUTOTRADE_SPELL_FILE" ||
                ! require_env_for_prove "CAST_APP_BIN" ||
                ! require_env_for_prove "CAST_PREV_TXS_FILE"; then
                stage_ok=0
                stage_error="missing required env for check stage"
            else
                cmd=(
                    "${SCRIPT_DIR}/cast-spell-check.sh"
                    --spell "${CAST_AUTOTRADE_SPELL_FILE}"
                    --app-bin "${CAST_APP_BIN}"
                    --prev-txs-file "${CAST_PREV_TXS_FILE}"
                    --receipt-file "$stage_receipt"
                )
                if [[ -n "${CAST_AUTOTRADE_PRIVATE_INPUTS_FILE:-}" ]]; then
                    cmd+=(--private-inputs-file "${CAST_AUTOTRADE_PRIVATE_INPUTS_FILE}")
                fi
                if ! "${cmd[@]}" >/dev/null; then
                    stage_ok=0
                    stage_error="cast-spell-check stage failed"
                fi
            fi
            ;;
        prove)
            if ! require_env_for_prove "CAST_AUTOTRADE_SPELL_FILE" ||
                ! require_env_for_prove "CAST_APP_BIN" ||
                ! require_env_for_prove "CAST_PREV_TXS_FILE" ||
                ! require_env_for_prove "CAST_CHANGE_ADDRESS"; then
                stage_ok=0
                stage_error="missing required env for prove stage"
            else
                cmd=(
                    "${SCRIPT_DIR}/cast-spell-prove.sh"
                    --spell "${CAST_AUTOTRADE_SPELL_FILE}"
                    --app-bin "${CAST_APP_BIN}"
                    --prev-txs-file "${CAST_PREV_TXS_FILE}"
                    --change-address "${CAST_CHANGE_ADDRESS}"
                    --output-file "$prove_output"
                    --receipt-file "$stage_receipt"
                )
                if [[ -n "${CAST_AUTOTRADE_PRIVATE_INPUTS_FILE:-}" ]]; then
                    cmd+=(--private-inputs-file "${CAST_AUTOTRADE_PRIVATE_INPUTS_FILE}")
                fi
                if [[ "$mock_prove" -eq 1 ]]; then
                    cmd+=(--mock)
                fi
                if ! "${cmd[@]}" >/dev/null; then
                    stage_ok=0
                    stage_error="cast-spell-prove stage failed"
                fi
            fi
            ;;
        sign)
            tx_json="${CAST_AUTOTRADE_TX_JSON:-$prove_output}"
            if [[ ! -f "$tx_json" ]]; then
                stage_ok=0
                stage_error="tx json for sign stage not found: ${tx_json}"
            else
                cmd=(
                    "${SCRIPT_DIR}/cast-sign-and-broadcast.sh"
                    --tx-json "$tx_json"
                    --receipt-file "$stage_receipt"
                )
                if [[ "$broadcast" -eq 1 ]]; then
                    cmd+=(--yes-broadcast)
                else
                    cmd+=(--dry-run)
                fi
                if ! "${cmd[@]}" >/dev/null; then
                    stage_ok=0
                    stage_error="cast-sign-and-broadcast stage failed"
                fi
            fi
            ;;
        inspect)
            inspect_tx_hex="${CAST_AUTOTRADE_TX_HEX:-}"
            if [[ -z "$inspect_tx_hex" && -f "${iteration_dir}/receipts/sign.json" ]]; then
                inspect_tx_hex="$(jq -r '.signed_tx_hex[0] // empty' "${iteration_dir}/receipts/sign.json" 2>/dev/null || true)"
            fi
            if [[ -z "$inspect_tx_hex" && -f "${iteration_dir}/receipts/sign_and_broadcast.json" ]]; then
                inspect_tx_hex="$(jq -r '.signed_tx_hex[0] // empty' "${iteration_dir}/receipts/sign_and_broadcast.json" 2>/dev/null || true)"
            fi
            if [[ -z "$inspect_tx_hex" ]]; then
                stage_ok=0
                stage_error="no tx hex available for inspect stage"
            else
                cmd=(
                    "${SCRIPT_DIR}/cast-show-spell.sh"
                    --tx "$inspect_tx_hex"
                    --chain "${CAST_CHAIN:-bitcoin}"
                    --receipt-file "$stage_receipt"
                )
                if ! "${cmd[@]}" >/dev/null; then
                    stage_ok=0
                    stage_error="cast-show-spell stage failed"
                fi
            fi
            ;;
        esac

        stage_finished="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
        stage_details_json="$(jq -cn \
            --argjson current "$stage_details_json" \
            --arg stage "$stage" \
            --arg started "$stage_started" \
            --arg finished "$stage_finished" \
            --arg receipt "$stage_receipt" \
            --arg error "$stage_error" \
            --argjson ok "$stage_ok" \
            '$current + [{
                stage: $stage,
                ok: ($ok == 1),
                started_utc: $started,
                finished_utc: $finished,
                receipt_file: $receipt,
                error: $error
            }]' )"

        if [[ "$stage_ok" -ne 1 ]]; then
            overall_ok=0
            failed_stage="$stage"
            failed_message="$stage_error"
            if [[ "$continue_on_error" -ne 1 ]]; then
                break
            fi
        fi
    done

    last_local_txid=""
    last_broadcast_txid=""
    if [[ -f "${iteration_dir}/receipts/sign_and_broadcast.json" ]]; then
        last_local_txid="$(jq -r '.local_txids[0] // empty' "${iteration_dir}/receipts/sign_and_broadcast.json" 2>/dev/null || true)"
        last_broadcast_txid="$(jq -r '.broadcast_txids[0] // empty' "${iteration_dir}/receipts/sign_and_broadcast.json" 2>/dev/null || true)"
    fi

    summary_payload="$(jq -n \
        --argjson iteration "$iteration" \
        --arg run_dir "$iteration_dir" \
        --arg config_file "$config_file" \
        --arg stages_csv "$stages_csv" \
        --arg failed_stage "$failed_stage" \
        --arg failed_message "$failed_message" \
        --arg mode "$(if [[ "$broadcast" -eq 1 ]]; then printf 'broadcast'; else printf 'dry_run'; fi)" \
        --arg mock_mode "$(if [[ "$mock_prove" -eq 1 ]]; then printf 'mock'; else printf 'real'; fi)" \
        --arg last_local_txid "$last_local_txid" \
        --arg last_broadcast_txid "$last_broadcast_txid" \
        --argjson stage_details "$stage_details_json" \
        --argjson ok "$overall_ok" \
        '{
          ok: ($ok == 1),
          operation: "autotrade_loop",
          iteration: $iteration,
          run_dir: $run_dir,
          config_file: $config_file,
          stages: ($stages_csv | split(",") | map(gsub("^[[:space:]]+|[[:space:]]+$"; "")) | map(select(length > 0))),
          mode: $mode,
          prove_mode: $mock_mode,
          failed_stage: $failed_stage,
          failed_message: $failed_message,
          last_local_txid: $last_local_txid,
          last_broadcast_txid: $last_broadcast_txid,
          stage_details: $stage_details
        }')"

    summary_json="$(cast_write_receipt "autotrade_loop" "$summary_payload" "$summary_receipt")"
    if [[ -n "$summary_file" ]]; then
        mkdir -p "$(dirname "$summary_file")"
        printf '%s\n' "$summary_json" > "$summary_file"
    fi
    printf '%s\n' "$summary_json"

    [[ "$overall_ok" -eq 1 ]]
}

iteration=0
overall_exit=0

while :; do
    iteration=$((iteration + 1))
    if ! run_iteration "$iteration"; then
        overall_exit=1
        if [[ "$once" -eq 1 || "$continue_on_error" -ne 1 ]]; then
            break
        fi
    fi

    if [[ "$once" -eq 1 ]]; then
        break
    fi
    if [[ "$max_iterations" -gt 0 && "$iteration" -ge "$max_iterations" ]]; then
        break
    fi
    sleep "$interval_seconds"
done

exit "$overall_exit"
