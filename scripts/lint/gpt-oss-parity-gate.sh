#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

log() {
  echo "[gpt-oss-parity-gate] $*"
}

run_autopilot_test() {
  local test_name="$1"
  log "cargo test -p autopilot-desktop --lib ${test_name} -- --exact"
  cargo test -p autopilot-desktop --lib "${test_name}" -- --exact
}

run_substrate_test() {
  local test_name="$1"
  log "cargo test -p openagents-provider-substrate --lib ${test_name} -- --exact"
  cargo test -p openagents-provider-substrate --lib "${test_name}" -- --exact
}

run_autopilot_test "app_state::tests::mission_control_runtime_policy_tracks_apple_cuda_metal_and_cpu_candidates"
run_autopilot_test "app_state::tests::mission_control_gpt_oss_cuda_view_model_cycles_refresh_warm_and_unload_actions"
run_autopilot_test "pane_renderer::tests::mission_control_apple_fm_ready_lane_renders_expected_status"
run_autopilot_test "pane_renderer::tests::mission_control_gpt_oss_ready_lane_renders_expected_status"
run_autopilot_test "pane_renderer::tests::mission_control_gpt_oss_busy_button_disables_inline_action"
run_autopilot_test "desktop_control::tests::runtime_serves_gpt_oss_ready_snapshot_fields"
run_autopilot_test "desktop_control::tests::snapshot_change_events_emit_local_runtime_and_gpt_oss_domains"
run_substrate_test "tests::gpt_oss_product_ids_and_aliases_preserve_truthful_backend_identity"

log "GPT-OSS parity gate passed"
