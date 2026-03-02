# CAD Parity Artifacts

This folder stores machine-readable artifacts used by the VCAD parity program.

Current artifacts:

- `vcad_reference_manifest.json` (`VCAD-PARITY-001`)
- `openagents_start_manifest.json` (`VCAD-PARITY-001`)
- `vcad_capabilities_inventory.json` (`VCAD-PARITY-002`)
- `openagents_capabilities_inventory.json` (`VCAD-PARITY-003`)
- `vcad_openagents_gap_matrix.json` (`VCAD-PARITY-004`)
- `parity_scorecard.json` (`VCAD-PARITY-005`)
- `fixtures/parity_fixture_corpus.json` (`VCAD-PARITY-006`)
- `parity_ci_artifact_manifest.json` (`VCAD-PARITY-008`)
- `parity_risk_register.json` (`VCAD-PARITY-009`)
- `parity_dashboard.json` (`VCAD-PARITY-010`)
- `kernel_adapter_v2_manifest.json` (`VCAD-PARITY-011`)
- `kernel_math_parity_manifest.json` (`VCAD-PARITY-012`)
- `kernel_topology_parity_manifest.json` (`VCAD-PARITY-013`)
- `kernel_geom_parity_manifest.json` (`VCAD-PARITY-014`)
- `kernel_primitives_parity_manifest.json` (`VCAD-PARITY-015`)
- `kernel_tessellate_parity_manifest.json` (`VCAD-PARITY-016`)
- `kernel_precision_parity_manifest.json` (`VCAD-PARITY-017`)
- `kernel_booleans_parity_manifest.json` (`VCAD-PARITY-018`)
- `kernel_boolean_diagnostics_parity_manifest.json` (`VCAD-PARITY-019`)
- `kernel_boolean_brep_parity_manifest.json` (`VCAD-PARITY-020`)
- `kernel_nurbs_parity_manifest.json` (`VCAD-PARITY-021`)
- `kernel_text_parity_manifest.json` (`VCAD-PARITY-022`)
- `kernel_fillet_parity_manifest.json` (`VCAD-PARITY-023`)
- `kernel_shell_parity_manifest.json` (`VCAD-PARITY-024`)
- `kernel_step_parity_manifest.json` (`VCAD-PARITY-025`)
- `primitive_contracts_parity_manifest.json` (`VCAD-PARITY-026`)
- `transform_parity_manifest.json` (`VCAD-PARITY-027`)
- `pattern_parity_manifest.json` (`VCAD-PARITY-028`)
- `shell_feature_graph_parity_manifest.json` (`VCAD-PARITY-029`)
- `fillet_feature_graph_parity_manifest.json` (`VCAD-PARITY-030`)
- `chamfer_feature_graph_parity_manifest.json` (`VCAD-PARITY-031`)
- `expanded_finishing_parity_manifest.json` (`VCAD-PARITY-032`)
- `sweep_parity_manifest.json` (`VCAD-PARITY-033`)
- `loft_parity_manifest.json` (`VCAD-PARITY-034`)
- `topology_repair_parity_manifest.json` (`VCAD-PARITY-035`)
- `material_assignment_parity_manifest.json` (`VCAD-PARITY-036`)
- `vcad_eval_receipts_parity_manifest.json` (`VCAD-PARITY-037`)
- `feature_op_hash_parity_manifest.json` (`VCAD-PARITY-038`)
- `fixtures/feature_op_hash_vcad_reference_corpus.json` (`VCAD-PARITY-038`)
- `modeling_edge_case_parity_manifest.json` (`VCAD-PARITY-039`)
- `core_modeling_checkpoint_parity_manifest.json` (`VCAD-PARITY-040`)
- `sketch_entity_set_parity_manifest.json` (`VCAD-PARITY-041`)
- `sketch_plane_parity_manifest.json` (`VCAD-PARITY-042`)
- `sketch_constraint_enum_parity_manifest.json` (`VCAD-PARITY-043`)
- `sketch_iterative_lm_parity_manifest.json` (`VCAD-PARITY-044`)
- `sketch_jacobian_residual_parity_manifest.json` (`VCAD-PARITY-045`)
- `sketch_constraint_status_parity_manifest.json` (`VCAD-PARITY-046`)
- `sketch_extrude_parity_manifest.json` (`VCAD-PARITY-047`)
- `sketch_revolve_parity_manifest.json` (`VCAD-PARITY-048`)
- `sketch_sweep_parity_manifest.json` (`VCAD-PARITY-049`)

Regeneration/check command:

```bash
scripts/cad/freeze-parity-baseline.sh
scripts/cad/freeze-parity-baseline.sh --check
scripts/cad/vcad-capability-crawler-ci.sh
scripts/cad/openagents-capability-crawler-ci.sh
scripts/cad/parity-gap-matrix-ci.sh
scripts/cad/parity-scorecard-ci.sh
scripts/cad/parity-fixture-corpus-ci.sh
scripts/cad/parity-ci-artifacts-ci.sh
scripts/cad/parity-risk-register-ci.sh
scripts/cad/parity-dashboard-ci.sh
scripts/cad/parity-kernel-adapter-v2-ci.sh
scripts/cad/parity-kernel-math-ci.sh
scripts/cad/parity-kernel-topology-ci.sh
scripts/cad/parity-kernel-geom-ci.sh
scripts/cad/parity-kernel-primitives-ci.sh
scripts/cad/parity-kernel-tessellate-ci.sh
scripts/cad/parity-kernel-booleans-ci.sh
scripts/cad/parity-kernel-boolean-diagnostics-ci.sh
scripts/cad/parity-kernel-boolean-brep-ci.sh
scripts/cad/parity-kernel-nurbs-ci.sh
scripts/cad/parity-kernel-text-ci.sh
scripts/cad/parity-kernel-fillet-ci.sh
scripts/cad/parity-kernel-shell-ci.sh
scripts/cad/parity-kernel-step-ci.sh
scripts/cad/parity-kernel-precision-ci.sh
scripts/cad/parity-primitive-contracts-ci.sh
scripts/cad/parity-transform-ci.sh
scripts/cad/parity-pattern-ci.sh
scripts/cad/parity-shell-feature-graph-ci.sh
scripts/cad/parity-fillet-feature-graph-ci.sh
scripts/cad/parity-chamfer-feature-graph-ci.sh
scripts/cad/parity-expanded-finishing-ci.sh
scripts/cad/parity-sweep-ci.sh
scripts/cad/parity-loft-ci.sh
scripts/cad/parity-topology-repair-ci.sh
scripts/cad/parity-material-assignment-ci.sh
scripts/cad/parity-vcad-eval-receipts-ci.sh
scripts/cad/parity-feature-op-hash-ci.sh
scripts/cad/parity-modeling-edge-cases-ci.sh
scripts/cad/parity-core-modeling-checkpoint-ci.sh
scripts/cad/parity-sketch-entity-set-ci.sh
scripts/cad/parity-sketch-plane-ci.sh
scripts/cad/parity-sketch-constraint-enum-ci.sh
scripts/cad/parity-sketch-iterative-lm-ci.sh
scripts/cad/parity-sketch-jacobian-residual-ci.sh
scripts/cad/parity-sketch-constraint-status-ci.sh
scripts/cad/parity-sketch-extrude-ci.sh
scripts/cad/parity-sketch-revolve-ci.sh
scripts/cad/parity-sketch-sweep-ci.sh
scripts/cad/parity-blocker-workflow.sh
scripts/cad/parity_check.sh
scripts/cad/parity-ci-lane.sh
```
