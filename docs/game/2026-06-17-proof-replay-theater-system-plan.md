# Proof Replay Theater System Plan

Date: 2026-06-17
Status: audit and implementation plan after reading all `docs/game/` direction
and `docs/launch/JUNE17_ROADMAP.md`.

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
settlement_recorded
payment_zap_confirmed
payment_zap_simulated
artifact_opened
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
10. Expose event-list, transcript, and proof-inspector mirrors for accessibility
   and auditability.

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

`apps/openagents-world-spacetimedb`:

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

## Open Decisions

- Bundle endpoint path: colocate under `/api/public/proof-replays` or run-scoped
  `/api/public/tassadar-replays` first?
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
