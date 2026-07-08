# CS336 A2 Continuous Thermal-Throttle Receipt Machinery

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


Date: 2026-06-28

Promise: `training.device_capability_dataset.v1` (stays planned after the
2026-06-20 revenue-loop tightening; no green flip).

Issue lineage: `OpenAgentsInc/openagents#6853`.

## What changed

The CS336 A2 benchmark path now has source-level machinery for continuous
sustained-vs-burst thermal evidence:

- `buildCs336A2ThermalThrottleMeasurementEvidence` derives a
  `sustained_vs_burst_throughput_ratio` row from burst and sustained throughput
  samples.
- Verified thermal rows project their row `receiptRefs`.
- Dataset and dashboard projections expose `thermalThrottleReceiptRefs` and
  `thermalThrottleFunnelReasonCodes`.
- The capacity funnel accepts closed `device_capability.public.*` reason-code
  refs without treating them as private hardware material.

## Receipt boundary

This is receipt-first machinery only. It does not mint a production receipt,
settle a benchmark assignment, or flip the product promise green by itself. The
blocker `blocker.product_promises.thermal_throttle_detection_missing` remains
until a real production contributor run records an owner-accepted verified
thermal-row receipt and any required claim transition is recorded under
`proof.claim_upgrade_receipts.v1`.

## Public safety

Thermal projections carry class-level ratios, refs, reason codes, and receipt
refs only. They do not expose device identifiers, owner refs, wallet material,
payment material, raw runner logs, host paths, or private hardware payloads.

## Verification

- `apps/openagents.com/workers/api/src/cs336-a2-benchmark-workload.test.ts`
- `apps/openagents.com/workers/api/src/training-device-capability.test.ts`
- `apps/openagents.com/workers/api/src/training-run-window-routes.test.ts`
- `apps/openagents.com/workers/api/src/pylon-capacity-funnel.test.ts`

