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

## Connection reference (KS-0.1 #8283 / KS-0.2 #8284 — redacted)

Cloud SQL instance (created 2026-07-04, PostgreSQL 17):

- **Instance:** `khala-sync-pg`, GCP project `openagentsgemini`, region
  `us-central1`.
- **Databases:** `khala_sync_prod` (production), `khala_sync_staging`
  (staging). Same instance, separate databases.
- **Roles:** `khala_app` (Worker request paths via Hyperdrive),
  `khala_migrate` (schema migrations, direct connection), `khala_capture`
  (changelog capture / LISTEN, direct connection).
- **Secrets:** passwords and the instance IP are NEVER committed. They live
  in the gitignored workspace file `~/work/.secrets/khala-sync-cloudsql.env`
  (owner machine) and inside the Cloudflare Hyperdrive configs. Do not copy
  them into tracked files, commits, or issue comments.

Cloudflare Hyperdrive (Worker request path only; config ids are public
references, not secrets):

- **Worker binding:** `KHALA_SYNC_DB` in
  `apps/openagents.com/workers/api/wrangler.jsonc` (`env.KHALA_SYNC_DB`,
  exposing `connectionString`).
- **Prod config:** `khala-sync-prod`, id
  `6cd885288e5b4b2f8fd2d76200c980bf` → `khala_sync_prod` as `khala_app`.
- **Staging config:** `khala-sync-staging`, id
  `63a375a24d6f475db60d9490f55c9102` → `khala_sync_staging` as `khala_app`.
- **Pooling mode:** transaction — no LISTEN/NOTIFY, no session `PREPARE`, no
  advisory locks on this path (SPEC §4). The postgres.js driver needs the
  Worker's `nodejs_compat` compatibility flag (already set).

Connectivity smoke: `GET /api/internal/khala-sync/db-smoke` (admin bearer
only) proves a round-trip parameterized query through the binding from the
deployed Worker and returns `{ ok, khalaSyncTables, latencyMs }` — see
`apps/openagents.com/workers/api/src/khala-sync-db-smoke-routes.ts`.

Status: contracts, schema, migration runner (KS-0.3), the transactional
outbox writer + per-scope version allocator (KS-2.1), and the mutation
ledger + client-group state (KS-2.4) landed; the mutator engine, reads,
capture, and hub land per the remaining KS-2/KS-3/KS-4 issues.

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

## Mutation ledger (KS-2.4)

`src/mutation-ledger.ts` implements the per-client idempotency ledger
(`khala_sync_mutations`) and the user-bound client-group state
(`khala_sync_client_state`) from SPEC §2.4/§3. Everything is
single-transaction-safe — no session state, no LISTEN/NOTIFY, no advisory
locks; serialization uses ordinary row locks, the same discipline as the
scope counter — so it runs through Hyperdrive's transaction-mode pooling.

Semantics:

- **Recording is the commit.** The push engine calls
  `recordMutation(writer.sql, { clientGroupId, clientId, mutationId, name,
  status, errorCode?, resultJson?, scope? })` INSIDE the mutator
  transaction, so the ledger row commits atomically with the business
  writes and changelog appends. Insert-once (`ON CONFLICT … DO NOTHING`):
  the first recording wins, and the returned `inserted` flag says whether
  this call created the row.
- **Duplicates never re-execute.** `checkAndReserve(writer.sql,
  { clientGroupId, clientId, envelope })` gates every envelope before
  execution. If the `(clientGroup, client, mutationId)` row exists, it
  returns `{ kind: "duplicate", recorded, result }` where `result` is
  `status: "duplicate"` carrying the RECORDED outcome (errorCode /
  errorMessageSafe from the recorded `resultJson`) — this is exactly the
  crash-between-execute-and-respond replay path: the commit already
  happened, so the replayed envelope is answered from the recording and no
  mutator runs (SPEC §7 invariant 3).
- **Per-client sequential ordering.** `lastMutationId` is
  `MAX(mutation_id)` for the `(clientGroup, client)` pair (0 when the
  client never pushed; also exposed as `lastMutationId(sql, key)`). Only
  `lastMutationId + 1` reaches execution; ids ≤ `lastMutationId` are
  duplicates; ids > `lastMutationId + 1` come back as
  `{ kind: "out_of_order", result }` — a TYPED, IN-BAND rejection
  (`status: "rejected"`, `errorCode: "out_of_order"`). Rejections ACK
  in-band and never 4xx/block the queue (SPEC §2.4 acceptance rules;
  §7 invariant 2) — but `out_of_order` specifically acks NOTHING: no
  ledger row is written and `lastMutationId` does not advance, so the
  client re-pushes the missing prefix and the gap heals. The ledger stays
  dense by construction.
- **Client groups are user-bound, and the state row is the serialization
  point.** `upsertClientState(sql, { clientGroupId, userId,
  schemaVersion })` inserts-or-updates `khala_sync_client_state` on every
  push (schema version refresh + `last_seen_at` bump) and throws a typed
  `KhalaSyncClientStateMismatchError` — leaving the row untouched — when
  the stored `user_id` differs. A client group never migrates between
  users. The push engine MUST call it inside the mutator transaction
  BEFORE gating envelopes: the upsert takes the group's row lock, so
  concurrent pushes for one client group serialize instead of racing
  `MAX(mutation_id)`. `checkAndReserve` enforces the order by re-taking
  that row lock (`SELECT … FOR UPDATE`) and failing typed if the state row
  is missing.

`getMutation(sql, { clientGroupId, clientId, mutationId })` reads one
recorded row back (`resultJson` re-canonicalized after jsonb
normalization). `src/mutation-ledger.test.ts` proves the semantics against
real local Postgres: duplicate replay with zero changelog side-effects,
interleaved duplicates within a batch, crash-between-execute-and-respond
replay returning the same recorded result, out-of-order gap rejection +
healing, sequential progression across batches, insert-once recording, and
client-state binding/rollback.

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
- **Staging / production** — the Cloud SQL instance exists (KS-0.1 #8283;
  see the connection reference above). Run the same command as
  `khala_migrate` against the instance's direct connection URL (Cloud SQL
  Auth Proxy or authorized-network IP — never the Hyperdrive connection
  string) and record the run on the KS-0.3 issue.

Verification: `bun test packages/khala-sync-server` and
`bun run --cwd packages/khala-sync-server typecheck`.
