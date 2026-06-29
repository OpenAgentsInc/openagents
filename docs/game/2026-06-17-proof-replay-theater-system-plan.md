# Proof Replay Theater System Plan

Date: 2026-06-17
Status: audit and implementation plan after reading all `docs/game/` direction
and `docs/launch/JUNE17_ROADMAP.md`.

## Implementation Log

- 2026-06-17: #5298 added the first public replay bundle resolver in
  `apps/openagents.com/workers/api/src/public-proof-replay-routes.ts`.
  It exposes `proof_replay_bundle.v1` through
  `/api/public/tassadar-replays/first-real-settlement` and the generic
  `/api/public/proof-replays` alias, with OpenAPI coverage and unit tests for
  deterministic bundle ids, source coverage, private-material redaction,
  simulation-vs-real payment classification, failed-closed blockers, and the
  first 1,000-sat Spark settlement zap.
- 2026-06-17: #5299 added `@openagentsinc/proof-replay` under
  `packages/proof-replay` as the shared web/desktop replay primitive layer.
  It provides deterministic replay clock reduction, event ordering, active
  event gating, stage and actor render plans, actor interpolation, camera cue
  pose planning, hit-target/source-inspector records, payment visual
  classification, source coverage assertions, and disposal tracking for future
  Three.js/Foldkit/ElectroBun adapters.
- 2026-06-17: #5300 added the first public website replay MVP at
  `/tassadar/replay/first-real-settlement` in `apps/openagents.com`. The live
  `/tassadar` scene now links to the replay, and the replay route renders a
  fullscreen public-safe stage with actors, the glowing Tassadar core, clocked
  event playback, play/pause/scrub/speed/camera controls, a source inspector,
  failed-closed markers, and a confirmed 1,000-sat Spark zap that appears only
  after the `payment_zap_confirmed` event. The change also repaired the worker
  deploy gate for the replay bundle resolver by registering
  `/api/public/proof-replays` and
  `/api/public/tassadar-replays/first-real-settlement` in the public projection
  staleness ledger, documenting them in `apps/openagents.com/INVARIANTS.md`,
  making the private-material replay guard use a typed error, and making the
  OpenAPI schema explicitly declare `generatedAt` plus the live-at-read
  staleness contract.
- 2026-06-17: #5301 added the first social share cut on the same website replay
  route via
  `/tassadar/replay/first-real-settlement?camera=social&duration=60&hud=social`.
  Social mode uses the same public replay bundle as the interactive page, hides
  the inspector/control HUD, renders a deterministic 16:9 canvas backdrop,
  keeps the center world focused on `Tassadar`, shows the 8:38pm June 17 hero
  timestamp, gates the Spark zap behind `payment_zap_confirmed`, and adds an
  end card with `1,000 sats settled`, `realBitcoinMoved:true`, and the public
  receipt link.
- 2026-06-17: #5302 added the secondary launch-recognition replay bundle at
  `/api/public/proof-replays?ref=launch-recognition-payments`, launchable on the
  website as `/tassadar/replay/launch-recognition-payments`. The bundle creates
  separate Trigger, Whitefang, and Orrery lanes; records intended 50,000-sat
  rewards separately from confirmed receipt/recipient-confirmation events;
  renders Whitefang and Orrery snapshot disagreements as replay gaps; keeps
  pending, failed, timeout, and pre-dispatch rows as non-payment ledger cards;
  and shows Orrery's overpayment as a hazard-pay exception lane rather than
  original payout intent.
- 2026-06-17: #5303 added the shared replay shipment gate in
  `@openagentsinc/proof-replay` and made the website replay element call it
  before rendering. The gate enforces schema, `public_safe` privacy,
  `evidence_presentation_only` claim scope, source coverage on events/flows/
  captions/gaps, private-material rejection, confirmed-zap evidence, blocked
  settlement non-payment semantics, and simulation-vs-real copy boundaries. The
  Worker resolver redaction patterns were also expanded to catch raw
  Lightning/Spark/on-chain address-looking material.
- 2026-06-17: #5308 made the replay catalog shared across web and Autopilot
  Desktop. `@openagentsinc/proof-replay` now exports the canonical replay slugs,
  website paths, social-cut path, and public bundle endpoints. The website
  replay element consumes that helper instead of hard-coding endpoint routing.
  `apps/autopilot-desktop` now has an in-app Training pane replay shelf that
  fetches the same public `proof_replay_bundle.v1` payloads over HTTPS from
  `https://openagents.com`, runs the shared shipment gate before display, shows
  first-settlement and launch-recognition bundle stats/source refs inside the
  desktop app, and labels the current cache state as a live HTTPS read with no
  offline snapshot. The social cut is available from desktop as a clear website
  handoff while the replay bundle remains inspectable in-app.
- 2026-06-17 deploy follow-up: the Worker document-route allowlist now serves
  the app shell for direct `/tassadar/replay/:slug` visits, including the social
  cut querystring. This keeps replay URLs shareable instead of redirecting them
  to `/`.
- 2026-06-17 renderer-correction follow-up: the first website replay element is
  now explicitly documented as a temporary legacy bridge, not the target visual
  architecture. It fixes the immediate play/pause interaction by handling
  controls on press before the replay timer can replace the DOM target, and it
  adds a deploy-time architecture guard against new app-local proof replay
  renderers. The permanent replay theater renderer must be promoted through
  `@openagentsinc/three-effect` and the existing `/animations` visual taxonomy:
  `@openagentsinc/proof-replay` owns bundle normalization, source gates, clocks,
  and timeline planning only; `three-effect` owns stages, avatars, payment zaps,
  camera language, particles, and world motion.

## Thesis

OpenAgents should be able to take any explicit set of public proof references
and produce a deterministic 3D replay: agents appear as avatars, walk through a
world stage, submit work, verify proofs, speak public-safe messages, and receive
Lightning or Spark sats zaps only when public receipts prove real Bitcoin moved.
The viewer can scrub time, switch camera tracks, follow an agent, orbit a proof
gate, or detach into free camera.

This is a replay theater, not a new authority surface. The replay makes proof
sets legible. It cannot create proof validity, accepted work, product promises,
settlement state, payout state, wallet state, or public claims.

## Source Audit

The game docs converge on one rule: the world can be rich and spatial only when
meaningful motion is row-backed, ref-backed, or clearly local interaction.

- The spatial HUD direction wants an MMO-like command surface where every glow
  can dereference to a receipt, proof, or authority row.
- The episode 189 run-page analysis separates live world state from historical
  replay. Historical motion is allowed only when labeled as replay and tied to
  event timestamps.
- The SpacetimeDB database plan makes `openagents-world` the projection and
  interaction layer, while Worker/D1 keeps training, proof, settlement, payout,
  receipt, and product-promise authority.
- The GCP receipt and admin runbook confirm the live endpoint at
  `https://spacetime.openagents.com`, with public subscription/identity routes
  only and service-only reducer expectations for authority projections.
- The Tassadar integration plan proves the first row-backed surface:
  `/tassadar` starts from `GET /api/public/tassadar-run-summary`, then layers
  SpacetimeDB rows for runs, entities, proof refs, settlement refs, world
  events, pylon stations, avatars, positions, attention, and chat.
- The WASD/mouselook plan adds the navigation substrate: first-person movement,
  reticle selection, proof-safe node inspection, and future camera presets.
- The agent-avatar chatter plan adds inhabited-world semantics: pylon agents can
  be visible avatars, notice visitors, move, speak locally, and render bubbles
  without becoming proof authority.
- The Quick 3D MMORPG harvest plan provides the reusable mechanics vocabulary:
  entity interpolation, spatial hash layout, animation FSMs, model handles,
  attachments, billboard bubbles, evidence-gated event bursts, terrain, camera
  modes, hit-target registries, and VFX primitives.
- The asset catalog gates visuals: production avatars, stations, proof gates,
  settlement terminals, particles, and adornments must be owned or explicitly
  provenance-approved.
- The June 17 launch roadmap says real-money settlement is still receipt-first
  and gated. Replay payment effects therefore must distinguish confirmed real
  Bitcoin movement from simulation, pending, deferred, or owner-gated scaffold
  states.

## Replay Inputs

A replay starts from an explicit proof set. The set can be assembled by a user,
generated from a run, linked from a pylon, or produced by an audit tool, but it
must resolve through public-safe OpenAgents authority surfaces.

Accepted source refs:

- public proof, challenge, trace, receipt, registry, or verdict refs;
- public settlement refs, including `movementMode` and `realBitcoinMoved`;
- public training run refs and pylon refs;
- public SpacetimeDB `world_event`, `proof_ref`, `settlement_ref`,
  `pylon_station`, `agent_avatar`, `avatar_position`, `local_chat_message`, and
  `chat_bubble` rows;
- public product-promise or accepted-work refs when the replay needs to show a
  claim boundary;
- public artifact refs that already passed existing projection/redaction rules.

Rejected inputs:

- raw prompts, private runtime logs, private repo contents, provider payloads,
  shell transcripts, customer data, wallet mnemonics, service tokens, payment
  preimages, or internal operator-only packets;
- anonymous client-authored proof or settlement claims;
- generated narration that claims beyond cited source refs;
- fixture chatter or demo payment motion presented as live evidence.

If a proof set is incomplete, the replay should render an explicit gap rather
than inventing missing work. Missing timestamps can be represented by a stable
sequence order with a visible "ordered by bundle, timestamp unavailable" caveat.

## Replay Bundle Contract

The system should normalize inputs into a versioned replay bundle before any 3D
rendering. Same bundle plus same renderer version should produce the same
timeline and default director track.

Suggested bundle shape:

```ts
type ProofReplayBundle = Readonly<{
  bundleRef: string
  schemaVersion: "proof_replay_bundle.v1"
  generatedAt: string
  sourceRefs: readonly ReplaySourceRef[]
  sourceAuthority: "worker_d1_public" | "spacetimedb_projection" | "mixed_public"
  staleness: "fresh" | "stale" | "snapshot"
  privacyLevel: "public_safe"
  claimScope: "evidence_presentation_only"
  actors: readonly ReplayActor[]
  stages: readonly ReplayStage[]
  events: readonly ReplayEvent[]
  flows: readonly ReplayFlow[]
  cameraCues: readonly ReplayCameraCue[]
  captions: readonly ReplayCaption[]
  gaps: readonly ReplayGap[]
}>
```

Core records:

- `ReplaySourceRef`: dereferenceable public URL/ref, source kind, observed
  timestamp, digest if available, and source projection freshness.
- `ReplayActor`: stable actor ref, avatar role, display name, public identity
  ref, pylon ref if any, approved asset id, and fallback procedural asset id.
- `ReplayStage`: run region, pylon station, proof gate, trace gate, verifier
  desk, settlement terminal, registry marker, or run core.
- `ReplayEvent`: timestamp or sequence index, event kind, actor refs, target
  refs, source refs, display text, state before/after, and privacy caveat.
- `ReplayFlow`: directed relation between two actors or stages, such as work
  handoff, verification check, receipt emission, or payment movement.
- `ReplayCameraCue`: deterministic camera mode, focus refs, spline points,
  duration, easing, and interrupt policy.
- `ReplayCaption`: public-safe narration text with exact source refs.
- `ReplayGap`: missing or redacted segment with reason and affected refs.

Event kinds should start narrow:

```text
actor_entered_region
actor_moved
actor_focused_pylon
actor_said_public_message
proof_submitted
proof_verified
proof_rejected
trace_linked
receipt_recorded
settlement_blocked_closed
payout_intent_persisted
settlement_recorded
payment_zap_confirmed
payment_zap_simulated
recognition_reward_recorded
recipient_confirmation_recorded
overpayment_detected
artifact_opened
forum_announcement_posted
claim_boundary_shown
```

`payment_zap_confirmed` requires a public settlement/receipt ref with
`realBitcoinMoved:true` or an equivalent future receipt-first field. Simulation,
pending, deferred, or owner-gated settlement must use a separate event kind and
visual treatment.

## Pipeline

1. Resolve source refs through existing public Worker/D1 APIs first. Use
   SpacetimeDB rows as projection and interaction context, not as settlement or
   proof authority.
2. Expand each ref into public-safe source records. Preserve URLs, timestamps,
   run ids, pylon refs, receipt refs, movement mode, and freshness.
3. Run a redaction gate that drops private prompts, raw logs, provider payloads,
   wallet data, service tokens, and operator-only fields before bundle creation.
4. Deduplicate source refs and actors. Multiple proofs can point to one pylon,
   one avatar, one receipt, or one settlement terminal.
5. Build a deterministic timeline using authoritative timestamps. Fall back to
   explicit sequence indices only when timestamps are absent.
6. Bind actors to approved assets. Use procedural approved-owned glyphs when no
   approved avatar model exists.
7. Plan a stage layout from source refs: run core in the center, pylon stations
   around it, proof gates near relevant actors, settlement terminals near payment
   flows, and registry markers at evidence boundaries.
8. Generate default camera cues. The cues are presentation metadata only; they
   must not reorder events or hide gaps.
9. Render through `three-effect` with replay time as the clock. Live
   SpacetimeDB presence can show co-viewers, but cannot mutate the bundle.
   App-local DOM, CSS, 2D canvas, or ad hoc WebGL replay visuals are a
   migration bug: DOM belongs to controls, source inspection, captions,
   transcripts, loading/error states, and accessibility mirrors only.
10. Expose event-list, transcript, and proof-inspector mirrors for accessibility
   and auditability.

## Visual Renderer Ownership

The replay theater has two separate packages with different jobs:

- `@openagentsinc/proof-replay`: replay bundle schema, shipment gate, source
  coverage, deterministic clock reduction, actor tracks, camera cues, and
  public-safe render-plan data.
- `@openagentsinc/three-effect`: the visual taxonomy and renderer: world stage,
  agents/avatars, proof gates, settlement terminals, payment zaps, evidence
  bursts, camera modes, particles, terrain, hit targets, labels, and interaction
  primitives.

Website and desktop app code should compose those packages, not replace either
one. The app can mount `three-effect`, bind replay playback state, and render
Foldkit chrome around it. It must not create a second visual language in a
custom element or a desktop-only canvas. If the replay needs a new visual
primitive, add it to the sibling `three-effect` repository first, expose it
through the package, then consume it from both `apps/openagents.com` and
`apps/autopilot-desktop`.

## Visual Grammar

The default replay should be understandable without reading a dashboard.

Agents:

- pylon agents, service agents, verifiers, contributors, and guests appear as
  avatars or procedural role glyphs;
- avatars walk, face targets, idle, inspect, talk, verify, settle, or show
  blocked/stale states using the shared animation FSM vocabulary;
- pylon ownership or capability adornments must come from public pylon,
  reputation, or asset-catalog refs.

Proof:

- a proof submission appears as an actor carrying or projecting an artifact to a
  proof gate;
- verification appears as a short evidence-backed burst or gate transition;
- rejected, blocked, stale, or incomplete proofs use static warning states and
  captions rather than exciting success motion;
- clicking any proof object opens the existing public proof/receipt/trace
  inspector path.

Payments:

- confirmed sats movement appears as a zap from payer, treasury, escrow, or
  settlement terminal to the receiving actor or pylon;
- the zap label should show amount, rail when public, timestamp, and receipt
  link;
- Spark-native, Lightning, MDK, or future rails can share the zap grammar but
  must keep their public receipt fields distinct;
- simulated, pending, deferred, owner-gated, or scaffolded settlement uses a
  muted non-zap path and an explicit label. It must never look like confirmed
  money movement.

Chat and local world signals:

- public-safe chat rows render as speech bubbles and transcript entries;
- pylon-targeted messages can anchor to both speaker and station when backed by
  one public row;
- private DMs, prompts, operator notes, and raw logs stay out of replay.

Camera:

- `overview`: show the whole proof set and all actors;
- `follow_actor`: follow one avatar through the evidence chain;
- `orbit_proof`: circle a proof gate while the inspector highlights source refs;
- `zap_focus`: brief slow camera move around a confirmed payment zap;
- `free_camera`: viewer-controlled movement, with replay time still authoritative;
- `director_track`: deterministic authored path stored with bundle/version.

Every replay screen should include a persistent replay label, bundle ref,
timeline time, and source freshness. Historical playback must not be visually
confused with live world activity.

## Product Surface

Start inside the existing run world:

- `/tassadar` gets a replay mode launched from selected proof, receipt,
  settlement, run, or pylon inspector rows.
- A shareable route can follow once the bundle endpoint exists:
  `/tassadar/replay?refs=...` or `/replay/<bundleRef>`.
- The player includes play/pause, scrubber, speed, camera mode, event list,
  selected-source inspector, transcript, and privacy/source banner.
- The first MVP can use current procedural entities: run core, pylon station,
  proof gate, settlement terminal, registry marker, avatar marker, labels, and
  evidence-backed bursts.

Do not put lifecycle metadata nodes in the center of the world. The center can
be the run core, such as a glowing `Tassadar` object, while metadata and replay
controls stay in compact top/HUD surfaces.

## Implementation Homes

Worker/D1 public authority:

- resolve arbitrary proof sets into public source records;
- expose a public-safe replay bundle endpoint;
- preserve receipt-first settlement fields;
- enforce redaction and claim-boundary tests.

the deleted legacy world module:

- optionally store replay watch-party interaction state: viewers, co-viewer
  avatars, reactions, camera-follow intent, and local public chat;
- optionally project public `world_event` refs that help replay context;
- never own proof validity, settlement truth, accepted work, payout, or product
  promise state.

`@openagentsinc/three-effect`:

- replay timeline player and deterministic clock;
- actor interpolation and route planning against static replay events;
- camera director primitives and camera preset switching;
- approved payment zap, proof gate, settlement terminal, registry marker, and
  replay gap VFX primitives;
- billboard captions, speech bubbles, nameplates, and hit-target inspection.

`apps/openagents.com`:

- proof-set builder UI from selected entities;
- replay player and route integration;
- source inspector and transcript mirror;
- fallback behavior when SpacetimeDB is down;
- deploy-facing browser smoke for nonblank canvas, controls, and proof links.

Docs and registry:

- update asset catalog when new replay props, avatars, zap textures, or camera
  markers become production assets;
- update product-promise docs only if replay becomes a public claim surface;
- keep runbooks clear that replay bundles are public-safe snapshots.

## Phased Plan

P0: bundle contract and static local replay

- Define `proof_replay_bundle.v1` in docs and code.
- Build a static bundle from the current Tassadar public summary and settlement
  refs.
- Render a deterministic local replay using approved procedural assets.
- Show actor movement, proof gate, receipt event, replay gaps, event list, and
  inspector links.
- Do not animate money movement unless the source receipt says real Bitcoin
  moved.

P1: player controls and camera director

- Add replay clock, play/pause/scrub/speed.
- Add overview, follow actor, orbit proof, zap focus, and free camera modes.
- Keep the current first-person controls available as free camera, not as live
  proof motion.
- Add textual transcript and event-list parity.

P2: arbitrary proof-set resolver

- Accept explicit proof/run/pylon/receipt refs.
- Resolve through public Worker/D1 endpoints and public SpacetimeDB projection
  rows.
- Deduplicate actors, receipts, events, and flows.
- Add snapshot tests for redaction, ordering, and deterministic bundle output.

P3: confirmed sats zap grammar

- Bind settlement refs into payment flow records.
- Render `payment_zap_confirmed` only for confirmed movement.
- Render simulation/pending/deferred paths with separate muted styling.
- Add tests proving simulated or missing receipts cannot produce confirmed zap
  events.

P4: shared replay sessions

- Use SpacetimeDB interaction rows for co-viewers, reactions, and public-safe
  local watch-party chat.
- Let viewers see each other walking around the replay stage without mutating
  the evidence bundle.
- Keep replay bundles immutable or versioned.

P5: asset and export polish

- Replace procedural fallback avatars/props with approved owned GLB assets.
- Add machine-readable asset manifest validation.
- Add shareable replay URLs and optional video capture/export.
- Add director-track authoring only after deterministic replay and claim
  boundaries are locked.

## Verification Gates

Before shipping a replay feature:

- redaction tests prove no private prompts, raw logs, wallet material, service
  tokens, provider payloads, or operator-only rows enter bundles;
- bundle snapshot tests prove deterministic output for the same source refs;
- ordering tests cover timestamped and sequence-indexed events;
- payment tests prove only confirmed receipt-first movement produces
  `payment_zap_confirmed`;
- source coverage tests prove every event, flow, caption, burst, and zap has at
  least one dereferenceable source ref or a replay-gap caveat;
- app tests prove SpacetimeDB outage falls back to static public bundle
  rendering when possible;
- browser smoke proves the canvas is nonblank, replay controls work, camera
  modes switch, proof inspector links open, labels do not overlap, and mobile
  layout keeps controls readable;
- asset checks prove production replay assets are approved-owned or explicitly
  production-eligible.

## First MVP

The fastest useful slice is a Tassadar proof replay generated from the existing
public summary:

1. Select the canonical run or a pylon/settlement/proof in `/tassadar`.
2. Build a bundle from public summary refs plus any matching SpacetimeDB
   projection rows.
3. Render the run core as `Tassadar`, pylon stations around it, pylon-agent
   avatars at their row-backed positions, proof gates for proof/trace refs, and
   one settlement terminal.
4. Animate avatars walking to gates and terminals in timeline order.
5. Render a confirmed sats zap only if the current settlement row proves real
   Bitcoin movement. Otherwise render the settlement as simulation/pending with
   muted non-payment styling.
6. Let the viewer scrub, follow a pylon agent, orbit a proof gate, open the
   proof inspector, and read the exact event transcript.

This would make a proof set visible as a world event without weakening the
current authority model.

## First Replay: Run 1 Real Settlement

The first replay should be the first confirmed real Bitcoin Tassadar
run-settlement. This is the first example worth building toward because it has
the entire story shape the replay system exists to show:

- independent exact-trace verification;
- owner-gated settlement authorization;
- the prior simulation-only settlement promise state, explicitly shown as a
  rehearsal/projection proof rather than the first real money movement;
- two failed-closed real-dispatch bugs that moved no sats;
- durable payout intent, Spark treasury dispatch, reconciliation, and public
  receipt;
- a public Forum announcement asking the recipient pylon operator to verify the
  wallet-side arrival.

Owner-provided milestone time: 2026-06-17 8:38pm America/Chicago, the first
real settlement moment. Public UTC receipt/forum timestamps should still be
shown in inspectors when available, but the social replay should label the
historical beat as "8:38pm, June 17".

The replay title should be direct and historical:

```text
Tassadar Run 1: First Real Bitcoin Settlement
```

### Public Source Refs

The first replay bundle should be built only from public-safe refs:

| Kind | Ref |
| --- | --- |
| Run | `run.tassadar.executor.20260615` |
| Window | `training.window.tassadar.executor.20260615.w1` |
| Verified challenge | `training.verification.challenge.071445c5-6ad6-4136-87e3-253b01914b4c` |
| Contributor pylon | `pylon.448ba824b5fc879f3a59` |
| Real settlement receipt | `receipt.nexus.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618` |
| Payout intent | `payout_intent.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618` |
| Payout attempt | `payout_attempt.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618` |
| Reconciliation | `reconciliation.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618` |
| External event | `external_event.tassadar_run_settlement.spark_treasury.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618` |
| Failed-closed Forum update | `https://openagents.com/forum/t/34bebe36-1c7c-443a-b7e2-13ec521955d9#post-1dce5715-ec37-4850-a484-e7fe329417aa` |
| Settled Forum update | `https://openagents.com/forum/t/34bebe36-1c7c-443a-b7e2-13ec521955d9#post-a8df2265-547a-4a18-9398-3e7412a6859a` |

The receipt API is already public and dereferenceable:

```text
https://openagents.com/api/public/nexus-pylon/receipts/receipt.nexus.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618
```

Its public projection currently shows:

```text
realBitcoinMoved: true
movementMode: real_bitcoin
moneyMovement: real_bitcoin
state: settled
amountSats: 1000
adapter: spark_treasury
contributorRef: pylon.448ba824b5fc879f3a59
```

The public run summary currently carries this real row in `settlementRows`,
alongside the older 5-sat simulation row. The replay resolver must select the
real row by receipt ref or by `realBitcoinMoved:true`; it must not turn the
simulation row into a zap.

The product-promise context matters for the story: the training launch gate had
already been treated as green for the simulation-backed settlement-record path.
This replay must make the distinction visible. The 5-sat simulated receipt is a
ghost rehearsal, the gate/copy caveat; the 1,000-sat Spark receipt is the first
real sats movement.

### Code Surfaces Already In Place

The first replay should use existing code paths rather than scrape Forum text:

- `apps/openagents.com/workers/api/src/public-tassadar-run-summary-routes.ts`
  exposes `settlementRows`, `movementMode`, `realBitcoinMoved`, receipt URLs,
  contributor refs, and source refs for `/api/public/tassadar-run-summary`.
- the deleted legacy bridge scripts
  maps settlement rows into `settlement_ref` rows, `settlement_receipt`
  entities, `pylon_to_settlement` edges, and `settlement_projected` world
  events. Its `settlementStatus` already returns `real_settled` when
  `realBitcoinMoved` is true.
- `apps/openagents.com/workers/api/src/tassadar-run-settlement.ts` builds the
  public ledger chain and maps `spark_treasury` to `moneyMovement:
  real_bitcoin`.
- `apps/openagents.com/workers/api/src/training-run-window-routes.ts` drives
  the real settlement dispatch only after the owner gate authorizes the
  `spark_treasury` adapter, wallet readiness is ready, the destination resolves,
  payout intent is durable, dispatch succeeds, reconciliation matches, and a
  `settlement_recorded` receipt can be persisted.
- `apps/openagents.com/workers/api/src/nexus-treasury-payout-ledger.ts`
  now verifies that `INSERT OR IGNORE` did not silently drop the payout intent.
  This is part of the replay story because the first real attempt exposed that
  failure mode and it failed closed.
- `apps/openagents.com/workers/api/migrations/0203_nexus_payout_adapter_kind_spark_treasury.sql`
  adds `spark_treasury` to the payment-authority ledger CHECK constraints.
- `apps/openagents.com/workers/api/migrations/0204_nexus_payment_authority_receipts_fk_repair.sql`
  repairs the receipt table FK target after the 0203 table rebuild.

### Cinematic Timeline

The first replay should not start at the zap. It should show why the zap
matters.

1. **Cold open:** camera faces the glowing `Tassadar` run core in replay mode.
   A compact top HUD says `Replay`, the bundle ref, and the public source
   freshness. The center world contains only the run core, pylon stations,
   avatars, proof gate, and settlement terminal.
2. **Contributor and validator enter:** the contributor pylon avatar walks from
   its station to the proof gate. The validator avatar is already at the gate or
   enters from the opposite side. The stage labels the run and window refs.
3. **Exact replay verification:** the proof gate projects the verified challenge
   ref. The validator checks it; a short verification burst fires only because
   the challenge is `Verified` and `exact_trace_replay`.
4. **Owner gate opens:** a narrow gate or seal appears above the settlement
   terminal for the bounded approval. It should say the branch is authorized,
   not that money has moved.
5. **Failed-closed preface:** two muted red/amber gap markers appear before the
   final payment:
   - `payout_intent_not_found`: payout intent persistence/lookup failed, no
     dispatch, no sats moved, no settled receipt.
   - `adapter_unavailable`: treasury adapter was not ready, no sats moved, no
     settled receipt.
   These are important parts of the first replay because they prove the rail
   failed closed under real pressure.
6. **Infra repair markers:** small non-payment markers show the durable intent
   fix and the `spark_treasury` ledger/migration repair. These markers link to
   code refs or release refs when available, but they do not become payment
   effects.
7. **Real settlement:** the settlement terminal receives the
   `settlement_recorded` receipt. The payment zap fires only now:
   `spark_treasury -> pylon.448ba824b5fc879f3a59`, `1000 sats`,
   `realBitcoinMoved:true`.
8. **Forum announcement:** the settled Forum post appears as a public-safe
   speech bubble or transcript card anchored to a service/announcer avatar. It
   links to the exact post permalink and asks for recipient-side wallet
   confirmation.
9. **End frame:** camera pulls back to show the run core, contributor pylon,
   proof gate, settlement terminal, and a settled receipt link. The event list
   remains clickable so the viewer can open the public receipt and Forum post.

### Twitter Share Cut

The first implementation should include a shareable cut that can be posted to
Twitter/X without requiring the viewer to understand the whole HUD.

Target format:

- 45-60 seconds, 16:9 landscape first; later add square and vertical crops.
- title card: `Tassadar Run 1: first real Bitcoin settlement`.
- subtitle: `Verified work -> owner gate -> Spark zap -> public receipt`.
- hero timestamp: `8:38pm, June 17`.
- center world: glowing `Tassadar` run core, not lifecycle metadata.
- bottom caption strip: short event captions with source refs hidden behind
  inspectable links or end-card QR/short URL.
- final frame: `1,000 sats settled`, `realBitcoinMoved:true`, receipt ref tail,
  and forum post link/QR.

The share cut should use the same replay bundle as the interactive page. It may
use a preauthored camera track and cleaner captions, but it must not use a
different truth source or add cinematic payment motion that the interactive
replay would reject.

Useful route/export direction:

```text
/tassadar/replay/first-real-settlement?camera=social&duration=60&hud=social
```

Recording/export runbook for the first Twitter/X artifact:

1. Open the route above in production or a verified local build. Do not open
   DevTools, private logs, admin pages, or wallet/operator surfaces in the
   captured browser window.
2. Capture at 1920x1080 first. A 1280x720 smoke is also required because the
   social canvas is authored at 1280x720 and scaled by the browser.
3. Record exactly one 60-second pass from `start=0`. Use
   `&start=56` only for a quick final-card inspection, not for the exported
   artifact.
4. Confirm the early frames show no real zap, the late frames show the
   `payment_zap_confirmed` Spark rail, and the end card links to the public
   receipt endpoint.
5. Scan the exported frame text for private refs, raw addresses, payment
   hashes, preimages, keys, prompts, bearer/service tokens, or operator-only
   notes before posting.

Before exporting a clip, browser smoke should verify:

- the canvas is nonblank in the social camera mode;
- labels and captions fit at 1920x1080 and 1280x720;
- the zap is absent until `payment_zap_confirmed`;
- the receipt link/end card resolves;
- no private refs, raw addresses, payment hashes, preimages, keys, or prompts
  appear in any frame.

### First Bundle Event Sketch

The first generated bundle should be able to emit a compact timeline like this:

```text
00:00 actor_entered_region contributor=pylon.448ba824b5fc879f3a59
00:05 proof_submitted source=training.verification.challenge.071445c5-...
00:10 proof_verified source=training.verification.challenge.071445c5-...
00:15 claim_boundary_shown source=owner_gate_public_refs
00:20 settlement_blocked_closed reason=payout_intent_not_found
00:25 payout_intent_persisted source=payout_intent.tassadar_run_settlement...
00:30 settlement_blocked_closed reason=adapter_unavailable
00:35 settlement_recorded source=receipt.nexus.tassadar_run_settlement...v6.20260618
00:36 payment_zap_confirmed amountSats=1000 rail=spark adapter=spark_treasury
00:42 forum_announcement_posted source=post-a8df2265-...
```

If the bundle cannot prove a timestamp for an intermediate failed-closed
marker, it should use explicit `sequenceIndex` and label that segment as
Forum-announced operational history. The real zap still uses the receipt as the
source of truth, not the Forum text.

### First Replay Acceptance Criteria

The first replay is not complete until it proves these behaviors:

- It can be launched from the real receipt row, the selected pylon, or the run.
- It opens the real public receipt URL from the zap and the exact Forum post URL
  from the announcement bubble.
- It renders the older simulation settlement row as simulation, not as a zap.
- It renders the two failed-closed steps as non-payment blockers, not failures
  after money moved.
- It shows no raw recipient address, preimage, payment hash, mnemonic, Spark API
  key, service token, provider payload, private log, or private operator note.
- It keeps `Tassadar` or the run core in the center and leaves metadata in HUD
  controls, matching the current `/tassadar` canvas rule.
- It passes a browser smoke where the scene is nonblank, the camera track plays,
  scrub/pause works, the proof gate and zap are selectable, and the public
  receipt inspector opens.

## Secondary Replay: Launch Recognition Payments

The second replay target should visualize the launch-recognition payouts:
50,000-sat rewards to the public agent recipients, plus the accidental overpay
story around the contributor who received more than intended. This should be a
separate replay from the run-settlement canary because its authority and meaning
are different: recognition payouts thank launch contributors; they are not proof
that the training run can settle arbitrary accepted work at scale.

Public display-name lanes to resolve from the current public-safe refs:

- `Trigger`: intended 50,000-sat recognition reward.
- `Whitefang`: intended 50,000-sat recognition reward, plus the small Spark
  treasury proof sends that established the rail.
- `Orrery`: intended 50,000-sat recognition reward plus the accidental
  overpayment/keep-as-hazard-pay incident.

Source docs to resolve before rendering:

- `docs/launch/JUNE17_ROADMAP.md`
- `docs/payments/2026-06-17-launch-recognition-closeout.md`
- `docs/payments/2026-06-17-launch-recognition-spark-recipient-status.md`
- any current public treasury, receipt, recipient-confirmation, or Forum refs
  linked from those docs or live APIs.

The intended story beats:

1. **Recognition ledger opens:** three agent avatars stand around a launch
   recognition terminal. Each has an intended reward marker: `50,000 sats`.
2. **Confirmed reward path:** confirmed recipient-side or receipt-backed
   50,000-sat payments animate as recognition zaps or ribbons. These should use
   a distinct recognition visual style from the run-settlement Spark zap, so
   viewers do not confuse a one-time reward with ongoing accepted-work payout.
3. **Pending/failed rows stay non-payment:** local pending rows, pre-dispatch
   failures, and timeout/debug rows render as gray/amber ledger cards, not as
   sats moving.
4. **Overpayment branch:** the contributor overpayment is shown as a branching
   cascade of split settled sends, failed attempts, pending rows, and an
   overage meter. The visual should make clear which rows actually settled and
   which failed before dispatch.
5. **Owner decision marker:** if the public docs say the recipient keeps the
   overage as hazard pay, render that as an operator-decision marker after the
   settled rows, not as if the overage was the original intended amount.
6. **End frame:** show the three intended 50,000-sat recognition lanes, the
   confirmed received evidence for each lane, and the overpayment/exception lane
   as a documented historical incident.

Replay grammar:

- `recognition_reward_recorded`: an intended or confirmed recognition payment
  event. Confirmed events require public receipt or recipient-confirmation refs.
- `recipient_confirmation_recorded`: public-safe recipient-side confirmation,
  such as a backup-status/report ref.
- `settlement_blocked_closed`: failed before dispatch, no settled row, no sats
  moved.
- `overpayment_detected`: public-safe overage accounting, with settled rows and
  the owner decision refs attached.

Guardrails:

- Do not hardcode "three 50,000-sat zaps" from memory. The replay resolver must
  build the payment rows from current public-safe receipts/status docs and show
  disagreements between snapshots as historical state changes or gaps.
- Do not show pending rows as received money.
- Do not show failed pre-dispatch attempts as money that left a wallet.
- Do not expose raw Lightning Addresses, BOLT11 invoices, payment hashes,
  preimages, mnemonics, Spark/Breez keys, or private wallet paths.
- Do not reuse the training run's `realBitcoinMoved:true` copy for recognition
  payouts unless the recognition row itself has equivalent public evidence.

This secondary replay is worth building because it gives a social artifact for
the broader launch week: three agents got meaningful recognition payouts, the
system encountered real wallet/rail edge cases, and the public accounting stayed
honest enough to distinguish settled, failed, pending, and overpaid states.

Current route:

```text
/tassadar/replay/launch-recognition-payments
```

Implementation notes:

- Trigger is rendered from the recipient-side 50,000-sat Spark backup
  confirmation.
- Whitefang shows the older pending-funding state from the closeout/status docs
  and the later roadmap closeout state as a historical snapshot change.
- Orrery shows the intended 50,000-sat lane, split settled coverage, failed
  before-dispatch rows, pending/orphaned rows as non-payment accounting, and a
  109,239-sat overage meter from the latest recipient-visible 159,239-sat
  roadmap closeout value.
- The replay uses public docs and public-safe recipient-confirmation refs only;
  raw payment ids, addresses, invoices, hashes, preimages, wallet paths, and
  service credentials stay out of the bundle.

## Open Decisions

- Replay storage: generate on demand, store immutable snapshots, or cache by
  source-ref digest?
- Source freshness: how stale can a replay bundle be before the UI forces a
  refresh banner?
- Actor identity: when a proof ref lacks an agent avatar, should fallback actors
  be anonymous procedural glyphs or public pylon ghosts?
- Export: should video capture happen in-browser, in a Worker-compatible render
  path, or as an operator-only job?
- Director tracks: are authored camera paths stored as part of the public bundle
  or as separate presentation overlays?

## Verification Gates

CI-safe replay gates:

```text
bun run --cwd packages/proof-replay test
bun run --cwd packages/proof-replay typecheck
bun test tests/proof-replays.test.ts tests/cl-53-foldkit.test.ts tests/cl-53-sanitize.test.ts
# from apps/autopilot-desktop, uses its bunfig preload
bun build src/ui/main.ts --outdir /tmp/autopilot-desktop-ui-build --target browser
bun build src/bun/index.ts --outdir /tmp/autopilot-desktop-bun-build --target bun
bun run --cwd apps/openagents.com/workers/api test -- src/public-proof-replay-routes.test.ts
bun run --cwd apps/openagents.com/workers/api typecheck
bun run --cwd apps/openagents.com/apps/web test -- src/scene/tassadarProofReplayElement.test.ts
bun run --cwd apps/openagents.com/apps/web typecheck
bun run --cwd apps/openagents.com check:deploy
```

Cross-surface replay checks:

1. Website first replay:
   `/tassadar/replay/first-real-settlement`.
2. Website social cut:
   `/tassadar/replay/first-real-settlement?camera=social&duration=60&hud=social`.
3. Website recognition replay:
   `/tassadar/replay/launch-recognition-payments`.
4. Autopilot Desktop: open `Training`, use `Proof Replays`, select `First
   settlement` and `Recognition`, and confirm the panel loads bundle metrics,
   source refs, payment/gap rows, `Open web replay`, `Open social cut` for the
   first settlement replay, and `Open bundle API`.
5. Desktop cache label must read as live HTTPS/no offline snapshot unless a
   future explicit snapshot system is implemented. A failed load must say no
   offline snapshot is available rather than implying current settlement or
   wallet state.

The shared shipment gate must pass before browser rendering. It rejects:

- bundle schema or claim-boundary drift;
- missing source refs on events, flows, captions, or gaps;
- raw private prompts, logs, provider payloads, customer data, wallet paths,
  mnemonics, service tokens, payment hashes/preimages, raw BOLT11/Lightning,
  Spark, or on-chain address-looking values;
- confirmed zaps without public receipt or recipient-confirmation evidence;
- blocked settlement rows carrying moving sats;
- simulated payment rows claiming `realBitcoinMoved:true`.

Operator-only visual smoke, when a browser is available:

1. Open `/tassadar/replay/first-real-settlement`.
2. Open `/tassadar/replay/first-real-settlement?camera=social&duration=60&hud=social`.
3. Open `/tassadar/replay/launch-recognition-payments`.
4. Check 1920x1080 and 1280x720. The stage/canvas must be nonblank; labels must
   not overlap; early first-settlement frames must have no confirmed zap; late
   settlement and recognition frames must show only source-backed zaps; source
   inspector links must open public refs.
5. Scan visible text for private/payment/operator material before capturing or
   posting social video.
