# Effect/TypeScript World Backend Replacement Audit

Date: 2026-06-22
Status: architecture audit and replacement-path exploration, not an active
authority change.

## Executive Read

Replacing the SpacetimeDB world backend with an OpenAgents-owned
Effect/TypeScript backend is plausible, and it fits the broader repo direction:
Bun, Effect, Effect Schema, Foldkit, `three-effect`, public Worker/D1
authority, and typed contracts.

It is not a small refactor. The current SpacetimeDB lane is already a live
product subsystem:

- a self-hosted GCP VM at `https://spacetime.openagents.com`;
- Rust/WASM module source in `apps/openagents-world-spacetimedb`;
- service-only reducers for public projection rows;
- browser/user reducers for bounded interaction rows;
- generated TypeScript bindings consumed by website and desktop paths;
- a desktop multiplayer client, pose publisher, remote avatar renderer,
  region-scoped subscriptions, high/low presence feeds, and a two-client smoke.

The right path is therefore not "rip out SpacetimeDB." It is:

1. Define a backend-neutral **Verse world contract** in Effect Schema.
2. Build an Effect/TypeScript world service that can replay the same public
   projection bridge and expose the same presence/subscription semantics.
3. Run it as a compatibility mirror behind a feature flag.
4. Switch desktop/web clients only after the mirror passes the existing
   SpacetimeDB multiplayer, outage, projection, and proof-safety gates.
5. Update `INVARIANTS.md` only when the default production backend actually
   changes.

The conclusion is a guarded yes: we should explore and prototype an
Effect/TypeScript replacement, but keep SpacetimeDB as the production world
backend until the custom service proves parity on the hard parts:
authorization, live subscriptions, bounded reducer semantics, replayable
projection bridges, row expiry, near/far presence, local cache behavior, and
non-fatal outages.

## Sources Reviewed

This audit follows the full `docs/game/` tree and the `docs/game/woc/` study
set. The load-bearing prior docs are:

- `2026-06-16-spatial-hud-agentic-mmo-wow-direction.md`
- `2026-06-17-episode-189-agentic-mmorpg-run-page-analysis.md`
- `2026-06-17-spacetimedb-openagents-mmo-database-plan.md`
- `2026-06-17-spacetimedb-gcp-deployment-receipt.md`
- `2026-06-17-spacetimedb-admin-runbook.md`
- `2026-06-17-spacetimedb-tassadar-integration-next-steps.md`
- `2026-06-17-tassadar-wasd-mouselook-controller-plan.md`
- `2026-06-17-agent-avatar-proximity-chatter-world-plan.md`
- `2026-06-17-openagents-world-asset-catalog.md`
- `2026-06-17-proof-replay-theater-system-plan.md`
- `2026-06-17-quick-3d-mmorpg-full-mechanics-harvest-plan.md`
- `2026-06-21-spacetimedb-verse-multiplayer-audit.md`
- `2026-06-21-autopilot-auto-forum-loop-and-verse-reflection-audit.md`
- `2026-06-21-verse-scene-graph-vs-react-three-fiber-audit.md`
- `2026-06-21-mmo-characters-per-account-verse-presence.md`
- `2026-06-22-verse-custom-keybindings-audit.md`
- `woc/01-overview.md`
- `woc/02-hud-and-hotbar.md`
- `woc/03-input-camera-targeting.md`
- `woc/04-multiplayer-netcode.md`
- `woc/05-chat-minimap-world.md`
- `woc/06-adaptation-plan.md`

## Why Even Consider Replacing SpacetimeDB?

SpacetimeDB was the right first substrate. It gave the Verse an immediate,
coherent model:

- tables as shared world state;
- reducers as the only mutation path;
- live row subscriptions;
- generated TypeScript clients;
- client-local caches;
- a strong reference model from BitCraft, Minecraft, and SpacetimeDB examples.

The current docs show that this let us get real multiplayer shape quickly:
regions, pylon stations, avatars, positions, attention rows, local chat,
bubbles, emotes, intent, near/far feeds, and service bridge projection.

The replacement pressure comes from ownership and alignment:

- **Two runtime stacks.** The world module is Rust/WASM and SpacetimeDB-specific
  while the repo's new production TypeScript direction is Bun + Effect +
  Effect Schema.
- **Operational overhead.** The live backend is a GCE VM, data disk, nginx/TLS,
  certbot, SpacetimeDB binaries, WASM publish flow, generated bindings, uptime
  checks, alert policies, and manual IAP operator runbook.
- **Generated binding coupling.** Website and desktop import generated
  SpacetimeDB bindings from the web app path. That works, but it is an unusual
  cross-app dependency for a monorepo that otherwise prefers shared packages.
- **Subscription policy belongs to us anyway.** The newer docs already design
  OpenAgents-specific subscription scopes: active region, selected entity,
  bounded visible targets, near/far presence, absent-means-unchanged deltas,
  and service-only projection bridges. These are domain policy decisions, not
  generic database features.
- **Effect is now the unifying runtime.** The scene-graph audit argues that
  `Scope`, `Layer`, `SubscriptionRef`, `Stream`, and managed frame clocks are
  the right primitives for retained 3D scenes. The same is true for a live
  world backend: typed services, scoped connections, streams, schedules, queues,
  PubSub, and testable reducers.

The biggest reason to replace SpacetimeDB is not "SpacetimeDB is bad." It is
that the Verse backend is becoming OpenAgents-specific enough that owning the
full contract in Effect/TypeScript may reduce long-term cognitive and
operational split.

## What Cannot Change

The backend implementation can change. These contracts cannot.

### Authority Split

Worker/D1 public projections remain the source of truth for:

- training run truth;
- proof validity;
- accepted work;
- settlement and payout truth;
- receipts;
- product promises;
- Forum/business authority;
- wallet state and payment authorization.

The world backend, whether SpacetimeDB or custom Effect/TypeScript, owns only:

- multiplayer presence;
- region occupancy;
- avatar position and motion hints;
- local chat/bubbles/emotes;
- pylon attention;
- transient interaction state;
- public-safe projections bridged from existing authority refs.

No replacement path may make browser clients create proof, settlement, receipt,
pylon, product-promise, training, or payout truth.

### Public-Safe Rows

The world backend must not store:

- wallet mnemonics or service tokens;
- raw Lightning/Spark/on-chain addresses beyond already-public receipt refs;
- payment hashes, preimages, or private wallet paths;
- raw prompts, private logs, provider payloads, private repo content, shell
  transcripts, customer-private data, or host/device-private metadata.

### Product Honesty

The game docs repeat one rule in many forms: motion is a claim.

Meaningful glows, beams, zaps, bubbles, growth tiers, movement, progress, and
event bursts must be backed by one of:

- a public authority row or source ref;
- a timestamped replay event;
- explicit local user interaction;
- clearly decorative ambience that is not tied to work, payment, proof, or
  network state.

The custom backend must preserve this rule mechanically through schemas,
reducers, source refs, and tests.

### Verse Availability

The Verse stays single-player-first when the world backend is down.

The desktop/web client must still render the Worker/D1 summary and local
movement if the live world connection fails. Multiplayer and reflection icons
are additive. Outages are diagnostics, not fatal UI states.

## What SpacetimeDB Currently Provides For Free

An Effect replacement must intentionally rebuild or replace these features:

| Current SpacetimeDB capability | Replacement obligation |
| --- | --- |
| Tables with typed Rust/WASM schema | Effect Schema row contracts and migrations |
| Reducers as the mutation boundary | Effect command handlers with authorization gates |
| Generated TypeScript client bindings | Shared `packages/world-contract` types and client SDK |
| WebSocket subscribe endpoint | Snapshot + delta stream transport |
| Client-local row cache pattern | Desktop/web world store with idempotent apply |
| Identity-bound reducers | Authenticated actor/session model |
| Service-only reducer enforcement | Service identity and capability checks |
| Public subscribe route with narrow nginx boundary | Narrow public transport routes and deploy guard |
| Scheduled/expiry reducers | Effect `Schedule`/clock-driven expiry workers |
| Row insert/update/delete callbacks | Typed event stream and subscription indexes |

The replacement needs feature parity on the shape that matters, not a clone of
SpacetimeDB internals.

## Target Custom Backend Shape

Name this conceptually as the **Verse World Service**. The concrete app/package
names can be decided later, but the shape should be:

```text
packages/world-contract/
  Effect Schema row types, command types, delta types, subscription plans,
  invariant helpers, test fixtures.

packages/world-client/
  Backend-neutral client facade used by desktop and web:
  connect, subscribe, callCommand, applyDelta, reconnect, diagnostics.

apps/openagents-world-effect/
  Effect/TypeScript service implementing the contract.
  Owns live presence, subscriptions, projection mirror, and expiry.
```

If the final deployment target is the existing OpenAgents Worker surface, this
service can compile into Worker-compatible modules later. If the first
prototype needs fewer moving parts, it can run as a Bun service beside the
current SpacetimeDB deployment. The first decision should be contract parity,
not hosting.

### Core Services

The Effect service should be built from small typed services:

| Service | Responsibility |
| --- | --- |
| `WorldStore` | Durable rows, indexes, idempotent upserts, snapshots. |
| `PresenceStore` | Hot avatar positions, near/far feed state, stale expiry. |
| `WorldAuth` | Actor identity, service capability, browser/user capability. |
| `WorldCommands` | Reducer-compatible command handlers. |
| `WorldSubscriptions` | Query planning, initial snapshots, delta fanout. |
| `WorldClock` | Testable time, TTL, expiry, keepalive policy. |
| `ProjectionBridge` | Public Worker/D1 and Forum projection ingestion. |
| `WorldPubSub` | Region/run/entity-topic fanout, backpressure, diagnostics. |
| `WorldRedaction` | Public-safety assertions for rows and deltas. |
| `WorldDiagnostics` | Connection state, bridge health, row churn, warnings. |

Effect gives these services natural structure:

- `Layer` for runtime composition;
- `Scope` for WebSocket/session lifetimes;
- `Queue`/`PubSub` for deltas;
- `Stream` for subscriptions and bridge input;
- `Schedule` for expiry;
- `Ref`/`SubscriptionRef` for hot region state;
- `Effect Schema` for every external boundary.

## Contract Compatibility

The first custom backend should intentionally speak the current world model,
not a redesigned dream schema.

### Rows To Preserve

Projection rows:

- `training_run`
- `run_entity`
- `world_edge`
- `proof_ref`
- `settlement_ref`
- `world_event`
- `projection_cursor`
- `bridge_health`

Interaction rows:

- `world_region`
- `pylon_station`
- `agent_avatar`
- `avatar_position`
- `avatar_position_near`
- `avatar_position_far`
- `pylon_attention`
- `local_chat_message`
- `chat_bubble`
- `local_emote`
- `agent_intent`

Administration rows:

- `service_identity` or a capability-equivalent service auth table;
- optional `subscription_cursor` / `session` rows if needed for reconnect.

### Commands To Preserve

Service-only commands:

- `upsert_training_run`
- `upsert_run_entity`
- `upsert_world_edge`
- `upsert_proof_ref`
- `upsert_settlement_ref`
- `append_world_event`
- `record_projection_cursor`
- `record_bridge_health`
- `record_bridge_success`
- `record_bridge_failure`
- `upsert_world_region`
- `upsert_pylon_station_from_projection`
- `ensure_pylon_agent_avatar`
- `record_system_world_message`
- `expire_interaction_rows`

Browser/user commands:

- `join_region`
- `leave_region`
- `set_avatar_position`
- `focus_pylon`
- `clear_pylon_focus`
- `send_local_message`
- `send_pylon_message`
- `send_emote`
- `set_agent_intent`

If the custom service renames these internally, the client SDK should still
expose compatibility functions while the migration is in flight.

### Subscription Lifetimes

The replacement must preserve the query lifetime model from the SpacetimeDB
plans:

- **Global lifetime:** feature/config refs, current identity, region catalog.
- **Run lifetime:** canonical run rows, run entities, world edges, recent
  public world events for the selected run.
- **Region lifetime:** stations, positions, chat, emotes, attention, bubbles,
  and intents for the active region.
- **Selected entity lifetime:** proof refs, settlement refs, event history,
  public receipt/proof links.

The desktop must not fall back to subscribing to every avatar or every event
globally.

## Effect Backend Subscription Semantics

SpacetimeDB sends row changes. The custom service should formalize this as a
versioned world delta stream:

```ts
type WorldDelta =
  | { kind: "snapshot"; subscriptionRef: string; rows: readonly WorldRow[]; cursor: string }
  | { kind: "insert"; subscriptionRef: string; row: WorldRow; cursor: string }
  | { kind: "update"; subscriptionRef: string; row: WorldRow; previous?: WorldRow; cursor: string }
  | { kind: "delete"; subscriptionRef: string; table: WorldTable; key: string; cursor: string }
  | { kind: "heartbeat"; subscriptionRef: string; observedAt: string; cursor: string }
  | { kind: "diagnostic"; level: "info" | "warn" | "error"; code: string; message: string }
```

Key rules:

- Initial subscription sends a full snapshot for that query.
- Later deltas are idempotent and cursored.
- Missing fields in sparse deltas mean unchanged, never default-to-empty.
- Reconnect can request "snapshot since cursor" or fall back to a fresh
  snapshot.
- Server controls subscription scopes and rejects unbounded query shapes.
- Row order is stable where the client uses it for rendering.

This keeps the useful SpacetimeDB client-cache behavior while making the
transport contract ours.

## Storage Model

The docs imply two very different storage classes:

1. **Durable projection rows.** These mirror public Worker/D1 truth and must be
   idempotently replayable: training run rows, proof refs, settlement refs,
   world events, forum activity events, projection cursors, bridge health.
2. **Ephemeral interaction rows.** These expire: avatar positions, attention,
   chat bubbles, local emotes, agent intents, local chat messages.

The custom backend should treat these separately:

- durable rows live in a durable store with unique keys and replay-safe
  upserts;
- hot presence lives in memory plus optional short durable checkpoints;
- expiry runs on a testable clock;
- service bridges can rebuild durable projection rows from the public Worker
  endpoints;
- interaction rows can vanish safely after TTL without damaging product truth.

This separation is the easiest way to keep "world backend down" from becoming
"product truth lost."

## Identity And Character Model

The custom backend must carry forward the 2026-06-21 character model:

```text
avatar.identity.<server-authoritative-identity>.char.<sanitized-character-id>
```

The identity segment must be server-authoritative. The character segment may be
client-supplied, but it is sanitized and bounded. This preserves:

- one account controlling multiple visible characters;
- same character id under different accounts never colliding;
- leave/update targeting exactly one character;
- no leakage of local `PYLON_HOME`, hostnames, device IDs, or profile paths.

The Effect contract should make this a pure helper with mirrored server/client
tests, not a string convention hidden in reducer bodies.

## Validation Rules To Preserve

The custom backend must reject or clamp:

- unknown region refs;
- non-finite coordinates;
- positions outside `world_region` bounds;
- impossible jumps;
- position writes faster than the region update interval;
- chat bodies over 280 characters;
- non-plain-text chat bodies;
- repeated chat within the one-message-per-second window;
- pylon attention for non-visible or non-region-local pylons;
- projection rows from non-service actors;
- world events without source refs;
- confirmed payment/zap semantics without receipt-backed evidence.

These should become `Effect Schema` decoders plus command-level tests, not
comments.

## Interest Scoping And Near/Far Feeds

WoC's netcode audit is the strongest argument for owning the backend logic
ourselves. It gives a clear target:

- interest scoping with enter/exit hysteresis;
- distance-tiered update rates;
- absent-means-unchanged deltas;
- settle rows when motion stops;
- client renderer as a mirror, not an authority.

SpacetimeDB gave us tables. The custom backend can make this policy explicit:

```text
local player window
  -> high-resolution avatar_position_near stream
  -> low-resolution avatar_position_far stream
  -> selected/focused avatars forced to high-resolution
  -> settle delta on idle transition
```

The current policy threshold from the multiplayer audit should remain:
single-region feed below roughly 96 avatars / 960 rows per second, split
near/far above that or behind a release gate.

## Bridge Compatibility

The replacement service must ingest the same public projections:

- `GET /api/public/tassadar-run-summary`;
- public forum activity projection once available;
- future proof replay / receipt / product-promise public endpoints only as
  source refs, not authority transfers.

The bridge behavior stays:

- idempotent;
- deterministic event refs;
- service-only;
- source URL and `generatedAt` recorded;
- replaying the same source does not duplicate `world_event`;
- failure records `bridge_health`, not fake data;
- bridge rows remain public-safe.

The current `project-tassadar-summary.mjs` and `project-forum-activity.mjs`
patterns should become contract tests for the custom backend before they become
new code.

## Client Compatibility

Desktop and web should consume a backend-neutral client:

```ts
type WorldBackendKind = "spacetimedb" | "effect"

type VerseWorldClient = {
  readonly backendKind: WorldBackendKind
  connect: () => Effect.Effect<WorldSession, WorldConnectionError>
  subscribe: (plan: WorldSubscriptionPlan) => Stream.Stream<WorldDelta, WorldStreamError>
  callCommand: (command: WorldCommand) => Effect.Effect<WorldCommandReceipt, WorldCommandError>
  disconnect: () => Effect.Effect<void>
}
```

Existing desktop surfaces should not know which backend is live. They should
continue to receive:

- a connection status;
- a region/run snapshot;
- remote avatars;
- pylon stations;
- local chat/bubble rows;
- forum `world_event` rows;
- diagnostics;
- non-fatal outage state.

The fastest migration win is to put this interface in front of the current
SpacetimeDB adapter first. Then the Effect backend has a target to satisfy.

## Deployment Options

This audit does not choose the final host, but the options are:

### Option A: Bun Service On Owned VM

Closest operationally to the current GCP deployment.

Pros:

- fastest prototype;
- direct Bun + Effect runtime;
- simple WebSocket ownership;
- can run beside the SpacetimeDB VM for mirror testing.

Cons:

- still has VM, disk, TLS, process, monitoring, and deploy runbook burden;
- does not consolidate onto the existing `openagents.com` Worker surface;
- must build durable storage and backup discipline.

### Option B: OpenAgents Worker Plus Region Actors

Move the world backend closer to the existing Worker/D1 authority surface.

Pros:

- stronger product topology alignment;
- service bridges and public API can share deployment/auth patterns;
- easier to keep public projection and world projection in one release gate;
- no separate Rust/WASM module publish lane.

Cons:

- WebSocket fanout, region actor lifecycle, storage limits, and backpressure
  need careful design;
- continuous multiplayer presence may want a different scaling shape than
  normal request/response Worker routes;
- must prove local two-client and packaged desktop smoke against the deployed
  runtime.

### Option C: Hybrid

Use a Worker-facing public API and an owned Bun/Effect world process for hot
presence until the Worker/actor design is proven.

Pros:

- reduces migration risk;
- keeps the backend-neutral client honest;
- lets us compare ops and latency before final cutover.

Cons:

- temporarily increases moving pieces;
- needs a clear sunset plan or it becomes a permanent split.

Recommended prototype path: Option C for one milestone, then decide between A
and B based on smoke results, operational friction, and deployment gate
complexity.

## Migration Plan

### Phase 0: Contract Extraction

- Add `packages/world-contract`.
- Encode current rows, commands, deltas, subscription plans, and helper
  functions with Effect Schema.
- Add tests for avatar ref construction, character id sanitization, region
  bounds, command authorization classes, row public-safety assertions, and
  source-ref requirements.
- Wrap the existing SpacetimeDB adapter with the backend-neutral
  `VerseWorldClient` interface.

Acceptance:

- No behavior changes.
- Existing SpacetimeDB desktop tests still pass.
- The contract package can decode current generated binding rows into neutral
  rows.

### Phase 1: Read-Only Effect Mirror

- Build `apps/openagents-world-effect` with durable projection rows and a
  WebSocket/SSE read stream.
- Replay `GET /api/public/tassadar-run-summary` into the mirror.
- Expose run/region/station/avatar snapshots.
- Do not accept browser/user writes yet.

Acceptance:

- Mirror row counts match SpacetimeDB for canonical Tassadar projection.
- Replaying the bridge is idempotent.
- `/tassadar` can render from the mirror in read-only mode behind a flag.
- SpacetimeDB outage semantics are unchanged.

### Phase 2: Interaction Commands

- Implement `join_region`, `leave_region`, `set_avatar_position`,
  `focus_pylon`, local message, pylon message, emote, and intent commands.
- Enforce the same bounds, velocity, cadence, TTL, and chat rules.
- Add expiry schedules and bridge health rows.

Acceptance:

- Existing desktop pose publisher works through the neutral client.
- Two-client smoke passes against the Effect backend.
- Bad writes are suppressed client-side and rejected server-side.
- Public-safe row tests pass.

### Phase 3: Subscription Parity

- Add region-scoped subscriptions.
- Add selected-entity subscriptions.
- Add near/far presence streams.
- Add absent-means-unchanged sparse deltas and settle-on-stop deltas.

Acceptance:

- Remote avatars interpolate smoothly.
- Local self is filtered correctly with multiple characters per account.
- Target candidate mapper sees only visible/nearby station/avatar rows.
- Row churn policy matches the current SpacetimeDB threshold.

### Phase 4: Bridge Parity

- Port forum activity bridge.
- Port activity timeline bridge if still needed.
- Preserve deterministic `event_ref` and source refs.
- Emit world events and optional system messages under service capability only.

Acceptance:

- Automated forum intro can appear as a Verse icon/bubble through the Effect
  backend.
- Icon dereferences to the public forum source.
- Duplicate bridge runs do not duplicate `world_event`.

### Phase 5: Cutover Gate

- Run both backends in parallel for one release window.
- Compare row counts, subscription deltas, two-client smoke, outage behavior,
  and visual diagnostics.
- Switch default `CHAT_WORLD_BACKEND=effect` only after parity.
- Update `INVARIANTS.md`, deployment docs, admin runbooks, and README language
  from "SpacetimeDB world projection" to backend-neutral "Verse world
  projection."
- Freeze SpacetimeDB writes, then decommission the VM only after rollback
  confidence exists.

## Tests And Formal Notes

This is an invariant-bearing replacement if executed. Before cutover:

- Model the reducer authority boundary as a bounded state machine:
  service actor vs browser actor vs agent actor, allowed commands, forbidden
  table mutations.
- Convert any meaningful counterexample into command tests.
- Add property-style tests for idempotent bridge replay.
- Add sparse-delta tests proving missing means unchanged.
- Add expiry tests with a fake clock.
- Add multi-character same-account tests.
- Add redaction tests for world rows, chat rows, deltas, diagnostics, and
  bridge failures.
- Add two-client smoke against both backends through the same neutral client.

The formal/model boundary should be explicit: model checks do not authorize
production behavior; runtime code and tests enforce the policy.

## Risks

### Rebuilding Too Much Database

SpacetimeDB's table/reducer/subscription model is convenient because it is
integrated. A custom backend can accidentally become a database project.

Mitigation: implement only the Verse contract, not a general database. Keep
query shapes enumerated and typed.

### Losing Auth Discipline

The reducer boundary is currently easy to state: service reducers vs user
reducers. A custom service must keep that boundary equally sharp.

Mitigation: commands carry an actor class; service-only command handlers reject
browser sessions before decoding row payloads.

### Subscription Fanout Complexity

Live multiplayer is mostly a fanout and backpressure problem. This can become
harder than the row schema.

Mitigation: start with one run and one region; implement near/far only after
single-region parity; expose row churn diagnostics early.

### Scope Creep Into Product Authority

Once the backend is ours, it will be tempting to move more OpenAgents state into
it.

Mitigation: keep projection rows replayable from public Worker/D1 APIs and
forbid the world service from being the first writer of proof/settlement/truth
state.

### Permanent Dual Backend

A mirror can become a second production backend if no cutover gate exists.

Mitigation: define the cutover and decommission criteria before Phase 1 ships.

## Recommendation

Build the Effect/TypeScript backend as a compatibility mirror, not as a new
world design.

Recommended first issue:

```text
Extract Verse world backend contract into Effect Schema and wrap the existing
SpacetimeDB adapter behind a backend-neutral client.
```

Definition of done:

- `packages/world-contract` owns row, command, delta, subscription, avatar ref,
  region bounds, and public-safety schemas.
- Desktop/web use a `VerseWorldClient` facade while still talking to
  SpacetimeDB by default.
- Existing SpacetimeDB tests and two-client smoke still pass.
- No production behavior changes.

Only after that should we implement the Effect mirror.

The final decision should be evidence-based:

- If the Effect mirror passes the same multiplayer, projection, redaction,
  outage, and bridge idempotency gates with less operational burden, promote it.
- If it cannot match SpacetimeDB's subscription/client-cache behavior without
  rebuilding too much infrastructure, keep SpacetimeDB and still retain the
  backend-neutral contract as useful insulation.

Either outcome is useful. The contract extraction is the no-regret step.

