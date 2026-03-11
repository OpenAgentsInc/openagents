#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${repo_root}"

cargo test -p autopilot-desktop --bin autopilot-desktop --no-run

tests=(
  "desktop_shell::tests::dev_mode_defaults_off"
  "pane_registry::tests::mission_control_command_maps_to_singleton_startup_pane"
  "pane_system::tests::mission_control_descriptor_defaults_to_fullscreen_presentation"
  "pane_system::tests::fullscreen_presentation_uses_full_bounds_as_content"
  "pane_system::tests::mission_control_withdraw_controls_fit_below_local_model_action"
  "app_state::tests::mission_control_production_lane_stays_apple_fm_only"
  "app_state::tests::mission_control_production_log_lines_do_not_fall_back_to_gpt_oss"
  "pane_renderer::tests::mission_control_button_prefers_supported_runtime_lane"
  "pane_renderer::tests::mission_control_reports_missing_supported_runtime_lane"
)

for test_name in "${tests[@]}"; do
  cargo test -p autopilot-desktop --bin autopilot-desktop "${test_name}" -- --exact
done
