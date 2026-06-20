# CS336 A2 Same-Class Replication Status

Promise: `training.device_capability_dataset.v1` (stays **yellow**; no green
flip).

Registry: `2026-06-20.34`.

## What changed

The public A2 device-capability projection now reports same-class replication
status explicitly instead of hiding it behind the generic benchmark blocker.

New public fields on each run-level dataset projection and on the aggregate
`GET /api/training/device-capabilities/a2` dashboard:

- `sameClassReplicationStatus`
- `sameClassReplicationSignals`
- `sameClassReplicationBlockerRefs`

Each measurement row also carries fail-closed replication fields:

- `sameClassReplicationScope`
- `sameClassReplicationStatus`
- `sameClassReplicationBlockerRefs`

Legacy settled rows default to `cross_process_same_host`, not cross-machine.
That matches the known first A2 closeouts: two registered Pylons on the same
physical host. A row becomes `cross_machine_replicated` only when its evidence
declares `sameClassReplicationScope: "cross_machine_same_class"`.

Measured-but-unsettled rows default to `single_observation`, which preserves
the x86_64 Linux Intel row as real measured data but not replicated, verified,
paid, or settled coverage.

## Why this matters

Before this pass, a statistically cross-checked same-host row could produce an
empty route-level `blockerRefs` array even though the product promise still
correctly carried `blocker.product_promises.same_host_replication_caveat`.

Now the public route is consistent with the promise copy:

- same-host-only rows report `sameClassReplicationStatus: "same_host_only"`
- the route carries
  `blocker.cs336_a2.requires_cross_machine_same_class_replication`
- only explicitly cross-machine same-class evidence clears the replication
  blocker in the projection

## Boundaries

This creates no paid assignment, verification verdict, settlement, earning
estimate, new device-class claim, or green transition.

Product blockers remain:

- `blocker.product_promises.thermal_throttle_detection_missing`
- `blocker.product_promises.same_host_replication_caveat`

Green still requires paid and statistical-cross-check verified coverage across
at least two distinct device classes, continuous thermal-throttle evidence, and
cross-machine same-class replication receipts.

## Tests

- `apps/openagents.com/workers/api/src/training-device-capability.test.ts`
- `apps/openagents.com/workers/api/src/training-run-window-routes.test.ts`
- `apps/openagents.com/workers/api/src/product-promises.test.ts`
