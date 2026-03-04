#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

printf 'Running Autopilot earnings epic test matrix gate...\n'

run_test() {
    local name="$1"
    shift
    printf ' - %s\n' "$name"
    (cd "$ROOT_DIR" && "$@")
}

run_test \
    "Condition evaluation core" \
    cargo test -p autopilot-desktop \
    state::goal_conditions::tests::evaluate_marks_goal_complete_when_all_targets_met \
    -- --nocapture
run_test \
    "Skill resolver deterministic ordering" \
    cargo test -p autopilot-desktop \
    state::goal_skill_resolver::tests::resolver_orders_priority_skills_deterministically_for_earn_goal \
    -- --nocapture
run_test \
    "Turn skill attachment deterministic ordering" \
    cargo test -p autopilot-desktop \
    input::tests::assemble_chat_turn_input_orders_and_dedupes_skills_deterministically \
    -- --nocapture
run_test \
    "Swap quote/accept matrix (both conversion directions)" \
    cargo test -p autopilot-desktop \
    state::swap_quote_adapter::tests::roundtrip_btc_to_usd_and_usd_to_btc_paths_accept_quotes_deterministically \
    -- --nocapture
run_test \
    "Swap adapter fallback audit path" \
    cargo test -p autopilot-desktop \
    state::autopilot_goals::tests::request_swap_quote_with_adapter_records_audit_and_fallback \
    -- --nocapture
run_test \
    "Blink quote payload parsing contract" \
    cargo test -p autopilot-desktop \
    input::tool_bridge::tests::blink_quote_payload_parsing_maps_swap_terms \
    -- --nocapture
run_test \
    "Blink script path missing contract" \
    cargo test -p autopilot-desktop \
    input::tool_bridge::tests::blink_script_selection_errors_when_no_candidate_exists \
    -- --nocapture
run_test \
    "Blink script failure contract" \
    cargo test -p autopilot-desktop \
    input::tool_bridge::tests::blink_script_runner_reports_script_failure \
    -- --nocapture
run_test \
    "Blink script JSON parse contract" \
    cargo test -p autopilot-desktop \
    input::tool_bridge::tests::blink_script_runner_rejects_non_json_stdout \
    -- --nocapture
run_test \
    "Blink quote required-field contract" \
    cargo test -p autopilot-desktop \
    input::tool_bridge::tests::blink_quote_payload_parser_rejects_missing_required_fields \
    -- --nocapture
run_test \
    "Blink execution required-field contract" \
    cargo test -p autopilot-desktop \
    input::tool_bridge::tests::blink_execution_payload_parser_rejects_missing_status \
    -- --nocapture
run_test \
    "Swap tool schema blocks injected quote/status fields" \
    cargo test -p autopilot-desktop \
    openagents_dynamic_tools::tests::swap_tool_schemas_do_not_allow_injected_quote_or_status_fields \
    -- --nocapture
run_test \
    "Interval scheduler trigger path" \
    cargo test -p autopilot-desktop \
    state::autopilot_goals::tests::interval_schedule_persists_and_triggers_queue_transition \
    -- --nocapture
run_test \
    "Cron scheduler trigger path" \
    cargo test -p autopilot-desktop \
    state::autopilot_goals::tests::scheduler_tick_triggers_due_cron_goal_and_sets_next_preview \
    -- --nocapture
run_test \
    "Restart recovery semantics" \
    cargo test -p autopilot-desktop \
    state::autopilot_goals::tests::recover_after_restart_applies_running_and_missed_run_semantics \
    -- --nocapture
run_test \
    "Swap policy enforcement (request)" \
    cargo test -p autopilot-desktop \
    state::autopilot_goals::tests::validate_swap_request_policy_by_goal_id \
    -- --nocapture
run_test \
    "Swap policy enforcement (quote)" \
    cargo test -p autopilot-desktop \
    state::autopilot_goals::tests::validate_swap_quote_policy_by_goal_id \
    -- --nocapture
run_test \
    "Payout-gated success path" \
    cargo test -p autopilot-desktop \
    state::earnings_gate::tests::accepts_wallet_backed_earnings_evidence \
    -- --nocapture
run_test \
    "Relay-backed NIP-90 provider ingress lane wiring" \
    cargo test -p autopilot-desktop \
    provider_nip90_lane::tests::worker_ingests_live_relay_request \
    -- --nocapture
run_test \
    "Desktop E2E harness (relay -> execute -> publish -> wallet confirm)" \
    cargo test -p autopilot-desktop \
    provider_nip90_lane::tests::desktop_earn_harness_relay_execute_publish_wallet_confirm_end_to_end \
    -- --nocapture
run_test \
    "Starter-demand generator budget/kill-switch controls" \
    cargo test -p autopilot-desktop \
    app_state::tests::starter_demand_ \
    -- --nocapture
run_test \
    "Starter-demand provenance propagates into receipts/history" \
    cargo test -p autopilot-desktop \
    app_state::tests::starter_provenance_propagates_from_inbox_to_history_receipt \
    -- --nocapture
run_test \
    "Mission Control aggregate counters pipeline" \
    cargo test -p autopilot-desktop \
    app_state::tests::network_aggregate_counters_ \
    -- --nocapture
run_test \
    "Earn bitcoin until +N sats flow" \
    cargo test -p autopilot-desktop \
    state::earnings_gate::tests::earn_bitcoin_until_target_sats_requires_wallet_confirmed_threshold \
    -- --nocapture
run_test \
    "Rollout gate stage/cohort enforcement" \
    cargo test -p autopilot-desktop \
    state::autopilot_goals::tests::rollout_gate_respects_feature_flag_stage_and_cohorts \
    -- --nocapture
run_test \
    "Rollout metrics and rollback health thresholds" \
    cargo test -p autopilot-desktop \
    state::autopilot_goals::tests::rollout_metrics_and_health_capture_false_success_latency_and_abort_distribution \
    -- --nocapture

printf 'Autopilot earnings epic test matrix gate passed.\n'
