# FC-4: hybrid fleet routing — desktop Pylons plus Agent Computers

Parent: #8638

## Outcome

One Sarah FleetRun can place independent work units on owner-local Pylons or
OpenAgents Agent Computers under an explicit typed target policy.

## Dependencies

- FC-1 durable run contract.
- FC-2 real local mixed-harness execution.
- #8547 Agent Computer Codex proof lane for brokered provider
  credentials and reproducible rootfs.

## Scope

1. Add a per-work-unit execution target policy:
   `owner_local | managed_cloud | auto`, with typed fallback history.
2. Keep authority rails distinct. Owner-local subscription accounts are never
   pooled or resold; managed cloud uses brokered owner grants and compute
   receipts.
3. Complete Codex in Firecracker first, then Claude and Grok through the same
   work-unit/closeout contract rather than provider-specific cloud APIs.
4. Let the planner use live capacity, quota, cost class, data posture, and task
   constraints. V1 may be deterministic and simple.
5. Preserve one claim registry across targets so local and cloud workers cannot
   take the same unit.
6. Normalize safe progress and closeout projections while retaining target-
   specific private evidence.

## Exit

One Sarah-created run completes at least two real units concurrently: one on a
desktop Pylon and one inside an Agent Computer. The canvas identifies the
target class and fallback history; owner scope, broker refs, exact token truth,
compute lifecycle receipts, verification, and reclaim all reconcile.
