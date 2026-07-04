# @openagentsinc/khala-sync-server

Server substrate for **Khala Sync** (see `packages/khala-sync` for the wire
contracts and `docs/khala-sync/SPEC.md` for the spec).

Components (workstreams in parentheses; see the KS epic on GitHub):

- **`migrations/`** — Cloud SQL Postgres schema (KS-2):
  `khala_sync_scopes` (per-scope version counters + retention watermark),
  `khala_sync_changelog` (the transactional outbox / replication log, with
  a `pg_notify` wake trigger), `khala_sync_mutations` (per-client mutation
  ledger), `khala_sync_client_state`.
- **Mutator engine** (KS-3) — `MutatorRegistry` of named,
  server-authoritative mutators; each executes in ONE Postgres transaction:
  permission check → validation → business writes → changelog appends →
  mutation ledger upsert. Rejections are in-band values that ACK the
  mutation; they never block the client queue.
- **Read service** (KS-2) — consistent bootstrap snapshot pages stitched to
  a cursor, and offset-resumable `LogPage` catch-up.
- **Capture** (KS-4) — tails `khala_sync_changelog` over a DIRECT Postgres
  connection (`LISTEN khala_sync_changelog_append` wake + poll fallback —
  Hyperdrive drops LISTEN/NOTIFY) and pushes `DeltaFrame`s to per-scope
  hubs.
- **`KhalaSyncHubDO`** (KS-4) — one Durable Object per scope: recent log
  window in DO SQLite, hibernating WebSockets, offset-resumable HTTP
  catch-up, `MustRefetch` for cursors behind the window. The DO is a cache
  and fan-out layer only; Postgres is authoritative.

Connectivity invariants:

- Worker request paths (push, bootstrap, log) reach Postgres through
  **Hyperdrive** — transaction-mode pooling, so mutators must be
  single-transaction and session state is forbidden.
- Capture and migrations use a direct connection path.

Status: contracts, schema, migration runner (KS-0.3), and the transactional
outbox writer + per-scope version allocator (KS-2.1) landed; the mutator
engine, reads, capture, and hub land per the remaining KS-2/KS-3/KS-4 issues.

## Outbox writer (KS-2.1)

`withSyncTransaction(sql, fn)` (`src/outbox-writer.ts`) opens ONE Postgres
transaction and hands `fn` a `SyncTransactionWriter`:

- **`writer.sql`** — the transaction's Bun SQL handle for the caller's own
  business writes. Business rows, changelog rows, and the scope counter all
  commit or roll back together (SPEC §7 invariant 5).
- **`writer.appendChange({ scope, entityType, entityId, op, postImage?,
  mutationRef? })`** — appends one changed entity to `khala_sync_changelog`.
  Post-images are serialized with `canonicalJson` from
  `@openagentsinc/khala-sync` (never pre-stringified JSON); `op: "delete"`
  entries are tombstones and must not carry a post-image — enforced at the
  type level, at runtime, and by the schema CHECK constraint. Appending the
  same entity twice in one transaction collapses to one row (last write
  wins): one row per changed entity per transaction per scope (SPEC §2.3).
- **`writer.allocateVersion(scope)`** — allocates this transaction's version
  for a scope via `INSERT … ON CONFLICT (scope) DO UPDATE SET last_version =
  khala_sync_scopes.last_version + 1 … RETURNING last_version`, i.e. under
  the counter row lock, INSIDE the business transaction (insert-on-conflict
  bootstrap for new scopes). One version per transaction per scope;
  `appendChange` allocates (or reuses) it automatically.

```ts
import { SQL } from "bun"
import { withSyncTransaction } from "@openagentsinc/khala-sync-server"

const entry = await withSyncTransaction(new SQL({ url }), async (writer) => {
  await writer.sql`UPDATE things SET state = 'done' WHERE id = ${id}`
  return writer.appendChange({
    scope,
    entityType,
    entityId,
    op: "upsert",
    postImage: { id, state: "done" },
    mutationRef: "mutation:example:1",
  })
})
```

**Invariant 1 (SPEC §7): per-scope versions are dense, monotonic, and
server-assigned in-txn.** This holds by construction: the counter update
runs inside the same transaction as the business writes and changelog
appends, so concurrent writers serialize on the counter row lock, and a
rollback rolls the counter back with everything else — committed version
sequences can have no gaps and no reorders. `src/outbox-writer.test.ts`
proves it against real local Postgres (sequential density, concurrent
interleaving, rollback-no-gap, per-scope independence, tombstone
constraints, and `ChangelogEntry` codec round-trips).

Failures are typed: SQL-layer errors map to `KhalaSyncStorageError`
(`connection_failed | transaction_conflict | constraint_violation |
unavailable`, `src/errors.ts`); the caller's own domain errors pass through
`withSyncTransaction` unchanged (still rolling back the transaction).

## Migrations runbook (KS-0.3)

The migration runner applies the ordered `.sql` files in `migrations/` over a
**direct** Postgres connection (never Hyperdrive — see the connectivity
invariants above). It is idempotent: applied files are recorded in a
`khala_sync_migrations` ledger table (`filename`, `sha256`, `applied_at`),
each file runs in one transaction together with its ledger insert, and the
runner takes a session advisory lock so concurrent runners cannot interleave.
It **refuses to run** (dry or real) if an already-applied file's content hash
changed on disk or if an applied file disappeared — never edit a landed
migration; add the next `NNNN_*.sql` instead.

```sh
# Plan only (CI-safe; no writes, not even the ledger table):
bun run --cwd packages/khala-sync-server migrate -- --dry-run \
  --database-url "postgres://user@host:5432/db"

# Apply (URL via flag or KHALA_SYNC_DATABASE_URL):
KHALA_SYNC_DATABASE_URL="postgres://user@host:5432/db" \
  bun run --cwd packages/khala-sync-server migrate
```

Environments:

- **Local** — the integration tests spin up a throwaway Postgres
  (`initdb` + `pg_ctl` on a random port, deleted afterwards) via
  `src/test/local-postgres.ts` (`startLocalPostgres()`, reusable by the
  KS-2.x lanes; requires `brew install postgresql@16`, tests skip cleanly
  without it). To poke at a persistent local database, start one yourself
  and point `--database-url` at it.
- **Staging / production** — NOT YET APPLIED. The Cloud SQL instance is
  KS-0.1, which has not landed as of 2026-07-04; once it exists, run the
  same command against the instance's direct connection URL (Cloud SQL Auth
  Proxy or private IP — never the Hyperdrive connection string) and record
  the run on the KS-0.1/KS-0.3 issues.

Verification: `bun test packages/khala-sync-server` and
`bun run --cwd packages/khala-sync-server typecheck`.
