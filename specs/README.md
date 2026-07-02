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
| `approval-protocol/ApprovalProtocol.tla` | `NoLostApprovals`, `NoDuplicateApprovalResponses`, `NoStaleRequestForgery`, `AllIssuedRequestsEventuallyClose` | Desktop approval RPCs, Inbox `approval_required`, Codex/Claude approval bridges. `StaleApproveRejected` is a modeled stale-approve action and is mutation-tested. |
| `session-thread-mapping/SessionThreadMapping.tla` | `NoOrphanThreadBinding`, `NoDoubleBind`, `PersistedMappingConsistent`, `CrashReloadEventuallyRestoresBindings` | Khala Code Desktop session catalog, thread list reload/reconcile, and session persistence paths. `Bind` and `PersistBinding` are split so crash-before-persist divergence is reachable. |

## Mutation Proofs

The checked mutation variants are under `specs/mutations/`:

- `fleet-paused-claim`: allows a stale paused-run claim and TLC violates
  `PausedRunsClaimNothing`.
- `approval-stale-forgery`: accepts an approval after interruption and TLC
  violates `NoStaleRequestForgery`.
- `session-crash-divergence`: reloads only one side of the session/thread map
  and TLC violates `NoOrphanThreadBinding`.

The JSON files under `fixtures/counterexamples/` are public-safe fixture seeds
that point to those checked mutation actions for future scenario/model ports.
