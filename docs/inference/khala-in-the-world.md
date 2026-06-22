# Khala in the World — Visualizing Inference Serving in the Verse

*Speculative design — 2026-06-22. Every inference request to the Khala endpoint
becomes a visible event in the Verse: crackling energy fans from the Khala nexus
to wherever the work actually goes — assigned Pylons in-world, a gateway portal
out to OpenRouter/Vertex/Fireworks, or a coding-agent avatar — and verified
results + Bitcoin settlement flow back as beams. Built on the new Effect-centric
multiplayer engine. Read as a design target, not a shipped feature.*

Companion to [`khala.md`](khala.md) (the spec), the world-backend audits in
`docs/game/`, and the spatial-thesis lineage in
`docs/sakana/tassadar-fugu-exploration.md` /
`docs/research/tmax/tassadar-tmax-exploration.md`.

## The idea

"One endpoint outside, many agents inside" (khala.md principle 1) becomes
**literal**. From the street the Khala nexus is one object. When a request hits
it, the coordinator's routing decision lights up the swarm: crackling arcs to the
Pylons it assigned, a portal stream to any external provider it called, a
coding-agent that warps in to do code work — then verified results and sats flow
back. We do *all* the pieces that go into serving inference, and you can watch
which piece served each request, keyed to a real receipt.

## What we build on (the engine, not from scratch)

The Verse is now a unified **Effect/TypeScript 3D world** (Cloudflare backend,
replacing SpacetimeDB as of `docs/game/2026-06-22-effect-typescript-world-backend-replacement-audit.md`):

- **`apps/openagents-world`** — a **Region Durable Object** per region owning
  presence, hibernatable WebSocket fanout, and hot interaction state; **D1**
  projection rows replayed from public source refs; cursor-based reconnect; a
  projection **bridge** that ingests `/api/public/*` feeds into world rows.
- **`packages/world-contract`** — the shared Effect Schema contract: typed
  **rows** (`pylon_station`, `agent_avatar`, `avatar_position`, `training_run`,
  `world_event`, `settlement_ref`, …), **commands** (browser vs **service-only**,
  actor-gated by `assertWorldCommandActorAllowed`), sparse **deltas**
  (`snapshot`/`update`/`delete`/`heartbeat`/`diagnostic`), and **cursors**.
- **`packages/world-client`** — `connect` / `subscribe` / `callCommand` /
  `applyDelta` / `reconnect`, used by desktop and web.
- **`@openagentsinc/three-effect`** + **Foldkit** — the Effect-first Three.js
  rendering stack with the primitives we need already shipped (see §5).

Two rules from the engine that govern everything here:

1. **Evidence-bound motion.** Every moving thing is keyed to a real
   `WorldSourceRef` or a state transition — no decorative pulses, and a zero-state
   fleet is visibly *still*. The source ref for this visualization is the **Khala
   receipt** (the `openagents` response block in khala.md §3: `receipt`, `route`,
   `workers`, `verification`, `cost_msat`, `settled`). Click any arc → the
   receipt.
2. **Authority boundary.** The world owns presence/interaction; the Worker/D1
   owns settlement/proof. The visualization *consumes* projected inference events;
   no browser command can fabricate a route, a verification, or a payment.

## The cast (world objects → serving pieces)

| Serving piece | World object | Basis |
|---|---|---|
| The Khala endpoint | **Khala Nexus** — central structure where requests land | reuse the run-board core / `trainingRunView` center |
| The coordinator | a small fast **nucleus** inside the nexus that fires routing energy (visibly cheaper than what it commands) | NEW small primitive |
| Assigned in-world worker | **Pylon station** (lights up, serves, gets paid) | exists: `pylon_station`, `projectChatWorldPylonScene`, `/api/public/pylon-stats` |
| External provider | **Gateway portal** — an actual gateway model: energy flows in and *offworld* to OpenRouter/Vertex/Fireworks/Claude/GPT | NEW `gateway_station` row + portal primitive |
| Code work | **coding-agent avatar** — warps in, works a terminal, returns an artifact | exists: `createAgentAvatar`, `createCharacterSpawner`, `createAgentWarpInEffect` |
| Verification | **verify glow / replay** — gold (Verified) vs red (Rejected) | exists: lifecycle stages in `trainingRunView`; verification classes from khala.md §6 |
| Settlement | **payment beam** to worker + validator, sized by sats | exists: `subscribePaymentParticles`, gold beams |
| Live metrics | **HUD meters** | exists: `createHudMeter` |

## The crackling-energy model (the core animation)

The central image: *crackling energy sent to Pylons based on what Pylons are
assigned to work.* Spec:

When a request's route decision lands, the Khala nucleus fires a **branching
electric arc** (crackling lightning, not a smooth beam) from the nexus to each
assigned destination. Arc intensity ∝ work size (tokens / `cost_msat`). The three
destinations are **visually distinct**, because the difference *is* the business:

1. **Pylon — in-world, verified, paid.** An electric-blue crackling arc to a
   Pylon station that *stays in the world*. The Pylon brightens / its mana bar
   works while serving; on a `Verified` verdict, a **gold settlement beam** flows
   back to the Pylon (worker) and to the validator. This is the lane where
   composition, verification, and Bitcoin all happen — the verified-work
   flywheel, lit up.
2. **Gateway → external (OpenRouter / Vertex / Fireworks / Claude / GPT).**
   Energy flows into the **Gateway portal** and streams *offworld* — a dimmer,
   different-colored current leaving the play space — and the answer returns
   through the portal. Margin-only: **no in-world contributor lights up**, no
   worker beam. You literally watch this work *leave the world*. (This makes the
   "how much did we keep in our paid/verified economy vs pass through?" question
   visible at a glance.)
3. **Coding agent.** Energy charges and **warps in a coding-agent avatar**, who
   walks to a workbench/pod, works a terminal session, and returns with the
   artifact; verified by its test/verification command (`test_passed`); then paid.

**Multi-worker requests (the Conductor / Fugu case)** are the payoff: the
coordinator fans energy to *several* destinations at once — a Thinker Pylon, a
Worker via the gateway portal, a Verifier — and you watch an agentic workflow
**compose across the map** in real time, each leg keyed to the same request
receipt.

## Data path: receipt → world event → render

```text
Khala serves a request
  → writes the `openagents` receipt block + metering/settlement receipt
  → surfaces on /api/public/activity-timeline (SSE; + /api/public/pylon-stats)
       │
  openagents-world bridge polls/ingests
  → assertWorldPublicSafety + source-ref audit
  → scheduled public-activity poller writes D1 projection rows and cursors
  → fresh region snapshots hydrate from D1 projection rows
  → sparse WorldDelta/live fanout for later follow-up work
       │
  world-client (desktop/web) receives delta
  → applyDelta → three-effect renders:
       arc (Khala→Pylon) | portal stream (Khala→Gateway→offworld) |
       avatar warp-in (coding) | verify glow | gold settlement beam
  → click any object → source-ref inspector → the Khala receipt
```

Multiplayer falls out of the engine for free: many humans in the same region,
via the Region DO and interest-scoped fanout, watch the *same real* request-flow
as world objects — you see the inference happening near you.

## Schema work (minimal additions, existing patterns)

Prefer modeling on existing rows; add exactly one genuinely new object.

- **New row — `gateway_station`** (interaction or projection, mirrors
  `pylon_station`): the external-provider portal. Fields: `gatewayRef`, `lane`
  (`vertex|fireworks|openrouter|passthrough`), provider label, position. This is
  the "actual gateway model" the user asked for. Implemented in
  `packages/world-contract` and mirrored by `apps/openagents-world`.
- **Inference events:** model as a specialized **`world_event`** payload
  (`requestRef`, `model` (`openagents/khala-*`), `route`, `workers[]`,
  `verification`, optional `cost_msat`, `settled`, `sourceRefs`) rather than a
  brand-new row kind — keeps the schema lean and reuses `append_world_event`.
  `cost_msat` is optional because the public paid receipt proves the event while
  intentionally omitting private amount/provider detail. (Call out the tradeoff:
  a dedicated `inference_event` row is cleaner if volume/typing warrants it
  later.)
- **Coding agents:** reuse `agent_avatar` + `run_entity` (+ `avatar_position`).
- **Commands:** add service-only `upsert_gateway_station`; otherwise reuse
  `append_world_event`, `upsert_run_entity`, `upsert_settlement_ref`. **No new
  browser command** — clients never fabricate inference/route/settlement.
- **Deltas:** standard sparse `update`/`delete`; the render layer maps an
  inference-event delta to arc/portal/avatar/beam animations.

## Rendering primitives (three-effect)

Mostly reuse; two new primitives:

- **`createCracklingArc`** — a branching electric bolt between two world points,
  intensity-parameterized, with a short crackle lifetime (sibling of
  `eventBurstPrimitives`). The signature animation. Implemented in
  `@openagentsinc/three-effect`.
- **`createGatewayPortal`** — the offworld portal/relay: energy-in, dimmer
  return, an "out of world" stream for external-provider work. Implemented in
  `@openagentsinc/three-effect`.
- **Reuse:** `createAgentAvatar` + `createAgentWarpInEffect` +
  `createCharacterSpawner` (coding agents); `projectChatWorldPylonScene` /
  `pylon_station` (workers); `subscribePaymentParticles` (settlement beams);
  `createResourceBar` (Pylon mana/earnings); `createHudMeter` (HUD);
  `trainingRunView` (the nexus core and verify lifecycle).

File seams that already do the closest thing today (Episode-240 era):
`apps/autopilot-desktop/src/shared/chat-world-visualization.ts`
(`trainingRunView`, `projectChatWorldPylonScene`, `activityEventToParticle`),
`chat-world-scene.ts`, and the payment layer (`subscribePaymentParticles`,
`motionPolicy.evidence = "required"`). The inference visualization is the same
shape as the existing payment-beam layer, extended with the crackling-arc and
gateway-portal encodings.

## HUD: the business, live

`createHudMeter` gauges, all from the receipt stream:

- **requests/sec** and **in-world vs gateway split** — how much work stayed in
  our paid/verified economy vs was passed through externally.
- **cost per accepted outcome** (the khala.md north-star), and the per-request
  worker mix.
- **accepted outcomes per kWh** (shared with the broader run-board world).
- **verified rate** — share of outcomes that cleared verification.

## Phasing

- **P0 — reuse the payment layer.** Project receipt-backed inference events from
  `/api/public/activity-timeline` the way payment particles already are; render a
  crackling arc to the assigned Pylon and a portal stream for external work.
  The public timeline event kind (`khala_inference_served`), Worker receipt
  source, world bridge mapper, gateway row, portal primitive, and desktop render
  path exist. The world worker now also has a scheduled public timeline poller,
  cursor persistence, manual poll route, and D1 snapshot hydration for new Verse
  clients. Remaining P0 work is deployed live proof: run the production bridge
  path, verify cursor/source-ref replay, and capture an owner-enabled Khala smoke
  proving the same receipt reaches the scene.
- **P1 — coding-agent lane.** Spawn agent avatars for code work; add the verify
  glow (gold/red) from the verification class.
- **P2 — multi-worker fan-out + HUD.** The Conductor/Fugu compose-across-the-map
  animation; the in-world-vs-gateway and AO/kWh meters.

**Honest gaps to close first:** the public timeline can now expose
receipt-backed Khala inference facts without private prompts or amounts, and the
world/desktop render path can consume them, including via scheduled bridge poll
and fresh-socket snapshot hydration. The remaining gap is live proof: deploy and
run the bridge path against the real timeline source, prove cursor resume and
source-ref clickthrough, and capture an owner-enabled Khala receipt flowing
through the real scene. The multiplayer engine, payment beams, Pylon scene,
agent avatars, HUD meters, and the evidence-bound motion contract already exist
— this is an extension, not a new world.

## Why it matters

The world becomes a **live, walkable projection of the Khala receipt stream** —
the most legible possible proof that inference is being served, routed, verified,
and paid. The in-world (Pylon, verified, paid) vs offworld (gateway, margin-only)
split renders the actual economics: you can *see* the verified-work flywheel
filling, and see exactly how much of each request we keep in our own paid economy
versus pass through. One endpoint outside; step inside and watch the energy go
where the work goes.
