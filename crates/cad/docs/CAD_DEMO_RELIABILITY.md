# CAD 20-Second Demo Reliability Test

This document defines the deterministic reliability lane for the scripted
20-second CAD demo flow.

## Test

- `cad_demo_20s_reliability_script_has_no_stalls_flicker_or_state_loss`
- Location:
  - `apps/autopilot-desktop/src/input/reducers/cad.rs`

## Script Fixture

- `apps/autopilot-desktop/tests/scripts/cad_demo_reliability_20s_script.json`

Flow covered:

1. Create rack spec intent.
2. Generate variants intent.
3. Cycle lightweight/low-cost/stiffness variants.
4. Manual typed dimension edit and rebuild.
5. Timeline row selection.
6. Warning escalation assertion.

## Deterministic Pass/Fail Criteria

- No stalls:
  - each scripted step duration must remain below hard threshold.
- No flicker:
  - variant/dimension rebuild steps must retain mesh payload and stable
    `last_good_mesh_id` with no pending rebuild request at step completion.
- No state loss:
  - monotonic non-regressing state revisions.
  - rebuild receipts present for each rebuild-producing step.
  - no terminal `last_error`.
- No budget regression:
  - Gate A/B/E benchmark snapshot from final state must pass.

## Run

```bash
scripts/cad/reliability-20s-ci.sh
```
