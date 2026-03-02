# Baseline Parity Dashboard

Issue coverage: `VCAD-PARITY-010`

## Snapshot

| Metric | Value |
| --- | --- |
| vcad commit | `1b59e7948efcdb848d8dba6848785d57aa310e81` |
| openagents commit (plan baseline) | `04faa5227f077c419f1c5c52ddebbb7552838fd4` |
| phase status | `phase_a_baseline_complete` |
| overall match rate | `0.072464` |
| docs match rate | `0.038961` |
| crates match rate | `0.122449` |
| commands match rate | `0.083333` |
| open risks | `24` |
| open hard blockers (p0) | `16` |
| CI source artifact count | `20` |

## Profile Gates

| Lane | Profile | Pass |
| --- | --- | --- |
| `risk_register` | `parity_complete_v1` | `false` |
| `scorecard` | `parity_complete_v1` | `false` |
| `risk_register` | `phase_a_baseline_v1` | `true` |
| `scorecard` | `phase_a_baseline_v1` | `true` |

## CI Evidence Artifacts

- `fixtures_parity_fixture_corpus`
- `kernel_adapter_v2_manifest`
- `kernel_boolean_brep_parity_manifest`
- `kernel_boolean_diagnostics_parity_manifest`
- `kernel_booleans_parity_manifest`
- `kernel_geom_parity_manifest`
- `kernel_math_parity_manifest`
- `kernel_nurbs_parity_manifest`
- `kernel_precision_parity_manifest`
- `kernel_primitives_parity_manifest`
- `kernel_tessellate_parity_manifest`
- `kernel_topology_parity_manifest`
- `openagents_capabilities_inventory`
- `openagents_start_manifest`
- `parity_dashboard`
- `parity_risk_register`
- `parity_scorecard`
- `vcad_capabilities_inventory`
- `vcad_openagents_gap_matrix`
- `vcad_reference_manifest`

## Next Actions

- Execute VCAD-PARITY-011 through VCAD-PARITY-025 sequentially
- Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes
- Refresh parity dashboard after each closed parity issue
