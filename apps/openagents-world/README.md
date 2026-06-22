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

The D1 database IDs in `wrangler.jsonc` are placeholders until the actual
Cloudflare resources are provisioned; keep the binding names stable because the
Worker code depends on them.
