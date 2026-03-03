#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

LANE_LABELS=(
    "parity-check"
    "ci-artifact-manifest-check"
    "artifact-copy"
    "artifact-bundle"
    "artifact-checksum"
)

ARTIFACT_SOURCE_PATHS=(
    "crates/cad/parity/vcad_reference_manifest.json"
    "crates/cad/parity/openagents_start_manifest.json"
    "crates/cad/parity/vcad_capabilities_inventory.json"
    "crates/cad/parity/openagents_capabilities_inventory.json"
    "crates/cad/parity/vcad_openagents_gap_matrix.json"
    "crates/cad/parity/parity_scorecard.json"
    "crates/cad/parity/parity_risk_register.json"
    "crates/cad/parity/parity_dashboard.json"
    "crates/cad/parity/kernel_adapter_v2_manifest.json"
    "crates/cad/parity/kernel_math_parity_manifest.json"
    "crates/cad/parity/kernel_topology_parity_manifest.json"
    "crates/cad/parity/kernel_geom_parity_manifest.json"
    "crates/cad/parity/kernel_primitives_parity_manifest.json"
    "crates/cad/parity/kernel_tessellate_parity_manifest.json"
    "crates/cad/parity/kernel_booleans_parity_manifest.json"
    "crates/cad/parity/kernel_boolean_diagnostics_parity_manifest.json"
    "crates/cad/parity/kernel_boolean_brep_parity_manifest.json"
    "crates/cad/parity/kernel_nurbs_parity_manifest.json"
    "crates/cad/parity/kernel_text_parity_manifest.json"
    "crates/cad/parity/kernel_fillet_parity_manifest.json"
    "crates/cad/parity/kernel_shell_parity_manifest.json"
    "crates/cad/parity/kernel_step_parity_manifest.json"
    "crates/cad/parity/kernel_precision_parity_manifest.json"
    "crates/cad/parity/primitive_contracts_parity_manifest.json"
    "crates/cad/parity/transform_parity_manifest.json"
    "crates/cad/parity/pattern_parity_manifest.json"
    "crates/cad/parity/shell_feature_graph_parity_manifest.json"
    "crates/cad/parity/fillet_feature_graph_parity_manifest.json"
    "crates/cad/parity/chamfer_feature_graph_parity_manifest.json"
    "crates/cad/parity/expanded_finishing_parity_manifest.json"
    "crates/cad/parity/sweep_parity_manifest.json"
    "crates/cad/parity/loft_parity_manifest.json"
    "crates/cad/parity/topology_repair_parity_manifest.json"
    "crates/cad/parity/material_assignment_parity_manifest.json"
    "crates/cad/parity/vcad_eval_receipts_parity_manifest.json"
    "crates/cad/parity/feature_op_hash_parity_manifest.json"
    "crates/cad/parity/modeling_edge_case_parity_manifest.json"
    "crates/cad/parity/core_modeling_checkpoint_parity_manifest.json"
    "crates/cad/parity/sketch_entity_set_parity_manifest.json"
    "crates/cad/parity/sketch_plane_parity_manifest.json"
    "crates/cad/parity/sketch_constraint_enum_parity_manifest.json"
    "crates/cad/parity/sketch_iterative_lm_parity_manifest.json"
    "crates/cad/parity/sketch_jacobian_residual_parity_manifest.json"
    "crates/cad/parity/sketch_constraint_status_parity_manifest.json"
    "crates/cad/parity/sketch_extrude_parity_manifest.json"
    "crates/cad/parity/sketch_interaction_parity_manifest.json"
    "crates/cad/parity/sketch_fixture_equivalence_parity_manifest.json"
    "crates/cad/parity/sketch_undo_redo_parity_manifest.json"
    "crates/cad/parity/sketch_constraints_checkpoint_parity_manifest.json"
    "crates/cad/parity/assembly_schema_parity_manifest.json"
    "crates/cad/parity/assembly_part_instance_parity_manifest.json"
    "crates/cad/parity/assembly_joint_frs_parity_manifest.json"
    "crates/cad/parity/assembly_joint_cb_parity_manifest.json"
    "crates/cad/parity/assembly_joint_limits_state_parity_manifest.json"
    "crates/cad/parity/assembly_fk_parity_manifest.json"
    "crates/cad/parity/assembly_ground_delete_parity_manifest.json"
    "crates/cad/parity/assembly_ui_selection_edit_parity_manifest.json"
    "crates/cad/parity/assembly_serialization_replay_parity_manifest.json"
    "crates/cad/parity/assembly_acceptance_scenes_parity_manifest.json"
    "crates/cad/parity/assembly_checkpoint_parity_manifest.json"
    "crates/cad/parity/drafting_kernel_scaffolding_parity_manifest.json"
    "crates/cad/parity/drafting_projection_parity_manifest.json"
    "crates/cad/parity/drafting_hidden_line_parity_manifest.json"
    "crates/cad/parity/drafting_dimension_parity_manifest.json"
    "crates/cad/parity/drafting_gdt_parity_manifest.json"
    "crates/cad/parity/drafting_section_parity_manifest.json"
    "crates/cad/parity/drafting_detail_parity_manifest.json"
    "crates/cad/parity/drafting_drawing_mode_ui_parity_manifest.json"
    "crates/cad/parity/drafting_persistence_parity_manifest.json"
    "crates/cad/parity/drafting_dxf_export_parity_manifest.json"
    "crates/cad/parity/drafting_pdf_export_parity_manifest.json"
    "crates/cad/parity/drafting_checkpoint_parity_manifest.json"
    "crates/cad/parity/step_import_entity_parity_manifest.json"
    "crates/cad/parity/step_export_post_boolean_parity_manifest.json"
    "crates/cad/parity/stl_import_export_parity_manifest.json"
    "crates/cad/parity/glb_export_parity_manifest.json"
    "crates/cad/parity/cad_cli_scaffold_parity_manifest.json"
    "crates/cad/parity/cad_cli_commands_parity_manifest.json"
    "crates/cad/parity/cad_mcp_tools_parity_manifest.json"
    "crates/cad/parity/compact_ir_parity_manifest.json"
    "crates/cad/parity/intent_modeling_parity_manifest.json"
    "crates/cad/parity/text_to_cad_parity_manifest.json"
    "crates/cad/parity/text_to_cad_dataset_parity_manifest.json"
    "crates/cad/parity/text_to_cad_training_eval_parity_manifest.json"
    "crates/cad/parity/headless_script_harness_parity_manifest.json"
    "crates/cad/parity/io_headless_ai_checkpoint_parity_manifest.json"
    "crates/cad/parity/viewport_camera_gizmo_parity_manifest.json"
    "crates/cad/parity/render_mode_parity_manifest.json"
    "crates/cad/parity/gpu_acceleration_parity_manifest.json"
    "crates/cad/parity/mesh_upload_processing_parity_manifest.json"
    "crates/cad/parity/direct_brep_raytrace_scaffolding_parity_manifest.json"
    "crates/cad/parity/analytic_ray_intersections_parity_manifest.json"
    "crates/cad/parity/trimmed_surface_ray_hit_parity_manifest.json"
    "crates/cad/parity/bvh_build_traverse_parity_manifest.json"
    "crates/cad/parity/raytrace_quality_mode_parity_manifest.json"
    "crates/cad/parity/raytrace_face_pick_parity_manifest.json"
    "crates/cad/parity/raytrace_ui_toggle_fallback_parity_manifest.json"
    "crates/cad/parity/rendering_raytrace_checkpoint_parity_manifest.json"
    "crates/cad/parity/physics_crate_integration_parity_manifest.json"
    "crates/cad/parity/sketch_loft_parity_manifest.json"
    "crates/cad/parity/sketch_profile_validity_parity_manifest.json"
    "crates/cad/parity/sketch_revolve_parity_manifest.json"
    "crates/cad/parity/sketch_sweep_parity_manifest.json"
    "crates/cad/parity/fixtures/feature_op_hash_vcad_reference_corpus.json"
    "crates/cad/parity/fixtures/sketch_vcad_reference_corpus.json"
    "crates/cad/parity/fixtures/assembly_schema_vcad_reference.json"
    "crates/cad/parity/fixtures/assembly_part_instance_vcad_reference.json"
    "crates/cad/parity/fixtures/assembly_joint_frs_vcad_reference.json"
    "crates/cad/parity/fixtures/assembly_joint_cb_vcad_reference.json"
    "crates/cad/parity/fixtures/assembly_joint_limits_state_vcad_reference.json"
    "crates/cad/parity/fixtures/assembly_fk_vcad_reference.json"
    "crates/cad/parity/fixtures/assembly_ground_delete_vcad_reference.json"
    "crates/cad/parity/fixtures/assembly_ui_selection_edit_vcad_reference.json"
    "crates/cad/parity/fixtures/assembly_serialization_replay_vcad_reference.json"
    "crates/cad/parity/fixtures/assembly_acceptance_scenes_vcad_reference.json"
    "crates/cad/parity/fixtures/drafting_kernel_scaffolding_vcad_reference.json"
    "crates/cad/parity/fixtures/drafting_projection_vcad_reference.json"
    "crates/cad/parity/fixtures/drafting_hidden_line_vcad_reference.json"
    "crates/cad/parity/fixtures/drafting_dimension_vcad_reference.json"
    "crates/cad/parity/fixtures/drafting_gdt_vcad_reference.json"
    "crates/cad/parity/fixtures/drafting_section_vcad_reference.json"
    "crates/cad/parity/fixtures/drafting_detail_vcad_reference.json"
    "crates/cad/parity/fixtures/drafting_drawing_mode_ui_vcad_reference.json"
    "crates/cad/parity/fixtures/drafting_persistence_vcad_reference.json"
    "crates/cad/parity/fixtures/drafting_dxf_export_vcad_reference.json"
    "crates/cad/parity/fixtures/drafting_pdf_export_vcad_reference.json"
    "crates/cad/parity/fixtures/step_import_entity_vcad_reference.json"
    "crates/cad/parity/fixtures/step_export_post_boolean_vcad_reference.json"
    "crates/cad/parity/fixtures/stl_import_export_vcad_reference.json"
    "crates/cad/parity/fixtures/glb_export_vcad_reference.json"
    "crates/cad/parity/fixtures/cad_cli_scaffold_vcad_reference.json"
    "crates/cad/parity/fixtures/cad_cli_commands_vcad_reference.json"
    "crates/cad/parity/fixtures/cad_mcp_tools_vcad_reference.json"
    "crates/cad/parity/fixtures/compact_ir_vcad_reference.json"
    "crates/cad/parity/fixtures/intent_modeling_vcad_reference.json"
    "crates/cad/parity/fixtures/text_to_cad_vcad_reference.json"
    "crates/cad/parity/fixtures/text_to_cad_dataset_vcad_reference.json"
    "crates/cad/parity/fixtures/text_to_cad_training_eval_vcad_reference.json"
    "crates/cad/parity/fixtures/headless_script_harness_vcad_reference.json"
    "crates/cad/parity/fixtures/viewport_camera_gizmo_vcad_reference.json"
    "crates/cad/parity/fixtures/render_mode_vcad_reference.json"
    "crates/cad/parity/fixtures/gpu_acceleration_vcad_reference.json"
    "crates/cad/parity/fixtures/mesh_upload_processing_vcad_reference.json"
    "crates/cad/parity/fixtures/direct_brep_raytrace_scaffolding_vcad_reference.json"
    "crates/cad/parity/fixtures/analytic_ray_intersections_vcad_reference.json"
    "crates/cad/parity/fixtures/trimmed_surface_ray_hit_vcad_reference.json"
    "crates/cad/parity/fixtures/bvh_build_traverse_vcad_reference.json"
    "crates/cad/parity/fixtures/raytrace_quality_mode_vcad_reference.json"
    "crates/cad/parity/fixtures/raytrace_face_pick_vcad_reference.json"
    "crates/cad/parity/fixtures/raytrace_ui_toggle_fallback_vcad_reference.json"
    "crates/cad/parity/fixtures/physics_crate_integration_vcad_reference.json"
    "crates/cad/parity/collision_shape_generation_parity_manifest.json"
    "crates/cad/parity/fixtures/collision_shape_generation_vcad_reference.json"
    "crates/cad/parity/fixtures/parity_fixture_corpus.json"
)

usage() {
    cat <<USAGE
Usage:
  scripts/cad/parity-ci-lane.sh
  scripts/cad/parity-ci-lane.sh --check
  scripts/cad/parity-ci-lane.sh --list
  scripts/cad/parity-ci-lane.sh --artifacts-dir <path>
  scripts/cad/parity-ci-lane.sh --skip-tests

Options:
  --check                Run parity checks + fixture checks only (no bundle output)
  --list                 Print CI lane step IDs and exit
  --artifacts-dir <path> Output directory for CI payload and bundle
  --skip-tests           Pass --skip-tests to scripts/cad/parity_check.sh
USAGE
}

ARTIFACTS_DIR="$ROOT_DIR/target/parity-ci"
CHECK_ONLY=0
LIST_ONLY=0
SKIP_TESTS=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --check)
            CHECK_ONLY=1
            shift
            ;;
        --list)
            LIST_ONLY=1
            shift
            ;;
        --artifacts-dir)
            if [[ $# -lt 2 ]]; then
                printf 'missing value for --artifacts-dir\n\n' >&2
                usage >&2
                exit 2
            fi
            ARTIFACTS_DIR="$2"
            shift 2
            ;;
        --skip-tests)
            SKIP_TESTS=1
            shift
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            printf 'Unknown argument: %s\n\n' "$1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

if (( LIST_ONLY == 1 )); then
    for lane in "${LANE_LABELS[@]}"; do
        printf '%s\n' "$lane"
    done
    exit 0
fi

run_parity_check() {
    if (( SKIP_TESTS == 1 )); then
        "$ROOT_DIR/scripts/cad/parity_check.sh" --skip-tests
        return
    fi
    "$ROOT_DIR/scripts/cad/parity_check.sh"
}

run_parity_check
"$ROOT_DIR/scripts/cad/parity-ci-artifacts-ci.sh"

if (( CHECK_ONLY == 1 )); then
    printf 'CAD parity CI lane checks passed.\n'
    exit 0
fi

mkdir -p "$ARTIFACTS_DIR"
PAYLOAD_DIR="$ARTIFACTS_DIR/payload"
rm -rf "$PAYLOAD_DIR"
mkdir -p "$PAYLOAD_DIR"

for rel in "${ARTIFACT_SOURCE_PATHS[@]}"; do
    src="$ROOT_DIR/$rel"
    dst="$PAYLOAD_DIR/$(echo "$rel" | sed 's#^crates/cad/parity/##' | tr '/\\' '__')"
    cp "$src" "$dst"
done

cargo run -p openagents-cad --bin parity-ci-artifacts -- \
    --output "$PAYLOAD_DIR/parity_ci_artifact_manifest.json"

BUNDLE_PATH="$ARTIFACTS_DIR/parity_ci_artifacts.tar.gz"
rm -f "$BUNDLE_PATH"
tar -czf "$BUNDLE_PATH" -C "$PAYLOAD_DIR" .

BUNDLE_SHA256="$(sha256sum "$BUNDLE_PATH" | awk '{print $1}')"
SHA_FILE="$ARTIFACTS_DIR/parity_ci_artifacts.sha256"
printf '%s  %s\n' "$BUNDLE_SHA256" "$(basename "$BUNDLE_PATH")" >"$SHA_FILE"

UPLOAD_ENV="$ARTIFACTS_DIR/parity_ci_upload.env"
cat >"$UPLOAD_ENV" <<ENV
PARITY_CI_ARTIFACT_DIR=$ARTIFACTS_DIR
PARITY_CI_PAYLOAD_DIR=$PAYLOAD_DIR
PARITY_CI_MANIFEST_PATH=$PAYLOAD_DIR/parity_ci_artifact_manifest.json
PARITY_CI_BUNDLE_PATH=$BUNDLE_PATH
PARITY_CI_BUNDLE_SHA256=$BUNDLE_SHA256
PARITY_CI_BUNDLE_SHA_FILE=$SHA_FILE
ENV

printf 'CAD parity CI artifacts generated.\n'
printf 'bundle: %s\n' "$BUNDLE_PATH"
printf 'sha256: %s\n' "$BUNDLE_SHA256"
printf 'upload metadata: %s\n' "$UPLOAD_ENV"
