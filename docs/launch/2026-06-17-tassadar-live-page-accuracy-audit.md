# Tassadar live page accuracy audit

Date: 2026-06-17
Status: implementation audit, not product copy
Scope: future `/tassadar`, current `/run`, `/components/training`, live
Tassadar public projections, and `@openagentsinc/three-effect` training
primitives.

## Implementation updates

- 2026-06-17: Issue #5186 implemented the canonical `/tassadar` route as a
  public alias over the live Tassadar run view. The browser scene now renders a
  live snapshot strip from the fetched public summary: run ref, run state,
  `generatedAt`, `projection_staleness.v1` composition, browser fetched time,
  and a manual refresh control. `/run` remains as the existing alias.
- 2026-06-17: Issue #5187 extended the public Tassadar summary contract with
  typed `settlementRows` and `realGradient.rejectedReplayPairs`. Settlement rows
  carry receipt refs, contributor refs, challenge refs, amount sats,
  `movementMode`, `realBitcoinMoved`, state, and public Nexus/Pylon proof URLs,
  so later visual layers can stop inferring payout proof from aggregates.
- 2026-06-17: Issue #5188 replaced automatic proof-tab opening with an in-page
  proof drawer and made receipt routing namespace-aware. Nexus/Pylon receipts now
  resolve through `/api/public/nexus-pylon/receipts/{receiptRef}` while forum
  receipts retain `/api/forum/receipts/{receiptRef}`; settlement rows feed the
  drawer's movement-mode and simulation caveats.
- 2026-06-17: Issue #5189 removed renderer fallback visuals from the web
  adapter. The `/tassadar` scene now passes an empty `lossCurve` unless the
  public summary provides real loss points, and contributor-ring marks are
  derived from public leaderboard rows rather than the dependency's synthetic
  placeholder contributors.

## Short answer

We have the right base, but not the full accurate page yet.

The accurate path is to make `/tassadar` the public/friendly route over the
existing live-run architecture:

- `GET /api/public/tassadar-run-summary`
- `apps/openagents.com/apps/web/src/scene/tassadarRunSnapshot.ts`
- `apps/openagents.com/apps/web/src/scene/tassadarRunElement.ts`
- `@openagentsinc/three-effect` `oa-training-run`

Do not fork a second data adapter. Do not use the new
`/components/training` grammar items as live state until they are wired to
public refs. Today those gallery primitives are useful visual grammar, but only
`oa-training-run` is actually data-bound to the live Worker projection.

The missing work is mostly about truthfulness, not raw visuals:

1. `/tassadar` is not a route yet; live `https://openagents.com/tassadar`
   redirects to `/`.
2. The current `/run` element fetches once on mount. A page called live needs an
   explicit refresh contract or visible "snapshot as of" state.
3. Settlement is not visually accurate enough. The live run summary reports
   `providerConfirmedSettledPayoutSats: 5`, but the linked receipt is
   `movementMode: simulation` and `realBitcoinMoved: false`. The page must say
   "settlement record" or "simulation-backed settlement proof", not real sats
   paid.
4. The run-level settlement join and the per-leaderboard rows disagree: the run
   metric sees 5 settlement-recorded sats, while every current leaderboard row
   reports `settledPayoutSats: 0`. Row-driven receipt bursts will therefore
   miss the settlement event.
5. Receipt proof links are not receipt-kind aware. The current scene helper sends
   `receipt.*` refs to `/api/forum/receipts/{ref}`, but the Tassadar settlement
   receipt resolves at `/api/public/nexus-pylon/receipts/{ref}` and its HTML
   receipt page.
6. The shared Three scene falls back to a default loss curve when no loss data
   exists. That is wrong for an executor-trace run unless it is explicitly
   labeled as a template or removed.
7. Rejected work is only a count in the current scene. The live summary has 3
   rejected exact-replay challenges, but the scene only draws verified replay
   pairs.
8. The product-promise state is not currently part of the `/run` page even
   though it is essential context: the scoped Monday launch promise is green,
   but the registry explicitly says the settlement receipt is simulation-backed
   and real paid-settlement copy still requires `realBitcoinMoved:true`.

## Sources reviewed

Launch and training docs:

- `docs/launch/JUNE15_LAUNCH_PLAN.md`
- `docs/launch/JUNE16_ROADMAP.md`
- `docs/launch/JUNE17_ROADMAP.md`
- `docs/launch/2026-06-17-tassadar-training-run-visual-language.md`
- `docs/training/2026-06-14-autopilot-desktop-training-ui-audit.md`
- `apps/openagents.com/docs/2026-06-11-cs336-a1-live-homework-paid-closeout-evidence.md`
- `apps/openagents.com/docs/2026-06-11-tassadar-trace-factory-contract-freeze-evidence.md`

Web and Worker implementation:

- `apps/openagents.com/apps/web/src/page/run.ts`
- `apps/openagents.com/apps/web/src/route.ts`
- `apps/openagents.com/apps/web/src/view.ts`
- `apps/openagents.com/apps/web/src/scene/tassadarRunElement.ts`
- `apps/openagents.com/apps/web/src/scene/tassadarRunSnapshot.ts`
- `apps/openagents.com/apps/web/src/scene/animations/trainingGrammar.ts`
- `apps/openagents.com/workers/api/src/public-tassadar-run-summary-routes.ts`
- `apps/openagents.com/workers/api/src/training-run-window-authority.ts`
- `apps/openagents.com/workers/api/src/public-pylon-stats.ts`
- pinned `@openagentsinc/three-effect`
  `github:OpenAgentsInc/three-effect#f1794af1165dbfdef2584372171b9fdd52ba46a9`
  `packages/core/src/trainingRun.ts`

Live checks while writing:

- `https://openagents.com/tassadar` returned `302` to `/`.
- `https://openagents.com/run` returned `200`.
- `https://openagents.com/api/public/tassadar-run-summary` returned the live
  run summary at `generatedAt: 2026-06-17T16:20:10Z`.
- `https://openagents.com/api/public/training/runs/run.tassadar.executor.20260615`
  returned the canonical public run envelope at the same time.
- `https://openagents.com/api/public/product-promises` returned the current
  promise registry.
- `https://openagents.com/api/public/pylon-stats` returned live fleet counters.
- `https://openagents.com/api/public/nexus-pylon/receipts/receipt.nexus.tassadar_run_settlement.idem.tassadar.settlement.59ba1f30.orrery.v2`
  returned the public settlement receipt.

## Live snapshot

As of the live checks above, the run-specific projection says:

- `runRef`: `run.tassadar.executor.20260615`
- `runState`: `active`
- `staleness`: `projection_staleness.v1`, `live_at_read`,
  `maxStalenessSeconds: 0`
- active windows: 1
- planned windows: 0
- sealed windows: 0
- reconciled windows: 0
- assigned contributors: 6
- distinct contributor devices observed for the real-gradient generic gate: 6
  of required 2
- verified work: 3
- rejected work: 3
- accepted exact-replay corpus traces: 3
- receipt refs: 32
- provider-confirmed settlement-record sats linked to the run: 5
- qualified contributors in the run summary: 1

The settlement receipt behind the 5 sats says:

- `receiptKind`: `settlement_recorded`
- `movementMode`: `simulation`
- `realBitcoinMoved`: `false`
- `publicProjection.amountSats`: 5
- `publicProjection.contributorRef`: `pylon.448ba824b5fc879f3a59`
- `publicProjection.trainingRunRef`: `run.tassadar.executor.20260615`
- `publicProjection.verificationChallengeRef`:
  `training.verification.challenge.59ba1f30-c2f0-40b0-b3ec-b9c5e1fb5316`

The promise registry says:

- `training.monday_decentralized_training_launch.v1`: green, scoped launch
  path proven, but the Orrery receipt is simulation-backed and does not prove
  real sats moved.
- `pylon.install_without_wallet_knowledge.v1`: green, self-serve
  install-to-verified-contribution path proven, with the same simulation
  settlement caveat.
- `models.tassadar_percepta_executor.v1`: red.
- `training.public_gradient_windows.v1`: planned.
- `training.public_distributed_training_run.v1`: red.
- `pylon.first_real_model_training_run.v1`: yellow.

The fleet-wide pylon stats snapshot says:

- pylons online now: 9
- pylons registered total: 68
- wallet ready now: 2
- assignment ready now: 2
- training assigned contributors: 6
- training accepted contributors: 0
- training model-progress contributors: 6
- public real sats settled in 24h: 338844
- public real sats settled total: 448344

That last block is useful context, but it is not the canonical truth source for
the run page. It currently reports `trainingAcceptedContributors: 0` while the
run-specific summary reports `qualifiedContributorCount: 1`, because the
run-specific endpoint resolves the settlement ledger for its receipt join. The
future `/tassadar` page should treat the run-specific public endpoint as
canonical and use pylon stats only as surrounding fleet context.

## Accuracy contract for `/tassadar`

A totally accurate page needs to satisfy these rules:

1. Every number must come from a public projection carrying `generatedAt` and
   the shared staleness contract.
2. The page must show the projection timestamp and whether the browser has
   refreshed it recently.
3. Every visible node, beam, burst, corpus tile, and proof drawer row must be
   backed by a public-safe ref, or must be visibly absent/unknown.
4. "Assigned", "verified", "settlement recorded", "real bitcoin moved",
   "qualified", "accepted trace", "corpus growth", and "trained model" must
   remain separate.
5. Simulation-backed settlement must never render as real paid Bitcoin.
6. Pending, queued, rejected, or stale work must not visually count as accepted
   work.
7. The page must tolerate zero and idle states without substituting demo curves,
   demo nodes, or optimistic copy.
8. Product-promise state must be displayed or linked where it affects the
   interpretation of the page.
9. Proof links must resolve to the correct public route for the ref kind.
10. Missing data must be an explicit state, not a default visual.

## Data coverage by primitive

### Run field

Current status: usable as the base. `oa-training-run` can render lifecycle
nodes, run state, windows, devices, verified/rejected work counts, receipt count,
settlement count, verified replay entities, beams, and payout bursts.

Needed for total accuracy:

- Add `/tassadar` route and title using the same view.
- Show `generatedAt`, `staleness.composition`, and last refresh age in the page
  chrome.
- Add polling or an explicit manual refresh. Today the custom element fetches
  once on connect; the animation breathes, but the data does not.
- Prevent default loss-curve rendering when no real loss curve exists.
- Feed product-promise signals into the scene or adjacent proof panel.

### Contributor node

Current status: partially data-bound through `realGradient.leaderboardRows` and
generic `entities`.

Accuracy problems:

- `assignedContributorCount` is distinct lease `pylonRef`, not online-now
  presence and not proof of accepted work.
- `leaderboardRows` currently attach every verified challenge ref to every
  observed pylon row. That is acceptable as a coarse run-row summary, but not as
  per-contributor proof.
- `settledPayoutSats` is zero on all leaderboard rows despite the run-level
  settlement-record metric being 5 sats.

Needed:

- Add per-contributor settlement and verification joins, or keep contributor
  nodes in assigned/observed states while verified replay pairs carry the proof.
- Add typed statuses at least for `assigned`, `verified`, `settlement_recorded`,
  `real_settled`, `simulation_settled`, `rejected`, and `blocked`.

### Trace strand

Current status: not data-bound. The `/components/training` trace strand is a
prototype scene.

The live summary has verified and rejected challenge counts, but it does not
project a submitted/queued trace list suitable for strand rendering.

Needed:

- Public-safe submitted trace refs if we want claimed/submitted strands.
- Rejected challenge refs with mismatch reason refs if we want rejected strands.
- Keep the page from inventing trace strands from counts alone.

### Replay pair

Current status: data exists for verified pairs. The live summary projects three
`verifiedReplayPairs`, each with worker ref, validator ref, challenge ref,
verdict ref, and source refs.

Needed:

- Add rejected replay pairs if the page should visualize failed work.
- Add device-distinctness labels or caveat refs in the proof drawer.
- Do not show a replay beam for queued or unverified challenges.

### Verification gate

Current status: partial. Counts and verified pairs exist. The scene can mark the
proof node as verified/sealed/blocked, but it does not expose the full state
machine.

Needed:

- Drawer/panel rows for `trace submitted -> replay challenge -> digest
  match/mismatch -> verdict`.
- A distinction between exact-replay verification for Tassadar and the
  CS336/Psion real-gradient blocker set.

### Receipt burst

Current status: unsafe for exact truth. `TrainingRunBurstDefinition` is only
`{ atId }`. It carries no receipt ref, amount, movement mode, or
`realBitcoinMoved` flag.

Needed:

- Extend the Worker public summary with settlement rows, for example:
  `receiptRef`, `contributorRef`, `verificationChallengeRef`, `amountSats`,
  `receiptKind`, `movementMode`, `realBitcoinMoved`, `state`, `apiUrl`, and
  `receiptPageUrl`.
- Extend the web adapter and/or `three-effect` burst type so burst color and
  proof text can distinguish:
  - `settlement_recorded_simulation`
  - `settlement_recorded_real_bitcoin`
  - `pending_payout`
  - `failed_or_expired`
- Until then, render the settlement as a proof drawer/list item rather than a
  celebratory payout burst.

### Corpus accretion

Current status: data exists as `corpus.acceptedTraceCount`, `traceRefs`, and
`verdictRefs`.

Needed:

- Bind corpus tiles/ring marks to accepted exact-replay `traceRefs` only.
- Label this as verified trace corpus growth, not model capability.
- Keep dataset-curation/distillation receipts separate. The current corpus count
  is evidence toward a dataset, not itself a trained model.

### Quarantine window

Current status: not relevant to the executor-trace mainline yet. It belongs to
future public-gradient windows and Psion/Tassadar student updates.

Needed:

- Do not render quarantine as a mainline Tassadar executor-trace primitive
  unless the page is also displaying `training.public_gradient_windows.v1`.
- If shown, label it as planned/future or separate from the active executor run.

### Energy/outcome meter

Current status: not run-specific enough for `/tassadar`.

Needed:

- Only show AO/kWh or energy/outcome if the run summary or an accepted-outcomes
  endpoint supplies run-linked measurement refs.
- Do not reuse fleet-wide or modeled metrics as run outcome truth.

### Proof drawer

Current status: click-through exists but is not accurate enough.

Problems:

- It opens a new tab immediately rather than giving a drawer first.
- It maps any `receipt.*` ref to `/api/forum/receipts/{ref}`; the live Tassadar
  settlement receipt 404s there and resolves through
  `/api/public/nexus-pylon/receipts/{ref}` plus its HTML receipt page.
- It cannot explain simulation vs real settlement.

Needed:

- A real drawer with ref, kind, state, caveats, source refs, and route.
- Receipt-kind-aware route resolution.
- No automatic tab open on selection; make "Open proof" explicit.

## Page composition recommendation

Use this structure:

1. Full-bleed `oa-training-run` scene as the first viewport.
2. Top strip:
   - `Tassadar run`
   - `run.tassadar.executor.20260615`
   - run state
   - generated-at age
   - staleness contract
   - refresh state
3. Left or bottom compact counters:
   - assigned contributors
   - verified exact-replay work
   - rejected exact-replay work
   - accepted trace corpus count
   - settlement-record sats
   - real-bitcoin-paid sats, if and only if a `realBitcoinMoved:true` receipt is
     linked
4. Proof drawer:
   - selected run/window/contributor/replay/receipt/corpus ref
   - provenance label
   - public endpoint
   - caveats and blocker refs
5. Promise/copy gate strip:
   - Monday launch: green, simulation caveat
   - install-to-verified-contribution: green, simulation caveat
   - trained Tassadar model: red
   - public gradient windows: planned
6. Secondary fleet context:
   - pylons online
   - wallet/assignment readiness
   - training assigned contributors
   - caveat that fleet stats are context and the run endpoint is canonical

Avoid a marketing landing page. `/tassadar` should open directly on the live
run instrument.

## Implementation checklist

### Route and shell

- Add `TassadarRoute` or map literal `tassadar` to the existing `RunRoute`.
- Add route tests for `/tassadar`.
- Give the document title `Tassadar run - OpenAgents` or reuse the existing
  `Live Tassadar run - OpenAgents`.
- Keep `/run` as an alias unless there is a deliberate migration.

### Public projection

- Add settlement rows to `public-tassadar-run-summary-routes.ts` or the shared
  `publicTrainingRunSummary` output.
- Fix per-leaderboard settlement attribution or stop using row
  `settledPayoutSats` for visual bursts.
- Add `realBitcoinMoved` and `movementMode` to public-safe settlement
  projections consumed by the page.
- Consider adding rejected replay pair projections with public-safe mismatch
  reason refs.
- Keep all new fields under the shared `projection_staleness.v1` contract.

### Web adapter

- Remove the default loss curve when no loss data exists.
- Map settlement rows into typed visual states.
- Map product-promise records into scene signals or an adjacent panel.
- Route receipt links by receipt namespace:
  - `receipt.nexus...` and `receipt.nexus_pylon...` ->
    `/api/public/nexus-pylon/receipts/{ref}` or the HTML receipt page.
  - Forum receipts -> `/api/forum/receipts/{ref}`.
  - Training challenge refs ->
    `/api/public/training/runs/{runRef}?focusRef={ref}` or a better focused
    proof route when implemented.
- Keep summary decoding defensive, but do not silently turn malformed important
  fields into success-looking zeroes. Important parse failure should become an
  error/unknown panel.

### Three primitives

- Keep `oa-training-run` as the main data-bound component.
- Promote the gallery-only grammar items only when each accepts data:
  - contributor node from contributor/ref status
  - trace strand from submitted/queued/rejected trace refs
  - replay pair from verified/rejected challenge refs
  - receipt burst from settlement row metadata
  - corpus accretion from accepted trace refs
  - proof drawer from selected public ref
- Extend `TrainingRunBurstDefinition` if settlement bursts remain in the scene.
  `{ atId }` is not enough for accurate payout semantics.

### Tests and smokes

- Worker tests:
  - public Tassadar summary includes `staleness`, settlement rows, and no private
    material
  - simulation settlement does not set real-paid fields
  - per-contributor settlement attribution matches receipt `contributorRef`
  - rejected replay projections include only public-safe mismatch refs
- Web unit tests:
  - `/tassadar` routes correctly
  - no fallback loss curve without loss evidence
  - receipt link resolver chooses the Nexus/Pylon route for Nexus receipts
  - proof drawer shows simulation caveat for the current receipt
  - row-zero/run-nonzero settlement state does not produce a misleading burst
- Browser smoke:
  - `/tassadar` returns 200
  - exact hashed JS asset returns 200
  - the WebGL canvas is nonblank
  - the page displays generated-at/staleness text
  - selecting a settlement/proof node opens a drawer with a public route that
    returns 200 by GET
- Copy gate:
  - no affirmative "real sats paid", "trained Tassadar", "largest", or
    "public gradients accepted" copy unless matching product-promise gates are
    green with evidence.

## Open questions

- Should `/tassadar` become canonical and `/run` remain a short alias, or should
  `/run` redirect to `/tassadar` after launch?
- What exact label should the current simulation-backed 5-sat receipt use in
  product UI: "settlement record", "simulation settlement", or
  "settlement path proof"?
- Should rejected replay pairs be visible in the main scene, or only in the
  proof drawer?
- What is the refresh interval for a live public run page? A 15 to 30 second
  poll is likely enough for public viewing; operator views can refresh faster.
- Should pylon fleet stats appear on `/tassadar`, or should the page stay
  strictly run-scoped and link to `/` or `/stats` for fleet context?

## Recommendation

Build `/tassadar` as a route-level alias or successor to `/run`, then close the
truth gaps before adding more visual grammar.

The current base scene is valuable, and the live run endpoint is strong. The
page becomes trustworthy when every visual success mark can answer three
questions:

1. Which public ref caused this mark?
2. How fresh is the projection?
3. Does the mark mean verified work, settlement recorded, real Bitcoin moved,
   corpus accepted, or only a planned/future gate?

Until those answers are built into the page, use the new `/components/training`
items as design studies, not as live proof. The main mistake to avoid is making
the run look more successful than the public receipts prove.
