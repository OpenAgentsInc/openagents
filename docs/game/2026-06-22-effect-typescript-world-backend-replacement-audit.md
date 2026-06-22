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

## Build Order (no phased mirror — build it, cut, delete SpacetimeDB)

This is a build sequence, not a "run both backends as production" hedge. There is
no `CHAT_WORLD_BACKEND` dual-adapter and no perpetual compatibility mirror. The
order below exists only because you cannot write all the code in one commit; each
step is a normal PR, and the desktop/web clients point at the new Cloudflare
service the moment it can serve them. SpacetimeDB is deleted at the end of the
same effort, not kept alive behind a flag.

The only thing that runs "in parallel" is a short verification window where the
old SpacetimeDB read path is used purely to diff row counts during the cut — not
a production fallback.

1. **Contract.** `packages/world-contract`: Effect Schema for rows, commands,
   the versioned `WorldDelta` stream, subscription plans, the
   `avatar.identity.<id>.char.<char>` helper, region bounds, and public-safety
   predicates. Tests for avatar-ref construction, character-id sanitization,
   region bounds, command authorization classes, row redaction, and source-ref
   requirements. This is the single source of types for server and clients.
2. **Cloudflare service skeleton.** `apps/openagents-world` Worker + a
   `RegionDurableObject` with hibernatable WebSocket fanout, a snapshot+delta
   transport implementing `WorldDelta`, and D1 schema for durable rows. Connect
   handshake, health, auth.
3. **Commands + presence in the region DO.** `join_region`, `leave_region`,
   `set_avatar_position`, `focus_pylon`, local/pylon message, emote, intent —
   with the same bounds, velocity, cadence, TTL, and chat rules enforced as
   Effect Schema decoders + command tests. Expiry on DO alarms; hot presence in
   DO memory; durable checkpoints in storage/D1.
4. **Subscriptions + near/far.** Region-scoped + selected-entity subscriptions,
   snapshot-then-delta, absent-means-unchanged sparse deltas, settle-on-stop,
   and high/low presence feeds with per-DO fanout policy.
5. **Projection bridge.** Service-only ingest of `GET /api/public/...` +
   forum activity into D1 projection rows and `world_event`s, with deterministic
   `event_ref`s, idempotent replay, and `bridge_health` on failure.
6. **Point the clients at it + cut.** `packages/world-client` is the only client
   facade; desktop and web import it and connect to the Cloudflare service. Run
   the two-client smoke and a one-shot row-count diff against the old
   SpacetimeDB read path to confirm parity, then **delete**
   `apps/openagents-world-spacetimedb`, the generated bindings, the GCP VM,
   nginx/TLS/certbot, the WASM publish lane, and the IAP runbook. Update
   `INVARIANTS.md`, `CLAUDE.md`/AGENTS, deployment docs, and README from
   "SpacetimeDB world projection" to "Verse world projection (Cloudflare)."

Definition of done for the whole effort: two desktop instances (and a web
client) see each other through the Cloudflare service via the two-client smoke,
the projection bridge is idempotent, public-safety/redaction tests pass, an
outage degrades to single-player, and **no SpacetimeDB code, VM, or binding
remains in the repo.**

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

Decided: **replace SpacetimeDB with an owned Effect/TypeScript Verse World
Service on Cloudflare (Worker + Durable Objects + D1).** Build it, point the
clients at it, verify on the existing gates, and delete SpacetimeDB in the same
effort. No compatibility mirror, no dual `CHAT_WORLD_BACKEND` flag, no
keep-both hedge.

First issue:

```text
Stand up the Cloudflare Verse World Service: packages/world-contract (Effect
Schema) + apps/openagents-world (Worker + RegionDurableObject + D1) speaking the
WorldDelta snapshot/delta protocol, with packages/world-client as the only client
facade.
```

Then proceed straight through the Build Order above to the cut. Done means: two
desktop instances and a web client see each other through the Cloudflare service
(two-client smoke), the projection bridge is idempotent, redaction/public-safety
tests pass, outages degrade to single-player, and **no SpacetimeDB module, VM,
generated binding, or runbook remains in the repo.**

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
  Option B/C here (Worker/Bun + Effect) collapse this to the same Bun + Effect
  + Worker/D1 release surface the rest of the product already uses.
- **Deterministic, replayable seeding.** Bug #3 is precisely the durable-vs-
  ephemeral split this audit calls for: a region is durable config that should
  be replayable from contract/source, not a one-shot `init` side effect that
  vanishes on redeploy. `ProjectionBridge` + idempotent upserts make "the world
  has no region" structurally impossible after a deploy.
- **Config as typed session parameters, not injected globals.** Bug #1's
  `globalThis.__OA_CHARACTER` injection is a workaround for not having a typed
  connection/session boundary. In the neutral client, the character id is just a
  field on `connect()`/the subscription plan — carried, validated, and tested,
  never smuggled through `executeJavascript`.
- **A test that would have caught it.** The desktop unit tests passed throughout
  this entire broken session, because they exercised the projection logic
  against mocked rows — never the real `connection.db` read path. The audit's
  "two-client smoke against both backends through the same neutral client" is
  the test that fails loudly on "0 rows delivered." Contract-level row-delivery
  tests + a fake transport would have turned a multi-hour live debug into a red
  CI check.

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
Cloudflare Effect service outright — not to insulate it behind a neutral client
and keep it underneath. The contract extraction is still step one; it is the
first step of the replacement, not a hedge to preserve the old backend.

