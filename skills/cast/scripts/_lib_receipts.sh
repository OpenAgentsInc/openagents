#!/usr/bin/env bash

cast_timestamp_utc() {
    date -u +%Y%m%dT%H%M%SZ
}

cast_timestamp_iso_utc() {
    date -u +%Y-%m-%dT%H:%M:%SZ
}

cast_run_dir() {
    if [[ -n "${CAST_RUN_DIR:-}" ]]; then
        printf '%s' "$CAST_RUN_DIR"
        return
    fi

    local ts
    ts="${CAST_RUN_TIMESTAMP:-$(cast_timestamp_utc)}"
    printf 'run/%s' "$ts"
}

cast_receipts_dir() {
    local dir
    dir="$(cast_run_dir)/receipts"
    mkdir -p "$dir"
    printf '%s' "$dir"
}

cast_file_sha256() {
    local path="$1"
    if [[ ! -f "$path" ]]; then
        printf ''
        return
    fi
    shasum -a 256 "$path" | awk '{print $1}'
}

cast_verify_app_bin_hash() {
    local app_bin="$1"
    local expected_hash="${CAST_APP_BIN_SHA256:-}"
    local actual_hash
    local expected_hash_lc
    local actual_hash_lc
    actual_hash="$(cast_file_sha256 "$app_bin")"

    if [[ -z "$actual_hash" ]]; then
        printf 'Unable to compute CAST app binary hash for: %s\n' "$app_bin" >&2
        return 1
    fi

    if [[ -z "$expected_hash" ]]; then
        printf 'Warning: CAST_APP_BIN_SHA256 not set; skipping CAST app binary hash verification.\n' >&2
        return 0
    fi

    actual_hash_lc="$(printf '%s' "$actual_hash" | tr '[:upper:]' '[:lower:]')"
    expected_hash_lc="$(printf '%s' "$expected_hash" | tr '[:upper:]' '[:lower:]')"

    if [[ "$actual_hash_lc" != "$expected_hash_lc" ]]; then
        printf 'CAST app binary hash mismatch for %s\nExpected: %s\nActual:   %s\n' \
            "$app_bin" "$expected_hash" "$actual_hash" >&2
        return 1
    fi

    return 0
}

cast_write_receipt() {
    local operation="$1"
    local payload="$2"
    local receipt_file="${3:-}"
    local timestamp_utc
    local run_dir
    local wrapped_payload

    if [[ -z "$receipt_file" ]]; then
        receipt_file="$(cast_receipts_dir)/${operation}.json"
    else
        mkdir -p "$(dirname "$receipt_file")"
    fi

    timestamp_utc="$(cast_timestamp_iso_utc)"
    run_dir="$(cast_run_dir)"
    wrapped_payload="$(jq -c \
        --arg schema_version "cast-receipt/v1" \
        --arg operation "$operation" \
        --arg run_dir "$run_dir" \
        --arg timestamp_utc "$timestamp_utc" \
        '. + {
            receipt_schema_version: $schema_version,
            operation: (.operation // $operation),
            run_dir: $run_dir,
            timestamp_utc: $timestamp_utc
        }' <<<"$payload")"

    printf '%s\n' "$wrapped_payload" > "$receipt_file"
    printf '%s\n' "$wrapped_payload"
}
