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
