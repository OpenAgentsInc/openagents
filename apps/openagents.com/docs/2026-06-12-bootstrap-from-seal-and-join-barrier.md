# Bootstrap From the Last Durable Seal and the Join-Blocking Window (Pluralis Roadmap P1.2/P1.3)

Date: 2026-06-12
Issues of record: openagents#4850 (P1.2) and openagents#4851 (P1.3); master
tracking issue openagents#4855
Rails: #4673 (training run/window authority), dispatcher pattern #4639
Roadmap source: `docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md`
(workspace `openagents` repo), items P1.2 and P1.3

## The two rules

**Rule 1 — snapshot lags live (P1.2, #4850).** A joining device bootstraps
from the run's **last durable seal**: the most recently sealed (or already
reconciled) window whose seal record carries a durably stored checkpoint
digest. A joiner never receives in-flight state, and never chases the live
run; it syncs forward from the seal via the shadow-window ramp (psionic
companion, roadmap P1.1, psionic#1125). The grant pins the seal's checkpoint
digest, and the joiner's acceptance must echo that digest exactly before any
work is assigned. How often a durable seal is published is now a stated
per-run contract value (`sealPublicationCadenceWindows`), because the
publication cadence bounds how far behind a fresh joiner starts.

**Rule 2 — the join-blocking window (P1.3, #4851).** While a merge/seal
operation is in flight, the dispatcher hands out **no state download and no
join-lifecycle transition**. Requests that arrive during the barrier are
**queued with a typed reason code, not rejected**: the outcome kind is
`queued` and the reason code is
`join_lifecycle.public.join_deferred_seal_in_flight`, drawn from the same
closed join-lifecycle taxonomy the ladder already publishes (P0.1, #4848).
Once the barrier clears, the identical request replays and proceeds against
the **new** last durable seal — the one the just-finished operation
produced, if it published a digest.

## Pluralis sources (agora)

- `docs/agora-system/startup-sequence.md` (state download): "The S3
  snapshot lags the live parameter state; it is published periodically, not
  on every step." Joiners download a deliberately stale snapshot and sync
  forward through the ramp; they never read live state.
- `docs/agora-system/fault-tolerance.md` (Join Blocking Window): "a peer
  joining mid-`AllReduce` … would read parameters that are neither the
  pre-round nor the post-round model. To prevent that, new joins are blocked
  during the steps immediately around an averaging round." Our equivalent
  failure is a joiner bootstrapping while a seal is being written. Cheap at
  R1 scale, load-bearing at R2.

## What changed

### Authority contract (`workers/api/src/training-run-window-authority.ts`)

- `TrainingWindowSealMetadata` gains an optional `checkpointDigestRef`
  (public-safe ref shape, validated at seal time). A sealed window without a
  checkpoint digest is **not** bootstrap material: the digest is what the
  joiner verifies against, so a digestless seal proves nothing to a joiner.
- `TrainingRunRecord` gains `sealPublicationCadenceWindows` (integer,
  1–10,000, default 1 = every window publishes a durable seal) and
  `sealInFlightAt` (nullable ISO timestamp; the run-level barrier marker).
  Both are persisted in D1
  (migration `0175_training_run_bootstrap_seal_barrier.sql`) and surfaced in
  the public run projection as `sealPublicationCadenceWindows` and the
  boolean `sealInFlight`.
- `TrainingRunPlanRequest` accepts `sealPublicationCadenceWindows`; runs
  that seal less often than every window must declare it so the
  joiner-staleness bound stays a stated per-run contract value.
- The store gains `beginRunSealBarrier(trainingRunRef, nowIso)` /
  `clearRunSealBarrier(trainingRunRef)`.

### Dispatcher rules (`workers/api/src/training-window-bootstrap.ts`)

Pure functions over authority records, timestamps always passed in:

- `selectLastDurableSealWindow` — newest sealed/reconciled window with a
  durably stored checkpoint digest (ordered by `sealedAt`, window-ref
  tiebreak).
- `decideTrainingWindowBootstrapGrant` — one typed outcome per request:
  - `queued` with `join_lifecycle.public.join_deferred_seal_in_flight` when
    the run's seal barrier is up;
  - `refused` with `training.bootstrap.public.no_durable_seal` when no
    durable seal exists (typed refusal, never a fabricated grant);
  - `granted` otherwise, with the grant pinning the seal's
    `checkpointDigestRef`, the sealed window ref, the seal's receipt refs,
    the joiner's echoed receipt refs, and a display-only seal age.
- `validateTrainingWindowBootstrapAcceptance` — the digest-echo contract: an
  acceptance must name the exact grant, the granted joiner, and echo the
  granted checkpoint digest exactly, or it is rejected with one of the typed
  `training.bootstrap.public.*_mismatch` codes. Work assignment follows
  acceptance, never precedes it.
- `applyPylonJoinLifecycleTransitionUnderSealBarrier` — join-lifecycle
  transitions obey the same barrier: seal in flight means the transition is
  queued with the same deferral reason code; the caller replays the
  identical transition input once the barrier clears.

### Barrier representation (design decision)

The barrier is the durable `training_runs.seal_in_flight_at` column, not an
in-memory flag. The seal route raises it **before** persisting the seal
transition and lowers it after the write completes (success or observed
failure — `transitionWindow` persists through one atomic D1 batch, so an
observed failure leaves no partial state). An **unobserved** crash mid-seal
conservatively leaves the barrier up, which fails toward queueing joiners
rather than letting one bootstrap from an unverified seal. Queued requests
are not stored server-side: `queued` is a typed, replayable outcome —
the joiner (or its dispatcher) replays the identical request after the
barrier clears, and the replay naturally lands on the new last durable seal.
This keeps the worker stateless about waiters while keeping the wait visible
through the join-lifecycle reason-code taxonomy.

### Route and OpenAPI surface

- `POST /api/training/runs/{trainingRunRef}/bootstrap-grant`
  (`workers/api/src/training-run-window-routes.ts`): decodes a
  `TrainingWindowBootstrapGrantRequest` (`joinerRef`, optional public-safe
  `receiptRefs`), reads the run and a bounded window list (limit 100), and
  returns the typed outcome envelope. Grants carry no payout, settlement, or
  wallet authority.
- The seal transition route wraps the seal write in
  `beginRunSealBarrier` / `clearRunSealBarrier`.
- Both schemas and the route are documented in
  `workers/api/src/openagents-openapi.ts`.
- The reason code `join_lifecycle.public.join_deferred_seal_in_flight` joins
  the closed taxonomy in `workers/api/src/pylon-join-lifecycle.ts` as a
  queue-visibility code; it is deliberately **not** a ladder edge — no
  transition in the closed set carries it.

## Tests

`workers/api/src/training-window-bootstrap.test.ts` (unit) and
`workers/api/src/training-run-window-routes.test.ts` (route-level) cover:
grant pinned to the latest durable seal; digestless seals skipped; typed
refusal when no durable seal exists; queued outcome with the deferral reason
code while the barrier is up (including the seal route raising/clearing the
barrier, observed via an instrumented store); the replayed request
proceeding against the new last durable seal after the barrier clears;
digest-echo acceptance plus all three typed rejection codes; join-lifecycle
transitions queued under the barrier and applied on replay; and cadence
config validation (invalid cadence rejected, declared cadence and default
projected publicly).

## Explicitly not claimed (hardware/rehearsal-gated)

- **R1 rehearsal join-from-seal cycle with receipts** (#4850 acceptance,
  third bullet): requires live operator devices; no rehearsal receipt is
  claimed here.
- **R1 rehearsal receipt demonstrating a join correctly deferred across a
  seal boundary** (#4851 acceptance, third bullet): same gating; the
  deferral is contract-tested, not live-demonstrated.
- **Live joiner digest verification on a real device**: the digest-echo
  contract is enforced and unit-tested; the live joining-device exercise is
  gated on R1 hardware.
- **Shadow-window ramp catch-up** (roadmap P1.1): psionic-side companion
  (psionic#1125), out of scope for this monorepo change.
