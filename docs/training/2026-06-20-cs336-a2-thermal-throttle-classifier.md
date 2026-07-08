# CS336 A2 Device-Capability Dataset: Thermal-Throttle Classifier

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


Date: 2026-06-20

Promise: `training.device_capability_dataset.v1` (stays **yellow**; no green
flip). Registry edit: `2026-06-20.26`.

Issue lineage: `OpenAgentsInc/openagents#5528`, `#4852`.

## What this advances

The device-capability dataset already accepted the
`sustained_vs_burst_throughput_ratio` probe kind, but the public A2 projection
did not classify that row into a thermal-throttle status. This pass adds a
deterministic public classifier to the dataset and dashboard projection:

- `thermalThrottleSignals`
- `thermalThrottleDetectionStatus`
- `thermalThrottleBlockerRefs`

The classifier consumes only admitted public-safe class-level distributions for
`sustained_vs_burst_throughput_ratio`. It does not read device identifiers,
owner refs, raw benchmark payloads, wallet material, or payment material.

## Classifier rule

The thermal ratio floor is fixed at `0.8`, matching the existing reasoned
admission-gate example for sustained collective training:

- If no sustained-vs-burst row exists, status is `missing` with
  `blocker.cs336_a2.requires_sustained_vs_burst_thermal_probe`.
- If a sustained-vs-burst row exists but is not cross-check verified, status is
  `needs_verified_thermal_probe` with
  `blocker.cs336_a2.requires_verified_sustained_vs_burst_thermal_probe`.
- If a verified sustained-vs-burst row has `p50 < 0.8`, the signal reports
  `thermal_throttle_observed`.
- If a verified sustained-vs-burst row has `p50 >= 0.8`, the signal reports
  `thermal_throttle_not_observed`.

Measured-but-unsettled rows are never allowed to become a positive thermal
claim; they can only report that the thermal probe still needs verification.

## Honest remainder

This does **not** clear
`blocker.product_promises.thermal_throttle_detection_missing` yet. The live
registry still has no production row from a real contributor device reporting a
verified sustained-vs-burst thermal probe, and this classifier is a projection
and admission rule rather than continuous fleet monitoring.

Green for `training.device_capability_dataset.v1` still requires paid and
statistical-cross-check verified coverage on at least two distinct device
classes, continuous thermal-throttle evidence feeding the public reason codes,
and cross-machine replication. The same-host replication caveat remains.

## Verification

Code and tests:

- `apps/openagents.com/workers/api/src/training-device-capability.ts`
- `apps/openagents.com/workers/api/src/training-device-capability.test.ts`
- `apps/openagents.com/workers/api/src/training-run-window-routes.ts`
- `apps/openagents.com/workers/api/src/training-run-window-routes.test.ts`

The tests cover:

- benchmark-only A2 rows reporting thermal status `missing`
- verified sustained-vs-burst rows reporting `thermal_throttle_not_observed`
- measured-unsettled thermal rows reporting `needs_verified_thermal_probe`
- the public dashboard carrying the thermal status fields

