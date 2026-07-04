# @openagentsinc/khala-sync

Wire and domain contracts for **Khala Sync**, the owned replication
substrate: Cloud SQL Postgres (authoritative) → Cloudflare edge (per-scope
Durable Object hubs) → SQLite clients (desktop/web/mobile).

> Naming: always the two-word compound **Khala Sync** / `khala-sync`. Bare
> "Khala" is the collective-intelligence product (Episode 242), never this
> engine.

This package is the single source of truth both sides depend on:

- **Branded primitives** — `SyncScope`, `SyncVersion` (per-scope dense
  monotonic server-assigned versions), `MutationId`, `ClientId` /
  `ClientGroupId`, `EntityType` / `EntityId`, `SyncSchemaVersion`,
  `MutatorName`.
- **Scope constructors** — `personalScope`, `teamScope`, `agentRunScope`,
  `threadScope`, `fleetRunScope`, `publicScope` (aligned with the
  `@openagentsinc/sync-worker` taxonomy).
- **Changelog** — `ChangelogEntry` (post-image v1 model; deletes are
  tombstones; apply idempotent by `(scope, version, entityType, entityId)`).
- **Mutations** — `MutationEnvelope` / `MutationResult` for named,
  server-authoritative mutators with in-band rejection (rejections ACK and
  never block the client queue).
- **Wire protocol** — `PushRequest`/`PushResponse`,
  `BootstrapRequest`/`BootstrapResponse` (snapshot pages stitched to a
  cursor), `LogPage` (offset-resumable catch-up), and the WebSocket
  `LiveFrame` union (`DeltaFrame`, `MutationAckFrame`, `MustRefetchFrame`,
  `PingFrame`).
- **Error taxonomy** — `SyncError` with the closed `SyncErrorCode` set.

Related packages:

- `@openagentsinc/khala-sync-server` — Postgres substrate, mutator engine,
  capture, `KhalaSyncHubDO`.
- `@openagentsinc/khala-sync-client` — local store, transport, rebase.

Spec: `docs/khala-sync/SPEC.md`. Design rationale:
`docs/fable/2026-07-04-database-alternatives-and-postgres-sync-engine.md`.

```sh
bun test packages/khala-sync
bun run --cwd packages/khala-sync typecheck
```
