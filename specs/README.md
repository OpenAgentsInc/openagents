# Formal Specs

This directory is the bounded TLA+ tier for `docs/fable/ROADMAP.md` task
T6.13 / GitHub issue #7857. These models are design and regression checks only:
they can inform runtime code, tests, and QA fixtures, but they never authorize
runtime behavior, broaden product claims, or weaken the repository invariants.

## Running TLC

```sh
specs/run-tlc.sh
```

The runner expects a standard `tlc` command. It runs the accepted bounded
models first, then runs the mutation models under `specs/mutations/` and fails
unless TLC reports a violation for each mutation. Deadlock checking is left on.

## Property Map

| Spec | Checked properties | Source seams |
| --- | --- | --- |
| `khala-fleet-delegate/FleetDelegateSupervisor.tla` (`FleetDelegateSupervisor.cfg`) | `ActiveAssignmentsNeverExceedAdvertisedCapacity`, `ClaimUniquenessUnderRacingSupervisors`, `PausedRunsClaimNothing`, `SupervisorReviveHonorsAutoRevivableGuard` | `clients/khala-code-desktop/src/bun/fleet-run-supervisor.ts`; `apps/pylon/src/orchestration/store.ts` (`tryClaimWorkUnit`, `expireWorkClaims`, `releaseWorkClaim`, `reconcileWorkClaims`, `isAutoRevivableFleetRun`) |
| `khala-fleet-delegate/FleetDelegateSupervisor.tla` (`FleetDelegateSupervisorLiveness.cfg`) | `TerminationUnderBoundedClaims`, `DrainEventuallyTerminates` | Same supervisor/store seams; liveness is checked at a smaller bound so the racing/TTL/reclaim safety state space remains practical. The termination property has no `phase = "idle"` disjunct; operator lifecycle intervention is modeled separately as authority. |
| `approval-protocol/ApprovalProtocol.tla` | `NoLostApprovals`, `NoDuplicateApprovalResponses`, `NoStaleApproveApplication` (action property: approve applies only at the recorded epoch), `AllIssuedRequestsEventuallyClose` (leads-to, falsifiable) | Desktop approval RPCs, Inbox `approval_required`, Codex/Claude approval bridges. `StaleApproveAttempt` models the stale retry; the mutation drops Approve's epoch guard in a copy of THIS spec. |
| `session-thread-mapping/SessionThreadMapping.tla` | `NoOrphanThreadBinding`, `NoDoubleBind`, `PersistedMappingConsistent`, `CrashReloadEventuallyRestoresBindings` | Khala Code Desktop session catalog, thread list reload/reconcile, and session persistence paths. `Bind` and `PersistBinding` are split so crash-before-persist divergence is reachable. |

## Mutation Proofs

The checked mutation variants are under `specs/mutations/`:

- `fleet-paused-claim`: allows a stale paused-run claim and TLC violates
  `PausedRunsClaimNothing`.
- `fleet-operator-revive`: in-place copy of the main fleet spec with the
  `stateSource # "operator"` conjunct dropped from `AutoRevivable`; TLC
  violates `SupervisorReviveHonorsAutoRevivableGuard` (the #7975/#7978 fix).
- `approval-stale-forgery`: accepts an approval after interruption and TLC
  violates `NoStaleApproveApplication`.
- `session-crash-divergence`: reloads only one side of the session/thread map
  and TLC violates `NoOrphanThreadBinding`.

Known modeling limit: `ClaimUniquenessUnderRacingSupervisors` holds largely
by construction — per-unit claim state is a single function entry, so the
SQLite partial-unique-index insert race inside `tryClaimWorkUnit` is not
representable in this bounded model. Treat that invariant as documenting the
intended shape, not as certifying the store's concurrency control; the
store-level tests own that seam.

The JSON files under `fixtures/counterexamples/` are public-safe fixture seeds
that point to those checked mutation actions for future scenario/model ports.
