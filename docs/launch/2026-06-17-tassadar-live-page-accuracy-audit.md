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
- 2026-06-17: Issue #5190 bound the remaining training grammar marks to public
  refs in the Tassadar adapter. Contributor entities now distinguish assigned,
  verified, simulation-settled, real-settled, pending, and failed settlement
  states; rejected replay pairs, settlement receipts, and accepted trace corpus
  refs become selectable proof entities; payout bursts render only for
  `realBitcoinMoved:true` settlement rows.
- 2026-06-17: Issue #5191 added a live product-promise copy gate to the
  `/tassadar` scene. It reads `/api/public/product-promises`, displays the
  launch/install/model/gradient/first-real-run states, links the registry, and
  keeps the simulation-settlement caveat visible so the page does not claim real
  sats paid, trained Tassadar, or accepted public gradients ahead of evidence.
- 2026-06-17: Issue #5192 added `smoke:tassadar:live-page` for deployment
  verification. The smoke checks `/tassadar`, hashed app script assets,
  `/api/public/tassadar-run-summary` staleness and typed proof projections,
  `/api/public/product-promises` gate refs, and the first settlement-row proof
  URL when present. The script is dependency-free; pixel-level WebGL nonblank
  validation still needs a browser-capable runner if we want it fully automated.
- 2026-06-17: Issue #5193 reconciled run-specific metrics with fleet pylon
  stats in the `/tassadar` page. The visible source split says the Tassadar run
  endpoint is canonical for accepted-work and settlement numbers, while
  `/api/public/pylon-stats` supplies surrounding fleet context. The live smoke
  now also verifies the pylon stats fields used by that reconciliation block.
- 2026-06-17: Deploy verification caught the remaining server-side route gap.
  `/tassadar` is now in the Worker document-route allowlist, so direct browser
  hits serve the app shell instead of redirecting home; the smoke's staleness
  assertion now matches the public summary's `projection_staleness.v1` shape.
- 2026-06-17: A follow-up live check caught the client-side startup gap after
  the server allowlist fix: `TassadarRoute` parsed, but
  `routing/startup.ts` did not include `Tassadar` in the public startup
  resolver lists where `/run` was already allowed. The app could therefore
  serve the shell while the browser startup redirected or fell back before the
  `oa-tassadar-run` scene mounted. The fix adds `Tassadar` to logged-out,
  incomplete-onboarding, and complete-onboarding startup resolution, and adds
  regression coverage that `/tassadar` skips auth bootstrap, initializes
  without a session, and renders the `oa-tassadar-run` element through the
  top-level view.
- 2026-06-17: A truth-map follow-up clarified the moving-dot semantics in the
  Three scene. The white dots moving along the fixed graph edges are renderer
  flow pulses, not pylon/device/work nodes and not rows in the public summary.
  The live data-bound marks are the contributor orbit dots, entity-ring marks,
  verified replay beams, settlement receipt entity, accepted trace entities,
  promise registry marks, and aggregate stage-node labels/statuses described
  below.
- 2026-06-17: Owner review tightened the motion rule. Abstract flow pulses are
  no longer acceptable live-run language for `/tassadar`: every moving dot,
  pulse, flow, beam, burst, or counter roll must be bound to a public ref or a
  timestamped live state transition. Static aggregate graph structure is allowed;
  anonymous motion is not.
- 2026-06-17: The motion rule is now implemented in `three-effect` and the
  `/tassadar` adapter. `oa-training-run` has a `motionPolicy` with static base
  edges and static ambient motion by default; `/tassadar` opts into
  `evidence:"required"`, so beams and bursts render only when they carry public
  `sourceRefs`. Verified replay beams now cite challenge/worker/validator/verdict
  refs, and payout bursts cite settlement receipt/contributor/challenge refs and
  still require `realBitcoinMoved:true`.
- 2026-06-17: The `/tassadar` page shell is now headerless and full-bleed. The
  public nav/header was removed for this route, and status/proof panels sit over
  the 3D canvas with translucent black glass instead of enclosing the scene in
  an app-frame card.
- 2026-06-17: The stage-node glyph grammar was tightened after visual review.
  `registered`, `qualified`, `state synced`, `active`, `sync reentry`, `R1`, and
  `R2` are aggregate run-stage concepts, not individual pylons. They now render
  with compact gate markers on `/tassadar`; pylon/record orb glyphs are reserved
  for actual contributor/entity refs such as `P1` through `P6`, replay workers,
  validators, trace refs, and receipt refs.
- 2026-06-17: A later live-page review found that even compact aggregate gates
  were too spatially prominent and still read like unexplained nodes. `/tassadar`
  now keeps the main 3D field to the run node plus public-ref entities only:
  pylon refs, replay worker/validator refs, rejected replay refs, trace refs, and
  settlement receipt refs. Aggregate lifecycle counters such as `registered`,
  `qualified`, `state synced`, `active`, and `sync reentry` are not placed as
  scene nodes. Verified replay beams are also removed from the main view for
  now, because the page does not yet explain them well enough. The loss panel,
  status mini-chart, stale ring, contributor orbit, product-promise gate, and
  fleet-stats text are hidden from the primary canvas.
- 2026-06-17: The simplified entity field now assigns real refs to explicit
  lanes instead of relying on the generic entity ring: pylon refs on the left,
  verified replay refs along the top, rejected replay refs along the bottom,
  accepted trace refs on the right, and settlement receipt refs lower-left. This
  keeps labels from collapsing into clusters while preserving the "only real
  nodes" rule.
- 2026-06-17: Issue #5236 published the first self-hosted SpacetimeDB
  `openagents-world` module from the separate
  `apps/openagents-world-spacetimedb` app. The module currently exposes only the
  minimal public projection tables (`training_run`, `run_entity`, `world_edge`,
  `proof_ref`, `settlement_ref`, `world_event`, `projection_cursor`, and
  `bridge_health`) plus private `module_owner` and `service_identity` authority
  tables. This does not change `/tassadar` authority: the Worker/D1 public
  summary remains the source of truth until the bridge and browser subscription
  adapter are implemented and verified.
- 2026-06-17: Issue #5237 implemented and ran the operator bridge from
  `https://openagents.com/api/public/tassadar-run-summary` into
  `openagents-world`. The bridge lives in the separate
  `apps/openagents-world-spacetimedb` app, calls service-only reducers over IAP
  SSH, and projected canonical run `run.tassadar.executor.20260615` into one
  `training_run`, 16 de-duplicated `run_entity` rows, 16 `world_edge` rows, 58
  de-duplicated `proof_ref` rows, one `settlement_ref`, 17 `world_event` rows,
  one `projection_cursor`, and one `bridge_health` row. Replay left
  `world_event` at 17 rows. `/tassadar` still renders from the Worker/D1 public
  summary until the feature-flagged browser subscription adapter lands.
- 2026-06-17: Issue #5238 added the feature-flagged browser subscription
  adapter for `/tassadar`. The route now passes public
  `data-spacetime-world-url="https://spacetime.openagents.com"` and
  `data-spacetime-database="openagents-world"` attributes into
  `oa-tassadar-run`; the custom element still fetches
  `/api/public/tassadar-run-summary` first, then subscribes anonymously to the
  public `training_run`, `run_entity`, `world_edge`, `proof_ref`,
  `settlement_ref`, and `world_event` tables. Row callbacks are converted back
  into the same public-summary visualization shape, so SpacetimeDB can update
  only public-ref-backed entities and timestamped projection transitions. If the
  WebSocket is disabled or unreachable, the Worker summary scene remains live.
- 2026-06-17: Issue #5239 hardened the self-hosted SpacetimeDB operations
  surface before production gameplay state. `/stdb` now lives on dedicated
  persistent disk `spacetimedb-world-data-1`, with rollback snapshots
  `spacetimedb-world-1-boot-20260617-pre-world-hardening` and
  `spacetimedb-world-data-1-20260617-post-migration`. Cloud Monitoring now has
  the `SpacetimeDB world identity 405` uptime check for
  `https://spacetime.openagents.com/v1/identity`, enabled alert policies for
  identity uptime failure, Nginx 5xx spikes, and `spacetimedb.service` restart
  loops, and Ops Agent-backed Nginx/syslog ingestion. The project still needs
  an external notification channel attached before alerts should page someone.
- 2026-06-17: Issue #5261 added the MVP shared-world interaction schema in the
  separate `apps/openagents-world-spacetimedb` app. The module now exposes
  public interaction tables for `pylon_station`, `agent_avatar`,
  `avatar_position`, `pylon_attention`, `local_chat_message`, `chat_bubble`,
  `local_emote`, and `agent_intent`, plus generated TypeScript bindings for the
  web adapter. Browser reducers are limited to joining/leaving, bounded and
  throttled position updates, pylon focus, local or pylon-targeted plain-text
  messages, emotes, and ephemeral intent. Service reducers own station/avatar
  projection and expiry. This does not change `/tassadar` truth authority:
  Worker/D1 remains canonical for run, pylon, proof, receipt, settlement, and
  product-claim state.
- 2026-06-17: Issue #5262 published the interaction schema to the self-hosted
  `openagents-world` module and extended the Tassadar bridge to seed inhabited
  world rows from public leaderboard pylon refs. The live dry-run planned 194
  reducer calls, including 6 `upsert_pylon_station_from_projection` calls and
  6 `ensure_pylon_agent_avatar` calls. Applying and replaying the bridge left
  projection counts stable (`world_event` stayed at 17) and produced 6
  `pylon_station`, 6 `agent_avatar`, and 6 `avatar_position` rows. The station
  coordinates mirror the existing left-lane P1-P6 pylon layout, and avatar refs
  are deterministic `avatar.pylon_agent.{pylonRef}` values derived only from
  public pylon refs.
- 2026-06-17: Issue #5263 rendered the public interaction rows on `/tassadar`.
  The browser adapter now subscribes to `pylon_station`, `agent_avatar`, and
  `avatar_position` in addition to the proof projection tables. The scene maps
  each public pylon station to a compact `station.{pylonRef}` entity and each
  seeded pylon-agent avatar to its row-backed world position. Station/avatar
  selections resolve back through the existing public pylon proof/receipt
  inspector instead of adding SpacetimeDB-only proof URLs. The page still starts
  from `/api/public/tassadar-run-summary` and falls back to that Worker/D1
  summary when SpacetimeDB is disabled or unreachable. Deploy version
  `337b4161-02d0-49fb-9755-a218b736cf3f` served hashed bundle
  `/assets/index-D3gMYS5a.js`; the live `/tassadar` smoke passed after deploy.
- 2026-06-17: Issue #5264 added the first live movement and pylon visitor
  attention loop. When SpacetimeDB connects, `/tassadar` joins the run region as
  a public guest avatar, tracks the WASD/mouselook controls into a bounded local
  avatar position, calls `set_avatar_position` no faster than every 250 ms, and
  sends a 5 second idle keepalive. The browser emits `focus_pylon` no faster
  than every 1 second when near, looking at, or inspecting a public pylon
  station. The scene subscribes to `pylon_attention`, renders guest/avatar rows
  from the run region, and marks stations with compact `+N` attention labels.
  Server reducers still clamp coordinates, reject impossible jumps, and keep the
  20 second stale-avatar TTL. Deploy version
  `5ca9e407-38ea-4887-b2b8-17345a9049b4` served hashed bundle
  `/assets/index-B9O9w_54.js`; the live `/tassadar` smoke passed after deploy.
- 2026-06-17: Issue #5265 added the first local chatter loop for the inhabited
  `/tassadar` world. The SpacetimeDB module now rate-limits local messages to
  one message per avatar per second, stores only visible plain-text message rows
  capped at 280 characters, and emits short-lived `chat_bubble` anchors for the
  speaker or the targeted public pylon station. The browser subscribes to
  `local_chat_message` and `chat_bubble`, sanitizes chat input before calling
  local or pylon-targeted reducers, renders a nearby transcript, and places
  bubbles over both the speaker and pylon station for pylon-channel messages.
  The implementation deliberately adds no fake fixture chatter and does not
  store private prompts, logs, wallet material, provider payloads, or
  SpacetimeDB-only proof authority in chat rows. The live self-hosted
  `openagents-world` module publish succeeded, and deploy version
  `c4621e06-4d60-4a60-8133-41ee3c7ff4d5` served hashed bundle
  `/assets/index-DQ9FGJ43.js`; live checks for `/`, `/tassadar`, the hashed
  asset, and `smoke:tassadar:live-page` passed after deploy.

## Current status update

As of 2026-06-21, the old web `/tassadar` live scene is retired so it cannot be
confused with the Autopilot Desktop Verse world. The plain `/tassadar` route now
serves a compact retired-scene notice with links to the public summary API and
the proof replay route. The live in-world Tassadar/Pylon surface belongs in the
Verse implementation, while `/tassadar/replay/{slug}` remains the web proof
replay surface.

## Historical short answer

We now have the right live-page base and a stricter first-read composition for
motion truth.

The old web-scene path was:

- `GET /api/public/tassadar-run-summary`
- `apps/openagents.com/apps/web/src/scene/tassadarRunSnapshot.ts`
- `@openagentsinc/three-effect` `oa-training-run`

That route-level live scene is no longer active. Do not rebuild a second web
world adapter from this historical note; use the Verse surface for in-world
Tassadar/Pylon work and keep web replay work on
`apps/openagents.com/apps/web/src/scene/tassadarProofReplayElement.ts`.

Do not fork a second data adapter. Do not use the new `/components/training`
grammar items as live state until they are wired to public refs. Today those
gallery primitives are useful visual grammar, but only `oa-training-run` is
actually data-bound to the live Worker projection.

The current main view should answer only one question on first read: "what real
public refs exist for this run right now?" It should not spatialize lifecycle
legend labels, show a loss chart before there is product-ready loss evidence, or
display promise/fleet copy in the primary canvas.

The remaining work is no longer to remove anonymous renderer motion or compact
aggregate gates from `/tassadar`; that is done. The next work is to deepen event
specificity and inspection:

1. Any replacement motion must come from typed motion events derived from public
   refs: replay challenge refs, trace refs, receipt refs, pylon refs, or
   timestamped projection transitions.
2. Counts can label or color aggregate stage nodes, but counts alone must not
   create moving dots, fake strands, fake traffic, fake payout effects, or
   spatial nodes in the main field.
3. Simulation-backed settlement can render as a selectable receipt/proof state,
   but it must not animate like real Bitcoin movement.
4. `/components/training` and `/animations` should keep unbound studies static
   or visibly fixture-only until they accept real refs.

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

Live checks while writing and later updating:

- Initial `https://openagents.com/tassadar` checks returned `302` to `/` before
  the route/startup fixes above.
- Current evidence-bound motion guidance check on 2026-06-17 returned `200` for
  `https://openagents.com/tassadar`.
- `https://openagents.com/run` returned `200`.
- `https://openagents.com/api/public/tassadar-run-summary` returned the live
  run summary at `generatedAt: 2026-06-17T16:20:10Z`.
- An earlier truth-map check of the same endpoint returned
  `generatedAt: 2026-06-17T17:49:53.847Z` and resolved the browser scene to 6
  contributor dots, 22 ref-backed entities, 3 verified replay beams, 0 payout
  bursts, and 0 loss-curve points.
- The evidence-bound motion guidance check returned
  `generatedAt: 2026-06-17T18:17:09.548Z` with 6 leaderboard rows, 3 verified
  replay pairs, 3 rejected replay pairs, 1 simulation settlement row, 0
  loss-curve points, and 3 accepted trace refs.
- `https://openagents.com/api/public/training/runs/run.tassadar.executor.20260615`
  returned the canonical public run envelope at the same time.
- `https://openagents.com/api/public/product-promises` returned the current
  promise registry.
- `https://openagents.com/api/public/pylon-stats` returned live fleet counters.
- `https://openagents.com/api/public/nexus-pylon/receipts/receipt.nexus.tassadar_run_settlement.idem.tassadar.settlement.59ba1f30.orrery.v2`
  returned the public settlement receipt and returned `200` again in the
  post-issue #5263 live smoke.
- The post-issue #5263 deploy check returned `200` for
  `https://openagents.com/`, `https://openagents.com/tassadar`, and
  `https://openagents.com/assets/index-D3gMYS5a.js`.
- The post-issue #5264 deploy check returned `200` for
  `https://openagents.com/`, `https://openagents.com/tassadar`, and
  `https://openagents.com/assets/index-B9O9w_54.js`; the live smoke also
  returned `200` for `/api/public/tassadar-run-summary`,
  `/api/public/pylon-stats`, `/api/public/product-promises`, and the first
  settlement proof route.
- The current simplified adapter resolves the same live payload to one run node,
  public-ref entities, zero contributor-orbit dots, zero verified replay beams,
  zero payout bursts, zero loss-curve points, and hidden optional scene chrome.

## Live snapshot

As of the live checks above, and rechecked at
`generatedAt: 2026-06-17T18:17:09.548Z`, the run-specific projection says:

- `runRef`: `run.tassadar.executor.20260615`
- `runState`: `active`
- `staleness`: `projection_staleness.v1`, `live_at_read`,
  `maxStalenessSeconds: 0`
- active windows: 1
- planned windows: 0
- sealed windows: 0
- reconciled windows: 0
- assigned contributors: 6
- contributor refs:
  - `pylon.448ba824b5fc879f3a59`
  - `pylon.5526de0746260942e85f`
  - `pylon.5651e69649c63004aa0b`
  - `pylon.7bb0d5628ca4b6e9c731`
  - `pylon.81f0facfe7971870f685`
  - `pylon.92141e78e39df40cc828`
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

## Evidence-bound motion rule

The moving white dots on the fixed graph edges are not real nodes. They are
`@openagentsinc/three-effect` flow pulses created for every edge between the
base training-stage nodes. They do not correspond to a pylon, a device, a lease,
a trace, a verification challenge, a receipt, a payout, or a training datum.

That is now considered live-page design debt, not an allowed exception.
`/tassadar` should not show data moving back and forth unless each moving mark is
bound to a real public ref or a timestamped live state transition. Fixed graph
edges may remain as static structure; their anonymous motion should be disabled
or replaced with typed, evidence-bound motion events.

The base stage labels are also not individual records. They are aggregate
grammar concepts whose label, status, and detail can be derived from the public
summary metrics. Examples:

- `registered`: `6 pylons seen`
- `qualified`: `6/2 device gate`
- `sync_reentry`: `3 blockers`
- `run`: `run.tassadar.executor.20260615`
- `training_window`: `0 plan / 1 act / 0 seal`
- `receipt`: `32 receipts`
- `settlement`: `5 sats`

Those aggregate concepts are allowed only as summarized counters or legend rows.
They are not allowed as spatial nodes in the `/tassadar` main field. They must
not create anonymous moving pulses and must not be read as "there are N hidden
real nodes behind each moving pulse".

They also must not use the same pylon/record orb glyph on live pages. A label
like `registered / 6 pylons seen` means "the registration stage has a public
count of six pylon refs observed"; it is not itself a pylon and not one of the
six pylon records. The six pylon records are the labeled `P1` through `P6`
entity marks derived from public `pylonRef` fields.

The data-bound marks in the current `/tassadar` scene are:

| Visual mark                                       | Source                                                         | Current truth                                                                                              |
| ------------------------------------------------- | -------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| Run node                                          | `runRef`, `runLabel`, `runState`                               | One public run anchor, not an aggregate lifecycle board.                                                   |
| Entity-ring pylon marks                           | `realGradient.leaderboardRows` plus top-level `settlementRows` | 6 real public pylon refs; `P1` is `simulation_settled`, the rest are `verified`.                           |
| Verified replay worker/validator entities         | `realGradient.verifiedReplayPairs[]`                           | 3 real verified worker-to-validator replay pairs, shown as selectable entities rather than animated beams. |
| Rejected replay entities                          | `realGradient.rejectedReplayPairs[]`                           | 3 rejected replay pairs, rendered as rejected worker/validator entities rather than success beams.         |
| Settlement receipt entity labeled `5s`            | top-level `settlementRows[]`                                   | 1 real public receipt ref, `movementMode: simulation`, `realBitcoinMoved: false`.                          |
| Payout burst particles                            | top-level `settlementRows[]` with `realBitcoinMoved: true`     | 0 currently. The current simulation settlement must not render a real-Bitcoin burst.                       |
| Accepted trace entities labeled `T1` through `T3` | top-level `corpus.traceRefs[]`                                 | 3 public verified challenge refs counted as accepted trace corpus entries.                                 |
| Loss curve points                                 | `realGradient.lossCurve[]`                                     | 0 currently. No default/demo loss curve is passed by the web adapter.                                      |
| Promise registry marks                            | `/api/public/product-promises`                                 | Real promise records, but not rendered in the primary scene chrome.                                        |

Current resolved visual layer from the live payload:

- `nodes`: 1 run node
- `contributors`: 0
- `entities`: 22
- `beams`: 0
- `bursts`: 0
- `lossCurve`: 0 points
- optional scene chrome: hidden contributor orbit, loss panel, stale ring, and
  status mini-chart

Truth rule: every animated mark must have a public reason to move. Every mark
that looks like a pylon, contributor, worker, validator, trace, receipt,
settlement, payout, beam, or proof must come from a public-safe ref. Every
pulse, flow, burst, animated counter roll, or liveness heartbeat must be bound
to one of those refs or to a timestamped projection transition. If there is no
ref or live transition, the fallback is static state.

The immediate roadmap is now:

1. Keep the `three-effect` `motionPolicy` static/evidence-required path green in
   tests. Base edge flow pulses are disabled for `/tassadar`; do not regress
   that back to graph-topology motion.
2. Introduce richer typed motion events only when the public summary can supply
   `motionId`, `motionKind`, `sourceRefs`, `generatedAt`, and stale/expiry
   semantics.
3. Extend the proof drawer to expose the `sourceRefs` behind selected animated
   marks.
4. Keep settlement bursts gated on settlement rows and `realBitcoinMoved:true`;
   simulation settlement can be selected and inspected, but it must not move
   like real Bitcoin.

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
3. Every visible data entity, beam, burst, corpus tile, settlement mark, and
   proof drawer row must be backed by a public-safe ref, or must be visibly
   absent/unknown. Fixed stage nodes may summarize public metrics, but they must
   read as aggregate grammar, not hidden record nodes. On `/tassadar`, aggregate
   stage nodes stay out of the main spatial field.
4. Every moving mark must be evidence-bound. Anonymous edge pulses, decorative
   flow fields, fixture-like particle motion, and count-derived fake motion are
   banned from live training pages.
5. "Assigned", "verified", "settlement recorded", "real bitcoin moved",
   "qualified", "accepted trace", "corpus growth", and "trained model" must
   remain separate.
6. Simulation-backed settlement must never render as real paid Bitcoin.
7. Pending, queued, rejected, or stale work must not visually count as accepted
   work.
8. The page must tolerate zero and idle states without substituting demo curves,
   demo nodes, or optimistic copy.
9. Product-promise state must be displayed or linked where it affects the
   interpretation of the page.
10. Proof links must resolve to the correct public route for the ref kind.
11. Missing data must be an explicit state, not a default visual.

## Data coverage by primitive

### Run field

Current status: usable as the base. `oa-training-run` can render lifecycle
nodes, run state, windows, devices, verified/rejected work counts, receipt count,
settlement count, verified replay entities, beams, and payout bursts.
The current `/tassadar` adapter intentionally disables most of that chrome and
keeps only the central run node plus public-ref entities in the main field.

Needed for total accuracy:

- Keep `/tassadar` public in the server document-route allowlist and browser
  startup resolver.
- Keep showing `generatedAt` and explicit manual refresh in the page chrome.
  Staleness detail can move to a secondary/supporting surface if the top strip
  becomes too dense.
- Consider polling if the page needs unattended wall-display behavior; today it
  is a live snapshot with manual refresh.
- Keep passing an empty `lossCurve` when no real loss evidence exists.
- Keep product-promise signals in docs, registry links, or a secondary support
  surface where they affect claim interpretation. Do not render a bottom
  `Promise gates` block in the main view.
- Disable fixed edge flow pulses for `/tassadar`, or bind every pulse to a typed
  motion event with public refs. Visual distinction is not enough; anonymous
  motion should not appear on the live run page.

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

Current status: partially data-bound. The `/components/training` trace strand is
still a prototype scene, but `/tassadar` renders accepted trace entities from
top-level `corpus.traceRefs`.

The live summary has verified and rejected challenge counts plus accepted trace
refs, but it does not project a submitted/queued trace list suitable for strand
rendering.

Needed:

- Public-safe submitted trace refs if we want claimed/submitted strands.
- Rejected challenge refs with mismatch reason refs if we want rejected strands.
- Keep the page from inventing trace strands from counts alone.

### Replay pair

Current status: data exists for verified and rejected pairs. The live summary
projects three `verifiedReplayPairs`, each with worker ref, validator ref,
challenge ref, verdict ref, and source refs. It also projects three
`rejectedReplayPairs`, each with public-safe failure/source refs.

Needed:

- Add device-distinctness labels or caveat refs in the proof drawer.
- Do not show a replay beam for queued or unverified challenges.

### Verification gate

Current status: partial. Counts and verified pairs exist. The scene can mark the
proof node as verified/sealed/blocked, but it does not expose the full state
machine.

Needed:

- Drawer/panel rows for trace submitted -> replay challenge -> digest
  match/mismatch -> verdict.
- A distinction between exact-replay verification for Tassadar and the
  CS336/Psion real-gradient blocker set.

### Receipt burst

Current status: safe in the `/tassadar` adapter, still structurally thin in
`three-effect`. `TrainingRunBurstDefinition` is only `{ atId }`, but the web
adapter now creates bursts only from top-level settlement rows with
`realBitcoinMoved:true`. The current live settlement row is
`movementMode: simulation` and `realBitcoinMoved:false`, so the resolved scene
has `bursts: []`.

Needed:

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

Current status: in-page proof selection exists. Node/entity selection renders an
aside with kind, state, ref, route, caveats, source refs, and an explicit
`Open proof` link. Nexus/Pylon receipt refs resolve through
`/api/public/nexus-pylon/receipts/{ref}`; forum receipts keep the forum receipt
route. The current settlement proof row includes the simulation caveat.

Problems:

- The proof drawer is still compact and selection-driven; it is not yet a full
  searchable proof table.
- Some aggregate stage nodes can be selected without a public proof ref because
  they summarize multiple public metrics rather than one row.

Needed:

- Keep aggregate stage-node selections honest by saying no single public proof
  ref is linked.
- Consider adding a persistent proof table for all public refs used by the
  current scene.

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
3. Supporting counters, if used, should be compact and clearly secondary:
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
5. Promise/copy gate access, outside the main scene:
   - Monday launch: green, simulation caveat
   - install-to-verified-contribution: green, simulation caveat
   - trained Tassadar model: red
   - public gradient windows: planned
6. Secondary fleet context, outside the main scene:
   - pylons online
   - wallet/assignment readiness
   - training assigned contributors
   - caveat that fleet stats are context and the run endpoint is canonical

Avoid a marketing landing page. `/tassadar` should open directly on the live
run instrument.

Current first-read rule: the main canvas shows nodes and selectable public-ref
entities. It does not show a loss curve, aggregate lifecycle board, promise-gate
panel, fleet-stats panel, or unexplained replay traffic.

## Implementation checklist

### Route and shell

- Keep `TassadarRoute` mapped to the existing live run view.
- Keep route/startup tests for `/tassadar` so logged-out users get the public
  scene instead of auth bootstrap or a home redirect.
- Keep the document title `Tassadar run - OpenAgents` or reuse the existing
  `Live Tassadar run - OpenAgents`.
- Keep `/run` as an alias unless there is a deliberate migration.

### Public projection

- Keep top-level `settlementRows` in the shared public summary output with
  `receiptRef`, `contributorRef`, `verificationChallengeRef`, `amountSats`,
  `receiptKind`, `movementMode`, `realBitcoinMoved`, `state`, `apiUrl`, and
  `receiptPageUrl`.
- Do not use leaderboard-row `settledPayoutSats` for visual bursts while row
  values remain zero and the run-level settlement receipt is represented by
  top-level `settlementRows`.
- Keep rejected replay pair projections public-safe; failure codes and source
  refs are acceptable, private trace payloads are not.
- Keep all fields under the shared `projection_staleness.v1` contract.

### Web adapter

- Keep passing `lossCurve: []` when no loss data exists.
- Keep mapping settlement rows into typed visual states.
- Keep product-promise records available through registry/API surfaces; do not
  add the bottom copy-gate panel back to the main `/tassadar` canvas without a
  deliberate secondary-support design.
- Keep routing receipt links by receipt namespace:
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
- Keep the renderer-level `motionPolicy` switch that makes base graph edges and
  ambient rotations static. Use it by default for live run pages unless the
  adapter supplies evidence-bound motion events.
- Introduce a typed motion-event API before re-enabling pulses:
  - `motionId`
  - `motionKind`
  - `sourceRefs`
  - `generatedAt`
  - stale/expiry policy
  - explicit simulation flag when relevant
- Promote the gallery-only grammar items only when each accepts data:
  - contributor node from contributor/ref status
  - trace strand from submitted/queued/rejected trace refs
  - replay pair from verified/rejected challenge refs
  - receipt burst from settlement row metadata
  - corpus accretion from accepted trace refs
  - proof drawer from selected public ref
- Extend `TrainingRunBurstDefinition` if settlement bursts remain in the scene.
  `{ atId }` is not enough for accurate payout semantics.
- Do not derive motion from graph topology alone. Edges can be structural; motion
  requires evidence.

### Tests and smokes

- Worker tests:
  - public Tassadar summary includes `staleness`, settlement rows, and no private
    material
  - simulation settlement does not set real-paid fields
  - per-contributor settlement attribution matches receipt `contributorRef`
  - rejected replay projections include only public-safe mismatch refs
- Web unit tests:
  - `/tassadar` routes correctly
  - `/tassadar` startup stays public and renders `oa-tassadar-run` without an
    auth session
  - no fallback loss curve without loss evidence
  - receipt link resolver chooses the Nexus/Pylon route for Nexus receipts
  - proof drawer shows simulation caveat for the current receipt
  - row-zero/run-nonzero settlement state does not produce a misleading burst
  - no anonymous motion definitions are emitted for `/tassadar`
- Browser smoke:
  - `/tassadar` returns 200
  - exact hashed JS asset returns 200
  - the WebGL canvas is nonblank
  - the page displays generated-at/staleness text
  - base edge pulses and replay-beam traffic are absent unless backed by live
    motion events and a clear legend/proof treatment
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
