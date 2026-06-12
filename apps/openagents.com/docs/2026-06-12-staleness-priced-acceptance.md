# Staleness-Priced Acceptance: steps_behind at Decision Time (Pluralis Roadmap P2.2)

Date: 2026-06-12
Issue of record: openagents#4853 (master tracking issue openagents#4855)
Rails: #4673 (training run/window authority), #4674 (verification classes)
Roadmap source: `docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md`
(workspace `openagents` repo), item P2.2
Predecessors: P0.2 staleness contract (#4849,
`docs/2026-06-12-window-seal-staleness-contract.md`), P0.1 join ladder
(#4848, `docs/2026-06-12-pylon-join-lifecycle-ladder.md`), P1.2/P1.3
bootstrap and seal barrier (#4850/#4851,
`docs/2026-06-12-bootstrap-from-seal-and-join-barrier.md`)

## The rule

P0.2 made `steps_behind` a contract field on the window-seal record. This
change makes it load-bearing at acceptance time. Every contribution carries
`stepsBehind` (how many optimizer steps behind the window head its base
state was); each verification class carries a staleness dimension; and a
contribution **beyond** the effective threshold routes to `sync_reentry` —
re-ramped through the shadow window (P1.1, psionic#1125) — rather than
being **rejected** (wasting a willing device) or **merged** (importing
divergence). The routing emits a typed event, never a bare rejection. The
decision union has no reject arm at all, so staleness alone cannot produce
a rejection **by type**.

This is the AsyncPP lesson applied to dispatch rather than to the
optimizer: measure the delay, respond to it, never pretend it is zero.

## AsyncPP citation

Pluralis Research, *Nesterov Method for Asynchronous Pipeline Parallel
Optimization* (Ajanthan, Ramasinghe, Zuo, Avraham, Long), ICML 2025,
arXiv:2505.01099. Read-only reference lane:
`projects/pluralis/repos/AsyncPP` (workspace root). The paper's core move
is to treat gradient delay as a first-class measured quantity the optimizer
compensates for, instead of assuming synchronous freshness. Pluralis node0
ships `max_allowed_stale: 5` as the dispatch-side prior art; our run-level
default (`DefaultMaxAllowedStaleSteps = 5` in
`workers/api/src/training-run-window-authority.ts`) remains a stated
per-run contract value, not an inherited constant.

## Decision function

`decideTrainingStalenessAcceptance` in
`workers/api/src/training-staleness-acceptance.ts` — a pure function;
timestamps and event ids are passed in, never read from a clock.

Inputs: `contributionRef`, `stepsBehind`, the `TrainingRunRecord`
(supplies `maxAllowedStale` and `trainingRunRef`), the verification class,
and optionally a registry override.

### Decision table

| Condition | Outcome |
| --- | --- |
| `stepsBehind` is not an integer in `[0, 1_000_000]` | typed `TrainingStalenessAcceptanceError` (`validation_error`) — malformed input, not a staleness verdict |
| `stepsBehind <= effectiveMax` | `{ outcome: 'accept' }` with `effectiveMaxStepsBehind` and `thresholdSource` recorded |
| `stepsBehind > effectiveMax` | `{ outcome: 'sync_reentry' }` with `reasonCode: 'join_lifecycle.public.beyond_max_allowed_stale'` and a typed `TrainingStalenessRoutingEvent` |
| (any staleness value) | **never** a bare reject — the union has no reject arm |

### Boundary decision (documented and tested)

A contribution at **exactly** `maxAllowedStale` is **accepted**. The
contract reads "beyond max_allowed_stale" — strictly greater — matching
the run-authority comment ("more than this many steps behind") and the
Pluralis `max_allowed_stale` reading. `stepsBehind = 5` against the
default run contract is the last accepted value; `6` routes to
sync re-entry.

### Effective threshold and clamping

`effectiveMaxStepsBehindFor(policy, runMaxAllowedStale)`:

- no policy or `inherit_run_default` → run's `maxAllowedStale`,
  `thresholdSource: 'run_default'`;
- `max_steps_behind_override` **tighter** than the run default → the
  override, `thresholdSource: 'class_override'`;
- an override **looser** than the run default clamps to the run default
  and reports `run_default` — the run-level contract is a ceiling, so a
  verification class can never import more divergence than the run
  contract allows.

## Per-class staleness dimension

`TrainingVerificationRegistration` in
`workers/api/src/training-verification.ts` gains an additive optional
`stalenessPolicy` field (`inherit_run_default` |
`max_steps_behind_override`). Current default registry values:

| Verification class | Staleness policy | Effective vs default run (5) | Rationale |
| --- | --- | --- | --- |
| `deterministic_recompute` | inherit run default | 5 | verifies against the contribution's own declared base; staleness threatens merge divergence, not verification power |
| `exact_trace_replay` | inherit run default | 5 | same: replay is pinned to the declared trace, not the live head |
| `freivalds_merkle` | inherit run default | 5 | algebraic check against committed matrices; head-independent |
| `seeded_replication` | override `maxStepsBehind: 3` | 3 | replication with pinned seeds tolerates some drift, but compares against near-head behavior |
| `statistical_cross_check` | override `maxStepsBehind: 2` | 2 | distribution-level comparison loses discriminating power fastest as the base state diverges from the head |

These per-class values are provisional engineering judgments, revisable as
run-config changes with their own receipts once sealed staleness
distributions (P0.2) accumulate.

## sync_reentry routing composes with the join ladder

`routeTrainingStalenessSyncReentry` composes a `sync_reentry` decision
with the join-lifecycle ladder (P0.1, #4848,
`workers/api/src/pylon-join-lifecycle.ts`), reusing the ladder's existing
closed reason-code taxonomy rather than inventing a parallel staleness
vocabulary:

- an `active` contributor walks the back edge in two reason-coded
  transitions: `active -> lagged`
  (`join_lifecycle.public.beyond_max_allowed_stale`) then
  `lagged -> sync_reentry`
  (`join_lifecycle.public.sync_reentry_started`), each emitting a
  receipt-compatible lifecycle event;
- a contributor already `lagged` takes only the second edge;
- a contributor already in `sync_reentry` gets an idempotent
  `already_in_sync_reentry` outcome (not an error);
- states with no ladder edge toward re-entry (`registered`, `qualified`,
  `state_synced`, `warmup`) get a typed `not_routable` outcome — only a
  device that has been on the live ladder can have produced a stale
  contribution.

From `sync_reentry` the device re-ramps through the existing
`sync_reentry -> state_synced` edge and the warmup/shadow path; the
willing device is recycled, not discarded.

## Pre-registered falsifier

If per-contribution staleness accounting generates more
dispute/adjudication overhead than the divergence it prevents at R2
scale, simplify to a binary fresh/stale gate at `max_allowed_stale` and
record the simplification in the seal-record schema rev. (Roadmap
"Staleness pricing (P2.2)" falsifier, restated here as the kill
condition for the per-class dimension.)

## Tests

`workers/api/src/training-staleness-acceptance.test.ts` covers: fresh
contribution accepted under the run default; boundary value (exactly
`maxAllowedStale`) accepted; over-stale contribution routed to
`sync_reentry` with the full typed event (never a bare reject);
class-level override (`statistical_cross_check`, 2) beating the run
default (5); a loosening override clamped to the run ceiling; typed
validation errors for non-integer/negative `stepsBehind`; threshold
resolution unit cases; and lifecycle composition (active routed through
lagged into sync_reentry with both reason-coded events, lagged routed in
one edge, sync_reentry idempotent, registered typed `not_routable`).

## Explicitly not claimed (hardware/rehearsal-gated)

- **R2-scope receipts showing a real contribution re-ramped** through
  sync re-entry: requires live contributor devices at R2 scale; no such
  receipt is claimed here. The decision and routing are contract-tested
  only.
- **Shadow-window ramp execution** (P1.1) is the psionic-side companion
  (psionic#1125), out of scope for this monorepo change.
- **Optimizer-side delay compensation** (the actual AsyncPP Nesterov
  correction) is psionic window-merge work, not dispatch work.
