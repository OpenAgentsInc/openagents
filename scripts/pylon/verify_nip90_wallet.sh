#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

CONFIG_PATH="$TMP_DIR/config.json"
export OPENAGENTS_PYLON_HOME="$TMP_DIR/home"
export OPENAGENTS_SPARK_NETWORK="regtest"

cd "$ROOT_DIR"

cargo run -p pylon -- --config-path "$CONFIG_PATH" init >/dev/null
cargo run -p pylon -- --config-path "$CONFIG_PATH" config set wallet_network regtest >/dev/null

config_json="$(cargo run -p pylon -- --config-path "$CONFIG_PATH" config show)"
grep -q '"wallet_network": "regtest"' <<<"$config_json"

activity_output="$(cargo run -p pylon -- --config-path "$CONFIG_PATH" activity)"
grep -q '^activity: none$' <<<"$activity_output"

cargo run -p pylon -- --config-path "$CONFIG_PATH" relays >/dev/null
cargo run -p pylon -- --config-path "$CONFIG_PATH" jobs >/dev/null
cargo run -p pylon -- --config-path "$CONFIG_PATH" earnings >/dev/null
cargo run -p pylon -- --config-path "$CONFIG_PATH" receipts >/dev/null
cargo run -p pylon -- --config-path "$CONFIG_PATH" payout >/dev/null
cargo run -p pylon -- --config-path "$CONFIG_PATH" job history >/dev/null
cargo run -p pylon -- --config-path "$CONFIG_PATH" job policy show >/dev/null

cargo test -p pylon relay_refresh_records_auth_challenges -- --nocapture
cargo test -p pylon publish_announcement_persists_handler_event -- --nocapture
cargo test -p pylon provider_scan_filters_targeted_requests -- --nocapture
cargo test -p pylon provider_run_publishes_payment_required_feedback_and_persists_invoice -- --nocapture
cargo test -p pylon provider_run_settles_paid_request_and_projects_retained_views -- --nocapture
cargo test -p pylon submit_buyer_job_publishes_request_and_persists_ledger -- --nocapture
cargo test -p pylon watch_buyer_jobs_persists_feedback_and_result_updates -- --nocapture
cargo test -p pylon approve_buyer_job_payment_submits_wallet_payment_and_persists_outcome -- --nocapture
cargo test -p pylon run_payout_withdrawal_persists_history -- --nocapture
cargo test -p pylon load_relay_activity_report_reads_retained_activity -- --nocapture

echo "Pylon NIP-90 and wallet verification passed."
