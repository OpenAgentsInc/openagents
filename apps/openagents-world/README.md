# OpenAgents World Worker

`apps/openagents-world` is the Cloudflare Worker and Durable Object home for
the custom Effect/TypeScript Verse world backend. It replaces SpacetimeDB as the
runtime authority for regional world state, WebSocket fanout, bridge ingest, and
public read-model projection.

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
- `POST /bridge/ingest` is the service-only bridge intake scaffold. It records
  an accepted bridge diagnostic and enqueues a retry-friendly marker when the
  queue binding is present.

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

The D1 database IDs in `wrangler.jsonc` are placeholders until the actual
Cloudflare resources are provisioned; keep the binding names stable because the
Worker code depends on them.
