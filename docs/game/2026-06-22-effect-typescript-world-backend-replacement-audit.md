# Effect/TypeScript World Backend Replacement Audit

Date: 2026-06-22
Status: DECISION — replace the SpacetimeDB world backend outright with an owned
Effect/TypeScript service on Cloudflare infrastructure. Not a phased mirror, not
a "keep both" hedge. Build it, cut over, decommission SpacetimeDB.

> **Owner decision (2026-06-22):** No phased/compatibility-mirror approach. Rip
> out SpacetimeDB. Build our own Verse world backend with TypeScript + Effect
> and, ideally, Cloudflare's infrastructure (Durable Objects, WebSockets, D1).
> The sections below are rewritten to that decision; earlier "guarded yes / keep
> SpacetimeDB until parity" framing is superseded.

## Executive Read

We are replacing the SpacetimeDB world backend with an OpenAgents-owned
Effect/TypeScript service running on Cloudflare. This is the correct call and it
collapses the architecture onto the stack the rest of the product already uses:
Bun/Workers, Effect, Effect Schema, Foldkit, `three-effect`, and the public
Worker/D1 authority surface — instead of a separate Rust/WASM database on a
hand-operated GCP VM behind a generated-binding seam.

The live debugging session captured in the addendum below is the proximate
reason: getting two desktop avatars to see each other required crossing four
stacks (Rust/WASM module, gcloud/IAP VM publish, generated TS bindings, desktop
TS) and the failures were all silent, untyped seams — culminating in a
camelCase-vs-snake_case accessor mismatch that delivered an empty world with no
error. That class of bug does not exist when one team owns one typed contract in
one language end to end.

### The target

- **`packages/world-contract`** — Effect Schema for every row, command, delta,
  subscription plan, the avatar-ref/character helpers, region bounds, and
  public-safety predicates. One source of types for server and clients. No
  codegen, no second naming convention.
- **`apps/openagents-world` (Cloudflare Worker + Durable Objects)** — the Effect
  service. One **Durable Object per region** owns that region's live presence,
  WebSocket fanout (with hibernation), and ephemeral interaction rows; **D1**
  holds the durable, replayable projection rows; the existing `openagents.com`
  Worker patterns provide HTTP/auth/deploy. Region DOs are the natural
  authoritative actor for multiplayer presence + backpressure.
- **`packages/world-client`** — one typed client used by desktop and web:
  `connect`, `subscribe`, `callCommand`, `applyDelta`, `reconnect`,
  `diagnostics`. Desktop/web speak only this; there is no SpacetimeDB adapter to
  keep alive.

### Why Cloudflare specifically

- **Durable Objects are the right multiplayer primitive.** A region is a
  single-writer actor with a small hot state and many subscribers — exactly a
  DO. WebSocket hibernation keeps idle regions cheap; `state.storage` gives
  durable checkpoints; alarms drive TTL/expiry on a testable clock.
- **One deploy surface.** Same Wrangler/Worker/D1 release and auth model as the
  rest of `openagents.com`. No VM, nginx/TLS/certbot, WASM publish lane, IAP
  operator runbook, or generated cross-app bindings.
- **It removes the seams that just cost a day.** Same language, shared types,
  typed errors, and one client — the failure modes in the addendum become
  compile errors or first-class diagnostic events.

This is not "rip out and figure it out." The world model, contract, invariants,
validation rules, subscription lifetimes, and bridge semantics below are all
preserved exactly — we are changing the *implementation and host*, not the
*contract*. SpacetimeDB stays running only long enough to stand the replacement
up and verify it on the same gates, then it is decommissioned.

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

Additional implementation references checked for this revision:

- Cloudflare Durable Objects concepts and rules: DOs are the low-latency
  coordination primitive for stateful Workers; they support WebSockets, the
  WebSocket Hibernation API, alarms, RPC, and SQLite-backed per-object storage.
- Cloudflare Wrangler configuration: Durable Object bindings, D1 bindings, and
  DO migrations belong in the Worker config from the first scaffold commit.
- Cloudflare Workers `waitUntil`: use it only for bounded post-response work;
  send longer or retriable background work through Queues instead.
- `projects/repos/effect-solutions` Effect guidance: schema-first data models,
  branded primitives, `Context.Service` + `Layer` dependency graphs,
  `Schema.TaggedErrorClass` errors, config layers, `@effect/vitest`, test
  clocks, and trace/log spans are the implementation baseline.
- `docs/game/woc/` after re-read: WoC's `IWorld`/`ClientWorld` seam,
  interest-scoped delta snapshots, handshake buffering, seq/ack movement
  receipts, chat moderation, hotbar/icon/keybind/minimap/nameplate systems, and
  pure-logic/test-first discipline should shape the backend cutover and the
  follow-on Verse issues.

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

The Cloudflare Verse world backend owns only:

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

Name this the **Verse World Service**. The concrete app/package names are part
of the decision now, because naming the cutover surfaces early prevents another
half-alive backend from growing beside SpacetimeDB:

```text
packages/world-contract/
  Effect Schema row types, command types, delta types, subscription plans,
  invariant helpers, test fixtures.

packages/world-client/
  Cloudflare Verse World client facade used by desktop and web:
  connect, subscribe, callCommand, applyDelta, reconnect, diagnostics.

apps/openagents-world/
  Cloudflare Worker + RegionDurableObject implementation of the contract.
  Owns live presence, subscriptions, projection bridge, and expiry.
```

There is no Bun sidecar prototype and no "later Cloudflare" step. The first
implementation target is Worker-compatible Effect code, one region Durable
Object at a time, with D1 and DO migrations present in the first service
scaffold.

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

### Effect Implementation Discipline

Every issue below should follow the same Effect rules. These are not polish;
they are the reason to replace the old stack.

- External inputs are decoded through `Effect Schema` at ingress and encoded at
  egress. No route, WebSocket message, D1 row, DO checkpoint, or bridge payload
  crosses a boundary as unchecked `unknown`.
- Domain primitives are branded: region refs, avatar refs, character ids,
  session refs, cursors, timestamps, row versions, distances, update intervals,
  and source refs. Raw strings are a boundary smell.
- Services are `Context.Service` classes with unique tags, implemented by
  `Layer`s. Service methods return `Effect` values whose dependencies have been
  supplied by the layer, not hidden globals.
- Expected failures are `Schema.TaggedErrorClass` values: validation,
  unauthorized actor, forbidden command, stale cursor, bridge failure,
  oversized message, rate limit, and storage conflict. Defects are reserved for
  invariant violations and runtime bugs.
- Cloudflare bindings are wrapped once: `WorkerEnv`, `WorkerRequestContext`,
  `WorldConfig`, `WorldD1`, `RegionDurableObjectNamespace`,
  `WorldWaitUntil`, and any Queue producers. Business logic depends on those
  typed services, not raw `env` and `ctx` plumbing.
- WebSocket/session lifecycle is scoped. The DO hibernation callbacks rehydrate
  session metadata from WebSocket attachments/checkpoints, then run typed
  handlers; no correctness-critical state lives only in module globals.
- Region is the Durable Object coordination atom. Do not build a global world
  singleton; per-region DOs own their sockets, hot presence, checkpoints,
  interest indexes, alarms, and local command sequencing.
- Deltas use `Stream`/`Queue`/`PubSub` with bounded buffers and explicit
  backpressure policy. Fire-and-forget is allowed only through a typed
  `waitUntil` service for bounded work; anything retriable or longer-running
  goes through a Queue-backed service.
- Time is a dependency. TTL, cadence, impossible-jump windows, expiry alarms,
  and bridge retry schedules use `Clock`/`Schedule`, with fake-clock tests.
- Storage is behind services. Prefer `@effect/sql-d1` for D1 projection rows and
  an Effect wrapper around DO SQLite storage for per-region checkpoints and
  migrations. SQL errors become tagged storage errors with redacted causes.
- Config is a layer. Secrets, origin allowlists, update intervals, TTLs, bridge
  endpoints, and feature gates are read through a typed config service with
  redacted secret values and test layers.
- Tests use `@effect/vitest`/Effect-aware tests where possible, with test
  layers for auth, storage, bridge sources, clocks, and WebSocket transports.
  Unit tests do not need real Cloudflare; smoke tests prove the deployed Worker.
- Observability is part of the contract: spans/logs name the service method or
  command, include public-safe refs, redact private values, and surface typed
  diagnostics to clients instead of silent empty worlds.

## Contract To Preserve, Not Backend Compatibility

The replacement intentionally speaks the current world model, not a redesigned
dream schema. That does **not** mean the new client preserves a SpacetimeDB
adapter or dual-backend mode. It means the public row, command, subscription,
and projection contracts below remain recognizable so desktop/web behavior can
move quickly.

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
expose the contract names above. The compatibility target is the world contract,
not the old backend implementation.

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

## Client Cutover Contract

Desktop and web should consume a single Cloudflare Verse client:

```ts
type VerseWorldClient = {
  readonly backendKind: "cloudflare-world"
  connect: () => Effect.Effect<WorldSession, WorldConnectionError>
  subscribe: (plan: WorldSubscriptionPlan) => Stream.Stream<WorldDelta, WorldStreamError>
  callCommand: (command: WorldCommand) => Effect.Effect<WorldCommandReceipt, WorldCommandError>
  disconnect: () => Effect.Effect<void>
}
```

Existing desktop surfaces should not know about transport or Cloudflare binding
details. They should continue to receive:

- a connection status;
- a region/run snapshot;
- remote avatars;
- pylon stations;
- local chat/bubble rows;
- forum `world_event` rows;
- diagnostics;
- non-fatal outage state.

The fastest migration win is to make this package compile against the new
`packages/world-contract` and point it directly at `apps/openagents-world`.
There is no `WorldBackendKind = "spacetimedb"` path, no `CHAT_WORLD_BACKEND`
flag, and no desktop setting that resurrects the old adapter. If a short cutover
diff needs to read SpacetimeDB row counts, that is a one-off smoke helper, not
production client code.

## Deployment: Cloudflare (decided)

The host is decided: **Cloudflare**, in the same account/Wrangler surface as
`openagents.com`. No GCP VM, no owned-VM Bun process, no hybrid.

```text
apps/openagents-world/                 (new Cloudflare Worker)
  worker (Effect)                      HTTP: connect handshake, auth, health,
                                       bridge ingest, admin; routes WS upgrades
                                       to the right region DO.
  RegionDurableObject (one per region) the authoritative live actor:
    - hibernatable WebSocket fanout to that region's subscribers
    - hot presence: avatar positions, near/far feed state, attention, bubbles
    - command handlers (join/leave/move/focus/chat/emote/intent) with auth +
      bounds/velocity/cadence validation
    - alarms drive TTL/expiry on a testable clock
    - durable checkpoints in DO storage; durable projection rows in D1
  D1                                   durable, replayable projection rows
                                       (training runs, run entities, edges,
                                       proof/settlement refs, world events,
                                       projection cursors, bridge health).
  ProjectionBridge                     service-only ingest of public Worker/D1
                                       + forum projections into world_event etc.
```

Why this shape:

- **Region = Durable Object.** A region is a single-writer actor with small hot
  state and many subscribers — the canonical DO use case. WebSocket hibernation
  keeps idle regions near-zero cost; `state.storage` gives durable checkpoints;
  DO alarms run expiry/keepalive deterministically.
- **D1 for durable rows; DO memory for ephemeral rows.** This is the
  durable-vs-ephemeral split this audit already requires (see "Storage Model").
  Interaction rows can vanish on TTL without touching product truth; projection
  rows are replayable from public Worker/D1 endpoints.
- **One deploy + auth surface.** Same Wrangler/Worker/D1/secrets model and
  release gate as `openagents.com`. The world API and the public projection API
  ship together.
- **WebSocket fanout is the real work, and DOs are built for it.** Per-region
  fanout, backpressure, and reconnect live inside one object with a clear
  lifecycle, instead of being spread across a VM process + nginx + SDK cache.

Scaling note: start single-run, single-region (one DO). Near/far feeds and
multi-region DO routing come on naturally because each region is already its own
object; the threshold from the multiplayer audit (single-region feed below
~96 avatars / ~960 rows/sec, split near/far above that) maps directly onto
per-DO fanout policy.

## Issue List: Fast Cloudflare Cutover (no backward compat)

This is an issue sequence, not a "run both backends as production" hedge. There
is no `CHAT_WORLD_BACKEND` dual adapter, no `WorldBackendKind = "spacetimedb"`,
and no perpetual compatibility mirror. Each issue either creates the Cloudflare
replacement or deletes old surface area. The only use of the old backend during
the cut is a one-shot read-only smoke helper that compares row counts and then
dies with the old code.

### P0: Open The Ripout Tracker

Create the tracking issue that names the decision: Cloudflare Worker + Durable
Objects + D1 replaces SpacetimeDB, with no backward compatibility. Acceptance:
the issue links this audit, lists the child issues below, names the decommission
gate, and states that SpacetimeDB work after this point is bug-fix-only until
deleted.

### P1: Extract `packages/world-contract`

Status: implemented in issue #5960.

Add the shared Effect contract package first. Acceptance:

- Effect Schema classes for every row, command, receipt, delta, subscription
  plan, diagnostic, bridge payload, and error envelope.
- Branded primitives for all refs/cursors/timestamps/bounded quantities.
- Pure helpers for avatar refs, character-id sanitization, region bounds,
  public-safety predicates, row keys, and deterministic `world_event` refs.
- WoC-style read-only `WorldReadModel` / `ClientWorld` projection schemas that
  render/HUD/minimap/nameplate code consume without knowing the transport.
- `WorldSubscriptionPlan` includes interest radius/drop-radius hysteresis,
  selected-target promotion, near/far tier policy, and cursor resume fields.
- `WorldCommandEnvelope` carries an optional client `seq`; command receipts echo
  the accepted/applied/rejected sequence for latency and dropped-command
  diagnostics.
- Tagged errors for validation, auth, redaction, command, storage, cursor, and
  bridge failures.
- Unit tests for avatar refs, character sanitization, row redaction,
  source-ref requirements, region bounds, command actor classes, sparse delta
  semantics, interest-plan validation, command sequence receipts, and JSON
  encode/decode.

Do this before writing the Worker. It gives every later issue a compile-time
contract and prevents Cloudflare code from inventing ad hoc shapes.

### P2: Scaffold `apps/openagents-world`

Status: implemented in issue #5961.

Create the Cloudflare Worker app with the production host shape from day one.
Acceptance:

- `wrangler.jsonc`/package scripts define the Worker, a
  `RegionDurableObject`, DO SQLite migration entries, D1 binding, Queue binding
  if bridge retry work is needed, and environment-specific config.
- The Worker exposes health, version, connect handshake, WebSocket upgrade, and
  service-only bridge ingest routes.
- The DO uses the hibernatable WebSocket API, attaches typed session metadata,
  and returns a typed diagnostic when a non-WebSocket request hits the socket
  route.
- The Worker routes each region to its own Durable Object. No global singleton
  sits in front of every region.
- A bounded handshake buffer stores frames that arrive while auth/session
  hydration is still attaching the live handler, then replays or rejects them
  with typed diagnostics.
- D1 migrations create the durable projection tables and indexes.
- DO SQLite storage/migration scaffolding exists for per-region checkpoints,
  cursors, and hibernation-safe session metadata.
- DO constructor initialization uses Cloudflare-safe migration discipline
  (`blockConcurrencyWhile`/tracked schema table or equivalent), not lazy
  request-time schema guesses.
- Effect runtime/layers wrap `env`, `ctx`, D1, DO namespace, queues, config,
  waitUntil, logging, and request context.

This issue should deploy a boring empty service that can accept a connection and
say "zero rows" loudly and typed. No game behavior yet.

Implementation note: the first scaffold lives in `apps/openagents-world` with
Cloudflare Worker routes for `/health`, `/version`, `/connect`,
`/regions/:regionRef/socket`, and `/bridge/ingest`; `RegionDurableObject`
routes by region ref, uses hibernatable WebSockets, persists session metadata in
DO-local SQLite, and sends a typed zero-row `WorldDelta` snapshot on connect.
The D1 projection migration is committed under `apps/openagents-world/migrations`
and the Wrangler config declares the DO, D1, Queue, environment vars, and
`new_sqlite_classes` migration from the first app commit. Placeholder D1 IDs
must be replaced during Cloudflare resource provisioning, but the binding names
are now fixed.

### P3: Implement Region DO Session + Snapshot/Delta Transport

Status: implemented in issue #5962.

Make the region object the authoritative live actor. Acceptance:

- One DO instance per region name/ref, routed by the Worker.
- `WorldDelta` snapshot/update/delete/heartbeat/diagnostic frames encoded from
  `packages/world-contract`.
- WoC-style `ClientWorld` snapshots are server-owned read models: render/HUD
  code sees a coherent projection, not backend row tables.
- Reconnect accepts a cursor and either resumes or returns a fresh snapshot with
  a typed stale-cursor diagnostic.
- Deltas are idempotent, cursored, ordered per subscription, and sparse fields
  mean unchanged.
- Initial sight sends a full record; subsequent deltas can send lite/dynamic
  fields; leaving interest prunes the local mirror so re-entry sends full again.
- Per-entity serialization/wire-cache avoids re-encoding the same entity for
  every viewer in the same tick/window.
- Session close/error paths emit public-safe diagnostics and release scoped
  resources.
- Backpressure policy is explicit: bounded queues, disconnect-or-downgrade
  behavior, and row-churn metrics.

This is the first smokeable issue: two local clients can connect to the same DO
and receive typed heartbeat/snapshot frames.

Implementation note: `apps/openagents-world` now emits typed transport
envelopes around `WorldDelta`, `WorldReadModel`, and `WorldDiagnostic` values.
Region reconnect accepts cursor query params and either resumes with a heartbeat
or returns a public-safe stale-cursor diagnostic plus a fresh snapshot. Pure
transport tests cover schema encode/decode, sparse absent-means-unchanged sight
planning, prune/re-entry full-record behavior, per-entity wire-cache reuse, and
bounded backpressure diagnostics; the DO uses the same helpers for live socket
open/message paths.

### P4: Implement User Commands And Hot Presence

Status: implemented in issue #5963.

Add the browser/user command path in the DO. Acceptance:

- `join_region`, `leave_region`, `set_avatar_position`, `focus_pylon`,
  `clear_pylon_focus`, `send_local_message`, `send_pylon_message`,
  `send_emote`, and `set_agent_intent` are implemented as Effect command
  handlers.
- Pose/intent commands include client `seq` where available, and typed receipts
  echo accepted/applied/rejected sequences so clients can show latency and drop
  diagnostics.
- Bounds, velocity, cadence, chat length/plain-text/rate-limit, pylon
  visibility, focus, and TTL rules are enforced through schema decode plus
  command tests.
- Hot rows live in DO memory with durable checkpoints only where reconnect
  needs them.
- No browser/user command can write projection authority rows.
- Typed receipts tell the client what changed, what was rejected, and why.

After P4, desktop avatars should be able to join, leave, move, chat, emote, and
focus pylons against the Cloudflare service locally.

Implementation note: browser WebSocket frames are decoded as
`WorldCommandEnvelope` values and applied through Effect command handlers over
DO-owned hot state. The handler set covers join/leave, avatar pose, pylon focus,
local/pylon chat, emotes, and agent intent. Receipts are always schema-encoded
and reject service-only writes, stale/duplicate sequences, out-of-bounds or
too-fast movement, cadence violations, and non-plain text. Hot rows live in DO
memory while the transport cursor clock remains in DO SQLite for reconnect.

### P5: Add Alarms, Expiry, And Deterministic Time

Status: implemented in issue #5964.

Move interaction expiry out of wishful thinking and into the DO runtime.
Acceptance:

- DO alarms drive stale presence, chat bubble, emote, attention, and intent
  expiry.
- Expiry uses `Clock`/`Schedule` behind services so fake-clock tests can advance
  time deterministically.
- Expiry deltas are emitted exactly once per cursor window and are safe after
  hibernation/restart.
- Storage checkpoints are pruned without touching durable projection truth.

This issue prevents stale avatars and bubbles from becoming fake product state.

Implementation note: P5 adds a `WorldClock` Effect service plus pure expiry
planning for fake-clock tests, DO-local `region_hot_expiry_refs` SQLite
metadata, Cloudflare DO alarm scheduling, exactly-once delete deltas per expiry
cursor window, and storage-prune helpers that remove hot checkpoints without
touching service-authoritative projection rows. The DO persists TTL metadata for
presence, chat, emote, focus, and intent rows, schedules a one-shot alarm for
the next deadline, broadcasts expiry deltas, then reschedules only if more work
remains.

### P6: Implement Subscription Scopes And Near/Far Feeds

Port the WoC subscription policy directly into the owned backend. Acceptance:

- Global, run, region, and selected-entity subscription plans are typed and
  server-controlled.
- The service rejects unbounded/global avatar/event queries.
- Region feed starts as one stream below the current audit threshold, then
  splits into near/far feeds with hysteresis, forced high-resolution selected
  targets, distance-tiered update rates, first-sight full records, lite dynamic
  deltas, interest-leave pruning, and settle-on-stop deltas.
- Interest scopes use separate enter/drop radii so boundary movement does not
  churn create/delete frames.
- Desktop/web stores apply deltas idempotently and never interpret missing
  sparse fields as empty/default values.
- Tests cover near/far enter/exit, selected-target promotion, stale cursor
  fallback, and absent-means-unchanged behavior.

This is the issue that makes the replacement better than the old table stream,
because interest policy is now explicit domain logic.

Implementation note: P6 adds `apps/openagents-world/src/subscriptions.ts` as the
Effect-backed server-side interest policy module. `/connect` returns an approved
`WorldSubscriptionPlan`; Region DO WebSocket attachments persist that plan plus
per-session sight/tier state for hibernation. Client query params may request a
scope, center, selected target, and cursor, but the service normalizes them and
rejects unbounded global avatar/event streams.

The policy planner keeps the feed as `single_region` below the audit threshold
and chooses `split_near_far` once avatar count or estimated row churn crosses
the documented limit. It implements separate enter/drop hysteresis, selected
target high-resolution promotion, explicit near/far update rates, first-sight
full records, lite continued updates, interest-leave pruning, re-entry full
records, settle-on-stop patches, and a sparse read-model apply helper that
preserves absent-means-unchanged semantics. P8/P9/P10 should reuse this module
when durable projection rows, the official `packages/world-client`, and richer
socket fanout land; they should not create a second interest-policy path.

### P7: Add World Chat Moderation And Abuse Controls

Adapt WoC's moderation posture before local chat/forum-reflection bubbles become
part of the production world stream. Acceptance:

- `WorldModeration` service runs before local chat, pylon chat, forum-reflection
  bubbles, and any user-authored diagnostic text are emitted as `WorldDelta`s.
- Soft-list masking is a client/user preference; server hard-list enforcement is
  whole-token and confusable-folded.
- The hard list ships empty in the open repo and is seeded privately through
  typed config/secrets/admin tooling. No slur list enters Git.
- Strikes escalate warning -> timed mutes, with public-safe reason codes and
  no raw private message bodies in diagnostics.
- Per-account and per-session command throttles are separate from any IP/edge
  throttle so distributed abuse does not bypass account-level policy.
- Tests cover false-positive avoidance (`class`, `despicable`), confusable
  matching, empty hard-list behavior, mute windows, and redaction.

Implementation note: P7 adds `apps/openagents-world/src/moderation.ts` as the
`WorldModeration` service and threads it through browser local/pylon chat before
any `local_chat_message` row can be emitted. The open Worker config ships empty
`OPENAGENTS_WORLD_MODERATION_HARD_TOKENS_JSON` and
`OPENAGENTS_WORLD_MODERATION_SOFT_TOKENS_JSON` arrays; private deployments can
seed JSON string arrays through config/secrets without committing a slur list.

The service performs whole-token hard-list enforcement after NFKD/confusable
folding, keeps soft-list masking as a client/user preference, avoids substring
false positives such as `class` and `despicable`, records strike/mute state in
the Region DO hot state, and emits only public-safe reason codes. It exposes
the same moderation gate for future forum-reflection bubbles and redacts
user-authored diagnostic text before public diagnostics. Chat throttles now
track per-account and per-session cadence separately from any later IP/edge
throttle.

### P8: Add Service Commands And Projection Bridge

Bring durable public projection rows into the Cloudflare service. Acceptance:

- Service-only commands are implemented for training runs, run entities, world
  edges, proof refs, settlement refs, world events, projection cursors, bridge
  health, regions, pylon stations, system messages, and interaction expiry.
- The bridge ingests `GET /api/public/tassadar-run-summary`, forum activity
  when available, and future proof/receipt/product-promise refs only as public
  source refs.
- Replay is idempotent: same source rows produce same keys and no duplicate
  `world_event`.
- Bridge failure records `bridge_health` and client diagnostics; it never
  fabricates data.
- Retriable bridge work uses Queue-backed services rather than long
  `waitUntil` chains.

After P8, the world service can rebuild its durable projection rows from public
authority and does not need SpacetimeDB as a projection source.

Implementation note: P8 adds `apps/openagents-world/src/bridge.ts`, wires
`POST /bridge/ingest` into D1 persistence, and opens the service-only command
lane inside `apps/openagents-world/src/commands.ts`.

The bridge now decodes `WorldBridgePayload` with Effect Schema, runs
`assertWorldPublicSafety` before persistence, dedupes rows by
`row.kind + worldRowKey(row)`, writes `world_projection_rows`, advances
`world_projection_cursors` when a cursor is present, and records
`world_bridge_ingest_log` for replay auditing. Successful ingest appends public
`bridge_health=current` and optional `projection_cursor` rows; failed ingest
writes `bridge_health=failed` and returns a public diagnostic without inventing
run/proof/settlement state. Queue integration stays bounded: valid requests
enqueue only a compact `bridge_ingest_requested` marker for retriable follow-up
work instead of chaining long `waitUntil` jobs.

Service commands can now upsert training runs, run entities, world edges,
proof refs, settlement refs, events, regions, pylons, bridge health, projection
cursors, system messages, and interaction-expiry deletes. Browser and agent
actors still fail the shared actor gate before service row payloads decode, and
unsafe/private service rows are rejected with redacted diagnostics.

### P9: Build `packages/world-client` As The Only Client

Add the client package that desktop and web will import. Acceptance:

- The client speaks only the Cloudflare Verse World protocol.
- APIs are `connect`, `subscribe`, `callCommand`, `applyDelta`, `reconnect`,
  `disconnect`, and `diagnostics`.
- It exposes the WoC-style read-only `WorldReadModel` / `ClientWorld` seam used
  by render, HUD, minimap, and nameplate code.
- `applyDelta` preserves absent-means-unchanged semantics, prunes interest
  exits, applies settle deltas, and retains selected-target promotion state.
- Command receipts expose client sequence ack state for movement/input
  diagnostics.
- Errors are Effect typed errors, not `console.warn` plus empty state.
- Reconnect/resubscribe is tested with fake transports and stale cursors.
- The package has no import from generated SpacetimeDB bindings and no backend
  kind union.

This issue is where the previous casing bug becomes structurally impossible.

Implementation note: P9 adds `packages/world-client` as
`@openagentsinc/world-client`, registers it in the root `test` and `typecheck`
scripts, and keeps the package transport-neutral. It exposes `connect`,
`subscribe`, `callCommand`, `applyDelta`, `reconnect`, `disconnect`,
`diagnostics`, `readModel`, and `state` over a typed Effect transport interface
that speaks only the Cloudflare Verse World protocol.

The package owns the WoC-style `ClientWorld` mirror and `applyDeltaToReadModel`.
Absent rows mean unchanged, `deletedRefs` prune every read-model table and
selected-target state, settle patches update motion without dropping position
rows, diagnostics flow into the read model, and command receipts project
`acceptedSeq`/`appliedSeq`/`rejectedSeq` for movement/input debugging. Tests use
fake transports for reconnect, resubscribe, stale cursor diagnostics, command
acks, interest pruning, selected-target retention, and settle deltas. There is
no backend-kind union and no generated SpacetimeDB import.

### P10: Point Desktop And Web At Cloudflare

Cut consumers to the new client as soon as P9 can support the current UI.
Acceptance:

- Desktop/web imports only `packages/world-client` and
  `packages/world-contract` for Verse world networking.
- Connection status, region snapshots, remote avatars, pylon stations, local
  chat/bubbles, forum/world events, diagnostics, and outage-to-single-player
  behavior are preserved.
- Character id is passed as a typed connect/session field; no renderer global
  injection workaround is required for world identity.
- Two desktop instances and one web client see each other through the
  Cloudflare service in smoke.
- No production flag or setting can switch the app back to SpacetimeDB.

This is the product cut. Do not wait for decommission work to begin before the
new backend is user-visible.

Implementation note: P10 moves the active desktop and web Verse networking
paths onto the Cloudflare world protocol. `packages/world-client` now includes a
browser/WebSocket transport that performs the `/connect` handshake, appends the
typed session fields (`actorRef`, `actorClass`, `characterId`, cursor), hydrates
the Region Durable Object socket, decodes snapshot/delta/diagnostic frames, and
resolves command receipts back into the client read model.

Autopilot Desktop now defaults `subscribeCloudflareWorld` to the Cloudflare
transport and `@openagentsinc/world-contract` command envelopes. Production code
no longer imports generated web bindings. The desktop adapter mirrors
`WorldReadModel` rows into the existing `ChatWorldMultiplayerProjection`,
preserving disconnected single-player fallbacks, pylon stations, remote avatars,
local chat rows, and focus intents.

The web Tassadar world subscription module also uses the shared Cloudflare
transport and local structural public-row adapters instead of generated
bindings. Focus, chat, pylon message, avatar pose, join, and leave commands are
sent through typed browser command envelopes. P11 removed the remaining
generated binding files, manifest dependencies, old comments, and adapter names;
there is no runtime rollback switch.

### P11: Delete SpacetimeDB Codepaths

Remove the old implementation immediately after P10 smoke passes. Acceptance:

- Delete deleted legacy world module.
- Delete generated SpacetimeDB TypeScript bindings and cross-app imports.
- Delete SpacetimeDB launch/publish scripts, nginx/TLS/certbot instructions,
  GCP/IAP VM runbooks, smoke profiles, and old env vars.
- Remove SpacetimeDB dependencies from package manifests and CI/deploy scripts.
- Remove camel/snake casing shims and any old adapter tests.
- Repository search for `spacetimedb`, `SpacetimeDB`, and old world VM hostnames
  returns only historical docs/audit references or explicit changelog notes.

This issue has no compatibility carve-out. If something still needs the old
backend, it blocks P11 and must be ported, not grandfathered.

Implementation note: P11 deletes the legacy world module directory, generated
web TypeScript bindings, bridge projection scripts, VM/admin runbooks, old
world backend dependencies, old generated-binding import paths, and active
desktop/web symbol names that implied backend compatibility. The active clients
now use Cloudflare-world filenames and exported names:
`chat-world-cloudflare.ts`, `subscribeCloudflareWorld`, and
`tassadarCloudflareWorld.ts`.

Validation includes repository search over active code for `spacetimedb`,
`spacetime.openagents.com`, generated binding paths, and the deleted module
path; the only remaining mentions are historical docs/audit references or
repo-agent guidance that forbids resurrecting the deleted backend. The
visibility freshness smoke no longer executes a local bridge-plan script from
the deleted module.

### P12: Update Invariants, Docs, And Operator Runbooks

Make the policy ledger match reality. Acceptance:

- Update `INVARIANTS.md`, `AGENTS.md`/`CLAUDE.md` where relevant, deployment
  docs, README pointers, and game docs from "SpacetimeDB world projection" to
  "Cloudflare Verse World Service".
- Record the invariant boundary: Worker/D1 public product authority stays
  authoritative; the world service owns only public-safe presence, interaction,
  and projection rows.
- Add the formal/model note for actor command authority and convert meaningful
  counterexamples into tests.
- Document the Cloudflare deploy, D1 migration, DO migration, Queue, alarm, and
  smoke procedure.

Because this is invariant-bearing work, this issue is part of the cutover, not
post-launch cleanup.

Implementation note: P12 renames the root invariant from a generic world
projection to the Cloudflare Verse World Service and records the explicit
boundary: Worker/D1 product surfaces stay authoritative for public training
truth, product promises, proof/receipt claims, settlement/payout projection,
Forum/product state, and all private/customer/provider material; the world
service owns only public-safe presence, interaction, diagnostics, fanout, and
projection rows derived from public refs.

The active operator docs now route world deployment through
`apps/openagents-world/README.md` and `docs/DEPLOYMENT.md`: preflight world
contract/client/service tests, Wrangler D1 migrations for `openagents-world`
or `openagents-world-staging`, Wrangler Durable Object class migrations in
`wrangler.jsonc`, `WORLD_BRIDGE_QUEUE` retry markers, DO alarm expiry tests, and
two-client live smoke. The visibility operations runbook now describes
`POST /bridge/ingest` on the Cloudflare service instead of the deleted VM bridge.

The formal/model note is
`2026-06-22-cloudflare-world-actor-command-authority-model.md`. The associated
counterexamples are command tests: browser, agent, and operator actors cannot
write service projection rows; service actors cannot send browser interaction
commands; unsafe service projection rows are rejected without payload echo.

### P13: Production Release Gate And VM Decommission

Finish by proving the new path and shutting the old one off. Acceptance:

- `check:deploy` passes.
- Unit/property tests pass for contract, command auth, redaction, sparse deltas,
  fake-clock expiry, idempotent bridge replay, and multi-character same-account
  refs.
- Two-client desktop smoke and web smoke pass against deployed Cloudflare.
- A one-shot read-only diff against the old SpacetimeDB path confirms expected
  row-count parity for the cutover sample, then the helper is deleted.
- Cloudflare logs/diagnostics show connection, command, bridge, and expiry
  behavior with public-safe refs.
- GCP VM, disk, DNS, nginx/TLS/certbot, alerting, and IAP operator surface are
  decommissioned or explicitly archived as historical infrastructure.

Definition of done for the whole effort: two desktop instances and a web client
see each other through the Cloudflare service, the projection bridge is
idempotent, public-safety/redaction tests pass, outages degrade to
single-player, formal/model notes are updated where policy changed, and **no
SpacetimeDB code, VM, generated binding, deploy lane, or operational runbook
remains active in the repo.**

Implementation note: P13 shipped the production Cloudflare Worker at
`https://openagents-world.openagents.workers.dev` with version
`76bd5d07-765c-4ea3-881e-8e9e2b7a5495`, provisioned production/staging D1
databases and bridge queues, and archived the old GCE SpacetimeDB VM as
historical infrastructure. The release smoke found two real cutover defects and
fixed them before close: clients were still deriving the old `.main` starter
region instead of `.street`, and Region DO command deltas were only sent to the
sender instead of being fanned out to all connected sessions. The durable
release receipt is
`docs/game/2026-06-22-cloudflare-world-production-release-receipt.md`.

## Post-Cutover WoC Adaptation Lanes

These are not blockers for deleting SpacetimeDB, but they are the next issues to
open once the Cloudflare world service is the only backend.

### W1: `three-effect` Procedural Icons

Re-author WoC's procedural canvas icon recipe system for OpenAgents taxonomy:
Pylon, agent, run, proof, receipt, settlement, training, chat, zap, inspect, and
focus. Acceptance: keyword fallback for unknown ids, deterministic output,
cache tests, and a HUD/3D texture consumer.

Implementation note: W1 shipped the shared procedural taxonomy primitive in
`@openagentsinc/three-effect` at commit `525d665`. The primitive exposes
deterministic recipe selection, keyword fallback for unknown ids, a draw-plan
cache, a canvas renderer for HUD/texture consumers, and tests that cover the
OpenAgents taxonomy without external image assets. The desktop Verse world
projection now consumes the shared primitive by attaching icon recipes to pylon
station and payment endpoint scene entities; the app pins the exact
`three-effect` hash so web and desktop consume the same shared visual contract.

### W2: `three-effect` Camera Follow, Collision, And Pointer Pick

Port the pure math patterns for auto-settle-behind, camera occlusion easing, and
click-vs-drag disambiguation. Acceptance: unit tests for follow/collision/pick
math plus desktop smoke proving mouselook and click-select do not fight.

Implementation note: W2 shipped the shared camera/input primitives in
`@openagentsinc/three-effect` at commit `b2bb8fa`. The primitive layer now
exports capped auto-settle-behind yaw math, occlusion distance easing with fast
pull-in and slower release, and a click-vs-drag gesture classifier. The mounted
training-run visualization uses the classifier before selecting/locking on
click, while desktop pins the same `three-effect` hash and carries a launch
checklist regression proving a mouselook drag is not treated as click-select.

### W3: Minimap, Compass, Coords, And Subzone

Build a minimap/readout layer from the same `WorldReadModel` the 3D scene uses.
Acceptance: Pylons, run core, assignment markers, and remote avatars share the
same source as the scene; subzone/region labels use hysteresis; minimap deltas
never drift from nameplates or 3D entities.

Implementation note: W3 shipped `projectWorldMinimapReadout` in
`@openagentsinc/world-client`. The pure projection consumes only
`WorldReadModel` and emits pylon, avatar, run-core, and assignment minimap
markers with world positions, minimap coordinates, compass coordinates, and a
region/subzone label. Subzone selection keeps a configurable hysteresis band so
labels do not flicker at centerline boundaries. Desktop tests compare minimap
pylon/avatar positions against the existing 3D scene projection built from the
same read model fixture, proving the layer does not subscribe to backend
transport or drift from scene data.

### W4: Nameplate And Label Projection Primitive

Promote a `three-effect` label/nameplate primitive using WoC's world-to-screen
anchor discipline. Acceptance: Pylon/agent/run labels include state/status bars,
are pooled/reused, and degrade without overlapping the core HUD.

Implementation note: W4 shipped shared Verse nameplate projection primitives in
`@openagentsinc/three-effect` at commit `b65d0cf`. The primitive projects
Pylon, agent, and run labels from world anchors into screen coordinates, derives
bounded status-bar tones for online/working/offline/blocked/pending states, and
degrades labels when they are offscreen, behind the camera, or overlapping a
core HUD exclusion rect. It also exposes a small nameplate pool reconciler so
stable ids are reused instead of constantly recreated. Desktop pins the exact
`three-effect` hash and carries a projection/pooling regression over
pylon/agent/run fixtures.

### W5: Desktop HUD Hotbar, Chat Channels, And Context Menu

Re-author WoC's pure hotbar, chat channel/timestamp/profanity models, and
context-menu action model for Verse actions. Acceptance: slots dedupe/sync,
channel prefixes map to local/run/global/forum contexts, right-click Pylon/avatar
actions are pure/tested, and Foldkit is only the renderer.

Implementation note: W5 shipped a desktop pure `verse-hud-action-model` module.
It dedupes and slots model-owned hotbar actions, routes `/local`, `/run`,
`/global`, and `/forum` chat prefixes, formats timestamps without renderer
state, composes display state from backend `WorldModeration` output without
app-local word lists, and builds pure context actions for Pylon and avatar
targets. The implementation is renderer-agnostic; Foldkit remains the view
consumer rather than the owner of HUD behavior.

### W6: Run-Step Progress, Agent Portrait Chips, And Perf Overlay

Adapt WoC's pure progress/portrait/perf-overlay models. Acceptance: run-step
progress bars map assignment -> trace -> replay -> verdict -> settle; portrait
chips use HiDPI/overscan math; WebGL frame-time/draw-call diagnostics are
available in development and smoke artifacts.

Implementation note: W6 shipped a desktop pure
`verse-progress-diagnostics-model` module. It maps receipt-backed run facts
through assignment, trace, replay, verdict, and settlement without inventing
later progress; projects circular avatar/agent portrait chips with clamped DPR
and overscan math that stays stable under viewport changes; and produces a
public-safe WebGL diagnostics artifact with frame-time, FPS, draw-call, and
entity counts for development/smoke paths. Production mode returns no noisy
overlay or artifact by default.

### W7: Optional Accessibility Movement And Touch Controls

Keep click-to-move, joystick, pinch, long-press-vs-tap, and double-tap-recenter
as later accessibility/touch issues. Acceptance is future-surface dependent; do
not pull these into the backend cutover.

Implementation note: W7 is intentionally a planning closure, not a backend
cutover task. Before any optional accessibility/touch control ships, create
separate implementation issues with target-surface-specific specs:

- click-to-move: target selection, path authority, turning cone, latency-aware
  stop distance, stuck reroute, and cancellation rules;
- touch joystick: deadzone, clamped origin, eight-way/vector output, camera
  stick rates, settings, and haptic affordances;
- pinch: two-finger zoom delta, min/max camera distance, and conflict handling
  with scroll/wheel/mouselook;
- tap vs long press: duration thresholds, movement tolerance, chat peek,
  composer focus, and context-menu interaction priority;
- double-tap recenter: timing window, ignored targets, camera settle behavior,
  and keyboard/mouselook regression expectations.

Each future issue must keep the default desktop WASD/mouselook behavior
unchanged unless the user explicitly opts into accessibility/touch mode. It
also needs unit tests for gesture disambiguation, mobile/touch smoke when a
touch Verse surface exists, and a desktop regression smoke proving keyboard,
wheel, and mouselook still behave as they do today.

## Tests And Formal Notes

This is an invariant-bearing replacement if executed. Before cutover:

- Model the reducer authority boundary as a bounded state machine:
  service actor vs browser actor vs agent actor, allowed commands, forbidden
  table mutations.
- Convert any meaningful counterexample into command tests.
- Add property-style tests for idempotent bridge replay.
- Add sparse-delta tests proving missing means unchanged.
- Add read-model seam tests proving render/HUD/minimap/nameplate consumers can
  read `WorldReadModel` without transport/backend imports.
- Add interest-scope tests for enter/drop hysteresis, first-sight full records,
  lite follow-up deltas, interest-leave pruning, selected-target promotion, and
  settle-on-stop.
- Add bounded handshake-buffer tests proving frames received during auth/session
  hydration are replayed or rejected with diagnostics, never silently dropped.
- Add command receipt tests proving client `seq` values are echoed for accepted,
  applied, duplicate, stale, and rejected pose/intent writes.
- Add expiry tests with a fake clock.
- Add multi-character same-account tests.
- Add redaction tests for world rows, chat rows, deltas, diagnostics, and
  bridge failures.
- Add moderation tests for empty hard-list behavior, whole-token matching,
  confusable folding, false-positive avoidance, strike escalation, mute windows,
  and no raw private bodies in diagnostics.
- Add two-client desktop smoke and web smoke against the deployed Cloudflare
  service, plus a one-shot read-only SpacetimeDB row-count diff helper that is
  deleted before decommission.

This section is also the model-boundary note paired with the root
`INVARIANTS.md` update from "SpacetimeDB World Projection" to "Verse World
Projection": the new invariant is not satisfied by documentation alone, only by
the P1-P13 contract, authorization, interest, handshake, receipt, moderation,
expiry, redaction, and smoke gates above.

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

### Dragging SpacetimeDB Past The Cut

The fastest way to lose the benefit of this work is to leave a hidden
SpacetimeDB fallback, generated binding, VM runbook, or old env flag "just in
case."

Mitigation: P11 is mandatory, repository search is part of acceptance, and any
remaining old-backend dependency blocks decommission until it is ported or
deleted.

## Recommendation

Decided: **replace SpacetimeDB with an owned Effect/TypeScript Verse World
Service on Cloudflare (Worker + Durable Objects + D1).** Build it, point the
clients at it, verify on the existing gates, and delete SpacetimeDB in the same
effort. No compatibility mirror, no dual `CHAT_WORLD_BACKEND` flag, no
keep-both hedge.

First issue:

```text
Open the ripout tracker and extract packages/world-contract: Effect Schema,
branded refs, tagged errors, row/command/delta/subscription contracts,
WoC-style read-model projection schemas, interest plans, command sequence
receipts, public-safety predicates, and tests. Then scaffold apps/openagents-world
as a Cloudflare Worker + RegionDurableObject + D1 service with hibernatable
WebSockets, bounded handshake buffering, per-region DO storage, and D1
projection tables; not a Bun sidecar.
```

Then proceed straight through the issue list above to the cut. Done means: two
desktop instances and a web client see each other through the Cloudflare service
(two-client smoke), the projection bridge is idempotent, redaction/public-safety
tests pass, interest scopes/deltas/seq receipts/moderation are covered, outages
degrade to single-player, and **no SpacetimeDB module, VM, generated binding, or
runbook remains in the repo.**

The contracts, invariants, validation rules, identity/character model,
subscription lifetimes, and bridge semantics in this document are preserved
exactly — we are changing the implementation and the host, not the world
contract. SpacetimeDB was the right first substrate to find the shape; owning the
contract in one language on one deploy surface is the right second one.

## Addendum (2026-06-22): What a Live Debugging Session Just Proved

This addendum is written immediately after a multi-hour, many-restart debugging
session trying to get two desktop instances to see each other's avatars. It is
not hypothetical. Every failure below was real, and every one of them is a
direct argument for the contract-first, Effect/TypeScript direction this audit
recommends. The session is the case study the rest of this document was missing.

### The bug chain we actually hit (in order)

1. **A runtime value could not cross the Bun→webview boundary.** `OA_CHARACTER`
   was set on the launcher process, but the renderer resolved it through
   `import.meta.env` (build-time Vite define) and `process.env` (absent in the
   webview), so both windows silently fell back to `"main"`. The fix was to
   inject `globalThis.__OA_CHARACTER` via `executeJavascript` and read it lazily.
2. **The deployed Rust/WASM module lagged the client.** The client sent
   `join_region(region, character_id, …)` against a module whose reducer didn't
   yet take `character_id`; the publish path is a separate gcloud/IAP VM
   operation, so client and server drifted.
3. **A table the client depends on was empty in production.** `world_region` is
   seeded only by the `init` reducer, which runs once on first publish and never
   on republish, so it was simply gone. `join_region` does `if (region === null)
   return` — a silent bail with no error anywhere.
4. **The client read every table under the wrong key.** This was the one that
   ate the most time. The SpacetimeDB SDK exposes `connection.db` accessors by
   **snake_case schema name** (`world_region`, `agent_avatar`,
   `avatar_position`, …), but the desktop's `worldRowsFromConnection` read
   **camelCase** (`worldRegion`, `agentAvatar`, …). Every read returned
   `undefined`. The introspection log told the whole story in one line:

   ```text
   db keys=[agent_avatar, … , world_region]
   worldRegion(camel)=false  worldRegionIter=n/a
   world_region(snake)=true  snakeIter=1
   ```

   The row was delivered the entire time (`snakeIter=1`). The read keys were
   wrong. No type error. No runtime error. No log. Just an empty world.

### Why every one of these is an Effect-backend argument

The common thread is **silent, untyped seams between stacks** — exactly what an
owned Effect/TypeScript contract removes:

- **No codegen drift, no casing boundary.** Bug #4 exists only because a
  generated SDK names tables one way and hand-written client code assumed
  another, across an unusual cross-app binding import (already flagged in
  "Generated binding coupling"). A shared `packages/world-contract` with one
  canonical, typed accessor surface makes `worldRegion` vs `world_region` a
  compile error, not a runtime void. There is no second naming convention to
  drift from because there is no second stack generating names.
- **Typed errors instead of silent bails.** Bugs #1, #3, and #4 all failed by
  silently producing emptiness. The `WorldDelta` stream in this audit already
  specifies explicit `snapshot` (with a row count), `heartbeat`, and
  `diagnostic{level,code,message}` events, and the `VerseWorldClient` facade
  returns `Effect`s with `WorldConnectionError`/`WorldStreamError`. "Subscribed,
  0 rows applied for `world_region`" would be a first-class, observable event —
  not something we discover by hand-adding `console.log`s for an hour. We
  literally had to instrument the connect path from scratch because it had no
  diagnostics; the audit's contract bakes them in.
- **One language, one deploy mental model.** This session crossed Rust/WASM, a
  gcloud/IAP VM publish, generated TS bindings, desktop TS, and an opaque SDK
  cache. Bug #2 (module/client drift) and bug #3 (init-only seeding lost on
  republish) are operational seams of running a separate Rust database service.
  The Cloudflare Worker + Effect path collapses this to the same Bun + Effect +
  Worker/D1 release surface the rest of the product already uses.
- **Deterministic, replayable seeding.** Bug #3 is precisely the durable-vs-
  ephemeral split this audit calls for: a region is durable config that should
  be replayable from contract/source, not a one-shot `init` side effect that
  vanishes on redeploy. `ProjectionBridge` + idempotent upserts make "the world
  has no region" structurally impossible after a deploy.
- **Config as typed session parameters, not injected globals.** Bug #1's
  `globalThis.__OA_CHARACTER` injection is a workaround for not having a typed
  connection/session boundary. In the Cloudflare Verse client, the character id
  is just a field on `connect()`/the subscription plan — carried, validated, and
  tested, never smuggled through `executeJavascript`.
- **A test that would have caught it.** The desktop unit tests passed throughout
  this entire broken session, because they exercised the projection logic
  against mocked rows — never the real `connection.db` read path. The audit's
  Cloudflare two-client smoke is the test that fails loudly on "0 rows
  delivered." Contract-level row-delivery tests + a fake transport would have
  turned a multi-hour live debug into a red CI check.

### Honest scope note

None of this means SpacetimeDB was the wrong first substrate — it got the world
shape to "real multiplayer in weeks," which a from-scratch Effect backend would
not have. The four bugs above were individually fixable in the current stack and
bug #4 has since been fixed (the camelCase→snake_case accessor read), so desktop
multiplayer can work today. But that is not the conclusion. The *cost* of this
class of bug came from **untyped seams between four stacks and a generated
binding layer**, and the only structural way to stop paying it is to own the
whole contract in one language on one deploy surface. That is why the decision
(see Executive Read and Recommendation) is to **replace** SpacetimeDB with the
Cloudflare Effect service outright — not to hide it behind a client facade and
keep it underneath. The contract extraction is still step one; it is the first
step of the replacement, not a hedge to preserve the old backend.
