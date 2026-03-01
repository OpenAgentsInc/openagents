# Memory Results (Spike Snapshot)

## Summary

Only coarse bootstrap-time signals are available from this short spike. Full corpus memory tracking is deferred.

## Observed Signals

| Option | Peak memory signal | Source |
|---|---|---|
| A: VCAD subset | Not measured (bootstrap failure before stable run) | `artifacts/vcad_kernel_check.log` |
| B: OpenCascade bindings | Not measured (build failure before stable run) | `artifacts/opencascade_check.log` |
| C: In-house minimal B-Rep | Not measured | Not executed |
| D: CSG then exact later | Not measured | Not executed |

## Follow-up measurement contract

- Record per-corpus peak RSS for `RACK-01..RACK-05`.
- Track repeated rebuild loop memory stability (100 rebuild iterations).
- Compare against Wave 1 memory budget target in plan.
