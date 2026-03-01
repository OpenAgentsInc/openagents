# Boolean Failure Log (Spike Snapshot)

## Summary

No full boolean corpus execution was possible in this short spike due environment-level kernel bootstrap blockers.

## Observed Results

| Option | Status | Evidence |
|---|---|---|
| A: VCAD subset | Blocked before boolean execution | `artifacts/vcad_kernel_check.log` (missing `tang` workspace dependency) |
| B: OpenCascade bindings | Blocked before boolean execution | `artifacts/opencascade_check.log` (`occt-sys`/CMake failure) |
| C: In-house minimal B-Rep | Not executed | Deferred by design; high implementation risk |
| D: CSG then exact later | Not executed | Deferred by design; weak STEP credibility for Wave 1 |

## Action

Re-run this log with real corpus cases (`RACK-01..RACK-05`) once kernel adapter wiring (issue #2454) is complete.
