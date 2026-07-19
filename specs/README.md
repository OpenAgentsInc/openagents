# OpenAgents specs

This root contains two complementary spec tiers:

- `.product-spec.md` intent artifacts under `specs/<area>/`, governed by
  [`CONVENTIONS.md`](CONVENTIONS.md) and validated by
  `packages/product-spec`. And
- bounded TLA+ models and mutation checks described below.

The current owner-delegated completion program is recorded in
[`../AUTHORITY.md`](../AUTHORITY.md) and
[`../docs/sol/2026-07-18-owner-delegated-autonomy-accepted-plan.md`](../docs/sol/2026-07-18-owner-delegated-autonomy-accepted-plan.md).
That authority permits agents to implement and prove the specs. It does not
let implementation silently rewrite intent or let a model authorize runtime
behavior.

## Product intent tier

The current ProductSpec areas are Desktop, mobile, OpenAgents-wide, and web.
Each consequential outcome keeps its intent here while the Sol roadmap, live
issues/packets, implementation, AssuranceSpec, formal models where warranted,
and product-promise registry retain their separate authorities. Validate the
tree with:

[`openagents/cursor-capability-parity.product-spec.md`](openagents/cursor-capability-parity.product-spec.md)
is the cross-surface breadth contract for competing with Cursor. It requires a
current evidence-pinned capability ledger and equivalent supported outcomes
across Desktop, web, mobile, CLI, background execution, automation, ecosystem,
and data lifecycle. Sibling surface specs own their exact interactions and
proof. The parity spec prevents stronger trust architecture from being used as
an excuse for missing capability. It does not claim the capabilities have
shipped or replace roadmap, assurance, acceptance, or release authority.

Revision 3 of that parity contract and revision 7 of the Desktop workbench
contract define the IDE bar as Zed-quality integration over one generation-
fenced project/evidence graph. Portable Sessions revision 4 owns IDE-13.
mobile revision 7 and web revision 7 own their bounded IDE-14 projections.
The canonical dependency sequence, built-in Vim contract, initial Tokyo Night
theme contract, and release-rung vocabulary live in
[the IDE roadmap](../docs/ide/ROADMAP.md). The exhaustive ProductSpec/
AssuranceSpec/packet mapping lives in
[`IDE_ROADMAP_CROSSWALK.md`](IDE_ROADMAP_CROSSWALK.md).

[`openagents/managed-agent-sandboxes.product-spec.md`](openagents/managed-agent-sandboxes.product-spec.md)
revision 1 now owns the concrete OpenAgents-managed GCP sandbox used by
IDE-13/IDE-17, Sarah revision 4, and bounded mobile/web supervision. It keeps
Google Cloud and OpenAgents workrooms authoritative while treating the Ascii
Box v1 API and exact TypeScript SDK as an isolated compatibility target. Full
Auto revision 14 remains unchanged and still excludes cross-machine run
admission. A managed sandbox work unit is not a remote `FullAutoRun` by
inference. Epic [#9023](https://github.com/OpenAgentsInc/openagents/issues/9023)
and its accepted plan own implementation sequence, not this index.

Effect/TypeScript owns application authority and persistence. Every boundary
contract is Effect Schema-first with derived types. Capability lifecycle uses
Effect services/layers/scopes. Rust is limited to supervised authority-free
native helpers. Mobile receives bounded review projections, and web/public
links receive allowlisted verifiable code-share bundles without editor or
execution authority. Tokyo Night is the one initial owned Desktop IDE theme.
Vim is built in and off by default. Broader light/high-contrast/system theme
support remains required before the complete-accessibility/full-parity gate.

Two exact-subject proposed proof-design companions now cover the reconciled IDE
contracts:

- [`desktop/desktop-trust-complete-workbench.assurance-spec.md`](desktop/desktop-trust-complete-workbench.assurance-spec.md)
  binds Desktop ProductSpec revision 7. And
- [`openagents/cursor-capability-parity.assurance-spec.md`](openagents/cursor-capability-parity.assurance-spec.md)
  binds Cursor parity ProductSpec revision 3.

Both are deterministic proposal skeletons: every criterion is represented,
but proof design remains `needs_design`. Neither is admitted, authorized to
execute, observed, owner-accepted, or release evidence. The existing Full Auto
AssuranceSpec revision 4 remains exactly bound to unchanged Full Auto
ProductSpec revision 14 and supplies the independent IDE-17 dependency. Frozen
MVP ProductSpecs and AssuranceSpecs remain historical exact-byte subjects and
are not rewritten as IDE proof.

```sh
node --import tsx packages/product-spec/src/cli.ts validate --specs-root specs
```

Validate either proposed IDE AssuranceSpec structurally with:

```sh
node --import tsx packages/assurance-spec/src/cli.ts validate \
  specs/desktop/desktop-trust-complete-workbench.assurance-spec.md
node --import tsx packages/assurance-spec/src/cli.ts validate \
  specs/openagents/cursor-capability-parity.assurance-spec.md
```

## Formal model tier

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

| Spec                                                                                       | Checked properties                                                                                                                                                                                       | Source seams                                                                                                                                                                                                                                                       |
| ------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `khala-fleet-delegate/FleetDelegateSupervisor.tla` (`FleetDelegateSupervisor.cfg`)         | `ActiveAssignmentsNeverExceedAdvertisedCapacity`, `ClaimUniquenessUnderRacingSupervisors`, `PausedRunsClaimNothing`, `SupervisorReviveHonorsAutoRevivableGuard`                                          | `clients/khala-code-desktop/src/bun/fleet-run-supervisor.ts`. `apps/pylon/src/orchestration/store.ts` (`tryClaimWorkUnit`, `expireWorkClaims`, `releaseWorkClaim`, `reconcileWorkClaims`, `isAutoRevivableFleetRun`)                                               |
| `khala-fleet-delegate/FleetDelegateSupervisor.tla` (`FleetDelegateSupervisorLiveness.cfg`) | `TerminationUnderBoundedClaims`, `DrainEventuallyTerminates`                                                                                                                                             | Same supervisor/store seams. Liveness is checked at a smaller bound so the racing/TTL/reclaim safety state space remains practical. The termination property has no `phase = "idle"` disjunct. Operator lifecycle intervention is modeled separately as authority. |
| `approval-protocol/ApprovalProtocol.tla`                                                   | `NoLostApprovals`, `NoDuplicateApprovalResponses`, `NoStaleApproveApplication` (action property: approve applies only at the recorded epoch), `AllIssuedRequestsEventuallyClose` (leads-to, falsifiable) | Desktop approval RPCs, Inbox `approval_required`, Codex/Claude approval bridges. `StaleApproveAttempt` models the stale retry. The mutation drops Approve's epoch guard in a copy of THIS spec.                                                                    |
| `session-thread-mapping/SessionThreadMapping.tla`                                          | `NoOrphanThreadBinding`, `NoDoubleBind`, `PersistedMappingConsistent`, `CrashReloadEventuallyRestoresBindings`                                                                                           | Khala Code Desktop session catalog, thread list reload/reconcile, and session persistence paths. `Bind` and `PersistBinding` are split so crash-before-persist divergence is reachable.                                                                            |

## Mutation Proofs

The checked mutation variants are under `specs/mutations/`:

- `fleet-paused-claim`: allows a stale paused-run claim and TLC violates
  `PausedRunsClaimNothing`.
- `fleet-operator-revive`: in-place copy of the main fleet spec with the
  `stateSource # "operator"` conjunct dropped from `AutoRevivable`. TLC
  violates `SupervisorReviveHonorsAutoRevivableGuard` (the #7975/#7978 fix).
- `approval-stale-forgery`: accepts an approval after interruption and TLC
  violates `NoStaleApproveApplication`.
- `session-crash-divergence`: reloads only one side of the session/thread map
  and TLC violates `NoOrphanThreadBinding`.

Known modeling limit: `ClaimUniquenessUnderRacingSupervisors` holds largely
by construction — per-unit claim state is a single function entry, so the
SQLite partial-unique-index insert race inside `tryClaimWorkUnit` is not
representable in this bounded model. Treat that invariant as documenting the
intended shape, not as certifying the store's concurrency control. The
store-level tests own that seam.

The JSON files under `fixtures/counterexamples/` are public-safe fixture seeds
that point to those checked mutation actions for future scenario/model ports.
