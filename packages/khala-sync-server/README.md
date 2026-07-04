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
outbox writer + per-scope version allocator (KS-2.1), the read service
(bootstrap snapshot + log pages, KS-2.2), the mutation ledger +
client-group state (KS-2.4), and the transactional push engine + mutator
registry (KS-3.1, wired to `POST /api/sync/push` in the `openagents.com`
Worker) landed; compaction, capture, and the remaining hub seams land per
the KS-2/KS-4 issues, fleet mutators per KS-3.2.

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

## Read service (KS-2.2)

`src/read-service.ts` implements the two Hyperdrive-path reads from SPEC §3.
Both are one self-contained Postgres transaction per call (REPEATABLE READ);
nothing is held between requests, because Hyperdrive's transaction-mode
pooling cannot pin a session across them.

### `logPage(sql, { scope, afterVersion, limit })`

Ordered catch-up: `khala_sync_changelog` rows for `scope` strictly after the
`afterVersion` watermark (`null`/0 = scope start), ordered by
`(version, entity_type, entity_id)`.

- **`nextCursor`** — the highest version returned, or `afterVersion` when
  the page is empty (a `SyncVersionWatermark`: 0 means "still at scope
  start").
- **`upToDate`** — `nextCursor === khala_sync_scopes.last_version`, with the
  counter read in the SAME transaction/snapshot as the page rows.
- **Pages never split a version.** The `limit` bounds *distinct versions*,
  not raw rows: since resumption reads `version > nextCursor`, a page that
  ended mid-version would silently skip that version's remaining rows. A
  page therefore holds at most `limit` versions, each bounded by the
  entities one transaction touched.
- **Retention refusal** — when `afterVersion <
  khala_sync_scopes.retained_from_version - 1`, the requested range has been
  compacted away; the call fails with the typed
  `KhalaSyncCursorBehindRetainedWindowError` (wire mapping:
  `MustRefetch(cursor_behind_retained_window)` — SPEC invariant 6: never a
  silently partial log).

### `bootstrap(sql, { scope, pageSize, pageToken? })`

Consistent snapshot pages of the scope's CURRENT entity states, stitched to
the scope version (the stitch cursor):

- **v1 state source** — current states are derived from the changelog:
  `DISTINCT ON (entity_type, entity_id) … ORDER BY … version DESC` picks
  each entity's latest row at `version <= snapshotCursor`; entities whose
  latest row is a tombstone are omitted. (Compaction must therefore always
  preserve each live entity's latest upsert row.)
- **The stitch cursor** — the first page reads
  `khala_sync_scopes.last_version` in the same REPEATABLE READ transaction
  as its rows. This is gap-free by construction: version allocation holds
  the scope-counter row lock until the writing transaction commits (KS-2.1),
  so versions are commit-ordered — observing `last_version = v` guarantees
  every version `<= v` is committed and visible.
- **Multi-page without a held transaction (the Hyperdrive seam design)** —
  page tokens are self-contained: base64url of
  `{ v, scope, snapshotCursor, lastEntityKey }`. Every later page re-derives
  latest-per-entity under `version <= snapshotCursor AND (entity_type,
  entity_id) > lastEntityKey`. Committed rows at versions `<=
  snapshotCursor` are immutable (writers only append at higher versions), so
  each page equals what a held snapshot at `snapshotCursor` would have
  returned, no matter how many writes commit between pages. Only compaction
  can invalidate a token: once `retained_from_version` passes the snapshot
  cursor, the post-bootstrap stitch (`logPage(afterVersion =
  snapshotCursor)`) could no longer be served, so every page fails closed
  with `KhalaSyncCursorBehindRetainedWindowError` and the client
  re-bootstraps.
- **Final page** — carries `cursor = snapshotCursor` (watermark; 0 for an
  empty scope) and no `nextPageToken`. The client stitches: apply all
  snapshot pages, then `logPage` from exactly `cursor` until `upToDate`.
  Entities that changed after the snapshot are re-delivered by the log with
  newer post-images; apply is idempotent per (scope, version, entity), so
  the seam is exact.
- Malformed, cross-scope, or ahead-of-scope tokens fail with the typed
  `KhalaSyncInvalidPageTokenError` (client restarts the bootstrap).

`src/read-service.test.ts` proves the seam against real local Postgres: the
acceptance test snapshots at version `v`, commits `v+1..v+k` interleaved
with the snapshot pages (updates to already-paged and not-yet-paged
entities, deletes, creates, create-then-delete), then verifies that
client-side apply of snapshot pages + `logPage((v, v+k])` converges to
exactly the final entity states — byte-equal canonical post-images, and
byte-equal to a fresh single-page bootstrap taken afterwards. Paging
correctness (version-boundary integrity, tombstone slots, empty-scope
watermarks) and both retention refusals are covered alongside.

## Push engine (KS-3.1)

`src/push-engine.ts` is the transactional mutator engine behind
`POST /api/sync/push` (SPEC §2.4/§3, invariants 3 and 5):

- **`makeMutatorRegistry([defineMutator({ name, decodeArgs, execute })])`**
  — named, server-authoritative mutators. `execute(args, ctx)` runs inside
  ONE Postgres transaction with a `MutatorContext`: `userId` (the
  AUTHENTICATED caller), `clientGroupId`/`clientId`/`mutationId`,
  `mutationRef` (pass it on every `appendChange` so changelog entries stay
  attributable — invariant 3), and `writer` (the transaction-scoped
  `SyncTransactionWriter`: `writer.sql` for business writes,
  `writer.appendChange` for changelog appends). Rejections are VALUES
  (`status: "rejected"`); the engine commits the transaction even for
  rejected results (the ledger ack must commit), so mutators MUST
  permission-check and validate BEFORE writing. Throwing is reserved for
  storage failures that abort the batch.
- **`executePush({ sql, registry, userId, request })`** — runs one decoded
  `PushRequest` batch. Per envelope, in ONE transaction each:
  `upsertClientState` (client-group row lock + user binding — a group bound
  to a different user throws the typed
  `KhalaSyncClientStateMismatchError`, a whole-request 403-class failure)
  → `checkAndReserve` (duplicate ⇒ recorded result, no re-execution;
  out-of-order ⇒ in-band rejection that acks NOTHING) → decode args
  (unknown mutator / bad args ⇒ in-band `rejected` recorded in the ledger;
  the decode error is never echoed — it can embed raw values) → mutator
  execution → `recordMutation` — all atomic. Storage failures abort the
  batch (`KhalaSyncStorageError`, retryable): the committed prefix stays
  committed and replays as duplicates. The response carries `results` in
  request order plus the `lastMutationId` watermark (`0` when nothing is
  acked yet).
- **Driver seam** — `src/sql.ts` types the SQL handle STRUCTURALLY
  (tagged-template query + `begin`), so the same engine runs on Bun's
  `SQL` (tests, capture) and on postgres.js through Hyperdrive inside the
  Worker (SPEC §4: single transactions, ordinary row locks, no session
  state).

The Worker surface is `POST /api/sync/push`
(`apps/openagents.com/workers/api/src/khala-sync-push-routes.ts`):
authenticated via the standard actor auth (browser session or agent
bearer), typed `SyncError` bodies for whole-request failures
(401 `unauthenticated`, 400 `invalid_request` /
`protocol_version_unsupported` / `schema_version_unsupported`,
403 `unauthorized_scope`, 503 `storage_unavailable`, 500 `internal`), and
per-mutation rejections IN-BAND in a 200 `PushResponse` — never a
queue-blocking 4xx. The Worker's v1 registry
(`src/khala-sync-mutators.ts` there) carries one system-test mutator,
`sync.debugEcho`, which writes a `sync_debug_echo` entity into the
caller's OWN personal scope only; fleet mutators land per KS-3.2.

`src/push-engine.test.ts` proves the engine against real local Postgres:
applied/rejected/duplicate/out-of-order flows, request-order results,
atomicity (a mid-mutator failure rolls back business write, changelog,
ledger row, AND the scope counter — versions stay dense on retry),
client-group user binding, and empty-push watermark reporting.

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
