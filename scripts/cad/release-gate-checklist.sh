#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

run_check() {
    local gate="$1"
    local check="$2"
    shift 2

    local tmp
    tmp="$(mktemp)"
    printf 'CAD release gate %s: %s\n' "$gate" "$check" >&2

    if ! (cd "$ROOT_DIR" && "$@" >"$tmp" 2>&1); then
        cat "$tmp" >&2
        rm -f "$tmp"
        printf 'CAD release gate failed (%s): %s\n' "$gate" "$check" >&2
        exit 1
    fi

    rm -f "$tmp"
}

# Gate A: kernel + validity + history determinism.
run_check A "deterministic rebuild" \
    cargo test -p openagents-cad deterministic_rebuild_is_stable_across_runs_and_insertion_order --quiet
run_check A "deterministic rebuild receipts" \
    cargo test -p openagents-cad deterministic_rebuild_receipt_is_stable_and_complete --quiet
run_check A "tolerance policy bound to eval" \
    cargo test -p openagents-cad eval_uses_policy_default_tolerance --quiet
run_check A "validity warning coverage" \
    cargo test -p openagents-cad warning_fixture_covers_all_required_classes_with_deep_links --quiet
run_check A "undo/redo history determinism" \
    cargo test -p openagents-cad multi_step_undo_redo_preserves_hashes_warnings_analysis_and_semantic_refs --quiet

# Gate B: viewport/pro UX and pane clipping invariants.
run_check B "cad controls stay in bounds" \
    cargo test -p autopilot-desktop cad_demo_controls_are_ordered_and_inside_content --quiet
run_check B "view cube layout bounds" \
    cargo test -p autopilot-desktop cad_view_cube_buttons_stay_inside_content_and_non_overlapping --quiet
run_check B "warning panel clipping invariants" \
    cargo test -p autopilot-desktop cad_warning_panel_and_markers_stay_within_content_no_overflow --quiet
run_check B "timeline panel clipping invariants" \
    cargo test -p autopilot-desktop cad_timeline_panel_rows_stay_within_content_in_small_panes --quiet
run_check B "dimension panel clipping invariants" \
    cargo test -p autopilot-desktop cad_dimension_panel_rows_stay_within_content --quiet
run_check B "render mode cycling" \
    cargo test -p autopilot-desktop hidden_line_mode_cycles_deterministically --quiet
run_check B "hotkey profile + conflict checks" \
    cargo test -p autopilot-desktop cycle_hotkey_profile_action_updates_profile_with_conflict_checks --quiet
run_check B "3d mouse mapping + axis locks" \
    cargo test -p autopilot-desktop three_d_mouse_profile_mode_and_axis_locks_toggle_deterministically --quiet
run_check B "selection inspect body metrics" \
    cargo test -p autopilot-desktop selection_inspect_lines_surface_volume_mass_and_bounds_for_body_selection --quiet
run_check B "selection inspect face/edge metrics" \
    cargo test -p autopilot-desktop selection_inspect_lines_return_face_area_normal_and_edge_length_type --quiet

# Gate C: deterministic rack generator + variants.
run_check C "rack template determinism" \
    cargo test -p openagents-cad rack_template_generator_is_deterministic --quiet
run_check C "objective variant determinism" \
    cargo test -p openagents-cad objective_engine_generates_four_seeded_variants_deterministically --quiet
run_check C "rack geometry goldens" \
    cargo test -p openagents-cad rack_geometry_snapshots_match_golden_fixture --quiet

# Gate D: schema-constrained AI intents and no free-text mutation path.
run_check D "intent schema dispatch coverage" \
    cargo test -p openagents-cad dispatch_covers_all_intent_types --quiet
run_check D "explicit free-text mutation rejection" \
    cargo test -p openagents-cad free_text_mutation_is_explicitly_rejected --quiet
run_check D "chat adapter intent json" \
    cargo test -p openagents-cad adapter_accepts_valid_intent_json --quiet
run_check D "chat adapter malformed recovery" \
    cargo test -p openagents-cad adapter_returns_recovery_prompt_for_malformed_json --quiet
run_check D "chat->cad interaction deterministic path" \
    cargo test -p autopilot-desktop follow_up_parameter_edit_interaction_matches_golden_receipts --quiet

# Gate E: engineering overlays, scripted reliability, and budget compliance.
run_check E "step checker + roundtrip fixtures" \
    "$ROOT_DIR/scripts/cad/step-checker-ci.sh"
run_check E "headless script harness" \
    "$ROOT_DIR/scripts/cad/headless-script-ci.sh"
run_check E "performance budget suite" \
    "$ROOT_DIR/scripts/cad/perf-benchmark-ci.sh"
run_check E "20 second reliability script" \
    "$ROOT_DIR/scripts/cad/reliability-20s-ci.sh"

printf 'CAD demo release gates passed (A-E).\n'
