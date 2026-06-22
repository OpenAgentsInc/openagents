# OpenAgents World Worker

`apps/openagents-world` is the Cloudflare Worker and Durable Object home for
the custom Effect/TypeScript Verse world backend. It is the runtime authority
for regional presence, local interaction state, WebSocket fanout, bridge ingest,
and public read-model projection rows. The `openagents.com` Worker/D1 public
product surface remains authoritative for training truth, product promises,
proof/receipt claims, settlement/payout projection, Forum state, private
customer data, provider credentials, private prompts, and private repo content.

## Runtime Shape

- `RegionDurableObject` owns one region per Durable Object ID.
- Worker routes `GET /regions/:regionRef/socket` to the region object by
  `idFromName(regionRef)`, so there is no global singleton.
- The Durable Object uses Cloudflare hibernatable WebSockets via
  `ctx.acceptWebSocket(server)` and stores JSON session metadata with
  `serializeAttachment`.
- Durable Object-local SQLite migrations run in the constructor under
  `blockConcurrencyWhile` and track applied schema in `_sql_schema_migrations`.
- D1 migrations under `migrations/` create the durable cross-region projection
  tables used by bridge ingest and public snapshots.
- Effect services wrap Worker bindings, request context, waitUntil, logging, and
  typed config at the Worker boundary.

## Routes

- `GET /health` returns service health and configured schema version.
- `GET /version` returns Worker and contract versions.
- `GET /connect` returns the default region and socket URL.
- `GET /regions/:regionRef/socket` upgrades to a region WebSocket. A plain HTTP
  request to this route returns a typed world diagnostic.
- `POST /bridge/ingest` is the service-only bridge intake. It schema-decodes
  `WorldBridgePayload`, rejects rows that fail public projection safety, upserts
  deterministic projection rows into D1, records bridge health/cursor state, and
  enqueues a retry-friendly marker when the queue binding is present.

## Transport Contract

Region sockets send JSON transport envelopes:

- `snapshot` frames contain a schema-encoded `WorldDelta` plus a server-owned
  `WorldReadModel` so clients render a coherent projection instead of backend
  table details.
- `delta` frames currently include typed heartbeat frames and reserve the sparse
  update/delete lane for command and bridge work.
- `diagnostic` frames wrap public-safe `WorldDiagnostic` values in a
  `WorldDelta` so cursor and backpressure failures are part of the same stream.

Reconnect accepts `?cursor=cursor.<region>.<sequence>`. Valid cursors resume
with a heartbeat at the current cursor; stale/foreign cursors receive a typed
cursor diagnostic followed by a fresh snapshot. The transport helpers also
define the WoC-style sight policy: first sight sends full records, continued
interest permits lite/dynamic updates, and leaving interest prunes local mirrors
so re-entry sends full records again.

## Browser Command Path

`RegionDurableObject` applies browser/user WebSocket frames as
`WorldCommandEnvelope` values through Effect command handlers. The P4 hot path
implements `join_region`, `leave_region`, `set_avatar_position`, `focus_pylon`,
`clear_pylon_focus`, `send_local_message`, `send_pylon_message`, `send_emote`,
and `set_agent_intent`.

Each handler returns a schema-encoded `WorldCommandReceipt` inside a
`WorldDelta`. Rejections are receipts too: browser actors cannot invoke
service-only projection commands, stale/duplicate pose sequences are rejected,
pose bounds/velocity/cadence are enforced, and chat/emote/intent text is
plain-text bounded with cadence checks. Hot presence rows stay in DO memory;
the DO persists only the transport clock needed for reconnect cursors.

## Expiry And Alarms

Hot rows register TTL metadata in DO SQLite:

- avatar presence: 30 seconds from join or latest pose
- chat messages: 60 seconds
- emotes: 10 seconds
- pylon focus: 30 seconds
- agent intent: 15 seconds

The DO schedules a one-shot Cloudflare alarm for the next expiry deadline, emits
one delete `WorldDelta` per cursor window, persists the updated transport clock,
and reschedules only when more TTL work remains. The expiry planner depends on a
`WorldClock` Effect service, so tests can advance a static clock without sleeping
or relying on wall time.

## Moderation And Abuse Controls

`src/moderation.ts` owns the P7 `WorldModeration` service. The open repository
ships empty hard/soft token JSON arrays in `wrangler.jsonc`; private operators
can seed `OPENAGENTS_WORLD_MODERATION_HARD_TOKENS_JSON` and
`OPENAGENTS_WORLD_MODERATION_SOFT_TOKENS_JSON` with JSON string arrays outside
Git. No private moderation list belongs in this repo.

Local and pylon chat commands pass through moderation before any
`local_chat_message` row is built. The service also exposes explicit gates for
future forum-reflection bubbles and user-authored diagnostic text so P8/P9 work
does not invent a parallel path. Hard-list enforcement is whole-token and
confusable-folded, avoiding substring false positives such as `class` and
`despicable`; soft-list masking remains a client/user preference. Strike state
is kept in the Region DO hot state and escalates from warning to timed mutes,
with public-safe reason codes only. Chat command throttles track account and
session lanes separately from any future IP/edge throttle.

## Subscription Interest Policy

`src/subscriptions.ts` owns the P6 WoC-style interest rules for the Cloudflare
service. `/connect` now returns a server-approved `WorldSubscriptionPlan`, and
WebSocket session attachments persist that plan plus the per-session sight state
needed across hibernation. Clients may request a scope, center, selected target,
and cursor, but the service normalizes the plan and rejects unbounded global
avatar/event row streams.

The pure policy planner keeps the single-region feed below the audit threshold
and switches to split near/far tiers when avatar count or estimated row churn
crosses the documented limit. Interest uses separate enter/drop radii, selected
targets are always promoted to high-resolution, first sight emits full rows,
continued sight can use lite dynamic rows, leaving interest prunes mirrors, and
idle movement emits settle patches. `applySubscriptionDeltaToReadModel` preserves
the sparse delta invariant: absent fields are unchanged, never treated as empty
or default values.

The D1 database IDs in `wrangler.jsonc` point at the provisioned Cloudflare
`openagents-world` and `openagents-world-staging` databases; keep the binding
names stable because the Worker code depends on them.

Production currently deploys to
`https://openagents-world.openagents.workers.dev`. The P13 release receipt lives
at `docs/game/2026-06-22-cloudflare-world-production-release-receipt.md` and
records the production version, live web/desktop smokes, bridge ingest sample,
and historical archive state for the retired GCE SpacetimeDB VM.

## Service Projection Bridge

`src/bridge.ts` owns P8 projection bridge helpers. The bridge never becomes the
source of truth for run, proof, settlement, or product-promise state; it only
persists public-safe rows replayed from public source refs such as
`/api/public/tassadar-run-summary`, future Forum activity refs, and receipt/proof
refs that are already public.

Khala inference serving receipts are bridge-shaped as public `world_event` rows
with typed inference payloads. External provider destinations are
`gateway_station` rows, written only through service identity via
`upsert_gateway_station`, so clients can render the gateway portal without
fabricating provider route or receipt authority.

Projection rows use `row.kind + worldRowKey(row)` as their D1 key, so replaying
the same source payload overwrites the same rows and cannot duplicate
`world_event` entries. Failed ingest writes a public `bridge_health` row and a
diagnostic, never fabricated run/proof/settlement data. Valid ingest adds
`bridge_health` and optional `projection_cursor` rows, then sends only a compact
Queue marker for retriable follow-up work.

Service-only `WorldCommandEnvelope` commands can write durable projection rows,
system messages, and interaction expiry deltas. Browser/agent actors still fail
the shared contract actor gate before service row payloads are decoded.

## Cloudflare Operations

Run from a clean `origin/main` worktree with Cloudflare credentials for the
target account. Keep the binding names in `wrangler.jsonc` stable:
`REGION_DURABLE_OBJECT`, `WORLD_DB`, and `WORLD_BRIDGE_QUEUE`.

Preflight:

```sh
bun run typecheck:world-contract
bun run test:world-contract
bun run typecheck:world-client
bun run test:world-client
bun run typecheck:openagents-world
bun run test:openagents-world
```

D1 migrations:

```sh
cd apps/openagents-world
bunx wrangler d1 migrations list openagents-world --remote
bunx wrangler d1 migrations apply openagents-world --remote
```

For staging, use the staging database name and environment:

```sh
cd apps/openagents-world
bunx wrangler d1 migrations list openagents-world-staging --remote --env staging
bunx wrangler d1 migrations apply openagents-world-staging --remote --env staging
```

The D1 migration files live in `migrations/`. Wrangler records applied D1
migrations in the database migration table; do not edit an applied migration.
Add the next numbered SQL file instead.

Durable Object migrations:

- `wrangler.jsonc` declares `RegionDurableObject` under
  `durable_objects.bindings` and the DO class migration under
  `migrations[{ tag: "v1", new_sqlite_classes: ["RegionDurableObject"] }]`.
- Future DO class renames, deletions, or new classes require a new unique
  Wrangler migration tag. Class deletion removes stored DO data and needs a
  separate invariant/audit note before deploy.
- Per-object SQLite schema migrations stay in the Durable Object constructor
  under `blockConcurrencyWhile`; tests must cover any new schema version.

Deploy:

```sh
cd apps/openagents-world
bun run deploy
```

Staging deploy:

```sh
cd apps/openagents-world
bunx wrangler deploy --env staging
```

Queue and alarm checks:

- The bridge uses the producer binding `WORLD_BRIDGE_QUEUE` and queue name
  `openagents-world-bridge` (or `openagents-world-bridge-staging`). Bridge
  ingest must succeed even if no private retry consumer is deployed yet; the
  queue marker is compact and public-safe.
- DO TTL expiry uses Cloudflare alarms. Keep expiry logic deterministic through
  the `WorldClock` Effect service; validate it with
  `bun test apps/openagents-world/src/expiry.test.ts` rather than sleeping in
  tests.
- Smoke the live Worker with `GET /health`, `GET /version`, `GET /connect`, one
  `POST /bridge/ingest` public-safe payload, and two WebSocket clients in the
  same region. The clients must receive snapshots, browser command receipts,
  interaction deltas, and expiry deletes without moving any private authority
  into the world service.
