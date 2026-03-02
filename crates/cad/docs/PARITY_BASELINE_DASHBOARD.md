# Baseline Parity Dashboard

Issue coverage: `VCAD-PARITY-010`

## Snapshot

| Metric | Value |
| --- | --- |
| vcad commit | `1b59e7948efcdb848d8dba6848785d57aa310e81` |
| openagents commit (plan baseline) | `04faa5227f077c419f1c5c52ddebbb7552838fd4` |
| phase status | `phase_g_cad_mcp_tools_complete` |
| overall match rate | `0.072464` |
| docs match rate | `0.038961` |
| crates match rate | `0.122449` |
| commands match rate | `0.083333` |
| open risks | `24` |
| open hard blockers (p0) | `16` |
| CI source artifact count | `114` |

## Profile Gates

| Lane | Profile | Pass |
| --- | --- | --- |
| `risk_register` | `parity_complete_v1` | `false` |
| `scorecard` | `parity_complete_v1` | `false` |
| `risk_register` | `phase_a_baseline_v1` | `true` |
| `scorecard` | `phase_a_baseline_v1` | `true` |

## CI Evidence Artifacts

- `assembly_acceptance_scenes_parity_manifest`
- `assembly_checkpoint_parity_manifest`
- `assembly_fk_parity_manifest`
- `assembly_ground_delete_parity_manifest`
- `assembly_joint_cb_parity_manifest`
- `assembly_joint_frs_parity_manifest`
- `assembly_joint_limits_state_parity_manifest`
- `assembly_part_instance_parity_manifest`
- `assembly_schema_parity_manifest`
- `assembly_serialization_replay_parity_manifest`
- `assembly_ui_selection_edit_parity_manifest`
- `cad_cli_commands_parity_manifest`
- `cad_cli_scaffold_parity_manifest`
- `cad_mcp_tools_parity_manifest`
- `chamfer_feature_graph_parity_manifest`
- `core_modeling_checkpoint_parity_manifest`
- `drafting_checkpoint_parity_manifest`
- `drafting_detail_parity_manifest`
- `drafting_dimension_parity_manifest`
- `drafting_drawing_mode_ui_parity_manifest`
- `drafting_dxf_export_parity_manifest`
- `drafting_gdt_parity_manifest`
- `drafting_hidden_line_parity_manifest`
- `drafting_kernel_scaffolding_parity_manifest`
- `drafting_pdf_export_parity_manifest`
- `drafting_persistence_parity_manifest`
- `drafting_projection_parity_manifest`
- `drafting_section_parity_manifest`
- `expanded_finishing_parity_manifest`
- `feature_op_hash_parity_manifest`
- `fillet_feature_graph_parity_manifest`
- `fixtures_assembly_acceptance_scenes_vcad_reference`
- `fixtures_assembly_fk_vcad_reference`
- `fixtures_assembly_ground_delete_vcad_reference`
- `fixtures_assembly_joint_cb_vcad_reference`
- `fixtures_assembly_joint_frs_vcad_reference`
- `fixtures_assembly_joint_limits_state_vcad_reference`
- `fixtures_assembly_part_instance_vcad_reference`
- `fixtures_assembly_schema_vcad_reference`
- `fixtures_assembly_serialization_replay_vcad_reference`
- `fixtures_assembly_ui_selection_edit_vcad_reference`
- `fixtures_cad_cli_commands_vcad_reference`
- `fixtures_cad_cli_scaffold_vcad_reference`
- `fixtures_cad_mcp_tools_vcad_reference`
- `fixtures_drafting_detail_vcad_reference`
- `fixtures_drafting_dimension_vcad_reference`
- `fixtures_drafting_drawing_mode_ui_vcad_reference`
- `fixtures_drafting_dxf_export_vcad_reference`
- `fixtures_drafting_gdt_vcad_reference`
- `fixtures_drafting_hidden_line_vcad_reference`
- `fixtures_drafting_kernel_scaffolding_vcad_reference`
- `fixtures_drafting_pdf_export_vcad_reference`
- `fixtures_drafting_persistence_vcad_reference`
- `fixtures_drafting_projection_vcad_reference`
- `fixtures_drafting_section_vcad_reference`
- `fixtures_feature_op_hash_vcad_reference_corpus`
- `fixtures_glb_export_vcad_reference`
- `fixtures_parity_fixture_corpus`
- `fixtures_sketch_vcad_reference_corpus`
- `fixtures_step_export_post_boolean_vcad_reference`
- `fixtures_step_import_entity_vcad_reference`
- `fixtures_stl_import_export_vcad_reference`
- `glb_export_parity_manifest`
- `kernel_adapter_v2_manifest`
- `kernel_boolean_brep_parity_manifest`
- `kernel_boolean_diagnostics_parity_manifest`
- `kernel_booleans_parity_manifest`
- `kernel_fillet_parity_manifest`
- `kernel_geom_parity_manifest`
- `kernel_math_parity_manifest`
- `kernel_nurbs_parity_manifest`
- `kernel_precision_parity_manifest`
- `kernel_primitives_parity_manifest`
- `kernel_shell_parity_manifest`
- `kernel_step_parity_manifest`
- `kernel_tessellate_parity_manifest`
- `kernel_text_parity_manifest`
- `kernel_topology_parity_manifest`
- `loft_parity_manifest`
- `material_assignment_parity_manifest`
- `modeling_edge_case_parity_manifest`
- `openagents_capabilities_inventory`
- `openagents_start_manifest`
- `parity_dashboard`
- `parity_risk_register`
- `parity_scorecard`
- `pattern_parity_manifest`
- `primitive_contracts_parity_manifest`
- `shell_feature_graph_parity_manifest`
- `sketch_constraint_enum_parity_manifest`
- `sketch_constraint_status_parity_manifest`
- `sketch_constraints_checkpoint_parity_manifest`
- `sketch_entity_set_parity_manifest`
- `sketch_extrude_parity_manifest`
- `sketch_fixture_equivalence_parity_manifest`
- `sketch_interaction_parity_manifest`
- `sketch_iterative_lm_parity_manifest`
- `sketch_jacobian_residual_parity_manifest`
- `sketch_loft_parity_manifest`
- `sketch_plane_parity_manifest`
- `sketch_profile_validity_parity_manifest`
- `sketch_revolve_parity_manifest`
- `sketch_sweep_parity_manifest`
- `sketch_undo_redo_parity_manifest`
- `step_export_post_boolean_parity_manifest`
- `step_import_entity_parity_manifest`
- `stl_import_export_parity_manifest`
- `sweep_parity_manifest`
- `topology_repair_parity_manifest`
- `transform_parity_manifest`
- `vcad_capabilities_inventory`
- `vcad_eval_receipts_parity_manifest`
- `vcad_openagents_gap_matrix`
- `vcad_reference_manifest`

## Next Actions

- Execute VCAD-PARITY-086 through VCAD-PARITY-092 sequentially
- Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes
- Refresh parity dashboard after each closed parity issue
