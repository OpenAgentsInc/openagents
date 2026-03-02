# Baseline Parity Dashboard

Issue coverage: `VCAD-PARITY-010`

## Snapshot

| Metric | Value |
| --- | --- |
| vcad commit | `1b59e7948efcdb848d8dba6848785d57aa310e81` |
| openagents commit (plan baseline) | `04faa5227f077c419f1c5c52ddebbb7552838fd4` |
| phase status | `phase_d_constraint_enum_complete` |
| overall match rate | `0.072464` |
| docs match rate | `0.038961` |
| crates match rate | `0.122449` |
| commands match rate | `0.083333` |
| open risks | `24` |
| open hard blockers (p0) | `16` |
| CI source artifact count | `43` |

## Profile Gates

| Lane | Profile | Pass |
| --- | --- | --- |
| `risk_register` | `parity_complete_v1` | `false` |
| `scorecard` | `parity_complete_v1` | `false` |
| `risk_register` | `phase_a_baseline_v1` | `true` |
| `scorecard` | `phase_a_baseline_v1` | `true` |

## CI Evidence Artifacts

- `chamfer_feature_graph_parity_manifest`
- `core_modeling_checkpoint_parity_manifest`
- `expanded_finishing_parity_manifest`
- `feature_op_hash_parity_manifest`
- `fillet_feature_graph_parity_manifest`
- `fixtures_feature_op_hash_vcad_reference_corpus`
- `fixtures_parity_fixture_corpus`
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
- `sketch_entity_set_parity_manifest`
- `sketch_plane_parity_manifest`
- `sweep_parity_manifest`
- `topology_repair_parity_manifest`
- `transform_parity_manifest`
- `vcad_capabilities_inventory`
- `vcad_eval_receipts_parity_manifest`
- `vcad_openagents_gap_matrix`
- `vcad_reference_manifest`

## Next Actions

- Execute VCAD-PARITY-044 through VCAD-PARITY-055 sequentially
- Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes
- Refresh parity dashboard after each closed parity issue
