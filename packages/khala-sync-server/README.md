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
client-group state (KS-2.4), the transactional push engine + mutator
registry (KS-3.1, wired to `POST /api/sync/push` in the `openagents.com`
Worker), the mutator authoring guide + queue-never-blocks behavior
contract (KS-3.3), the hub DO (KS-4.2, in the `openagents.com` Worker),
the capture worker (KS-4.1, `src/capture.ts` + `scripts/capture.ts` —
imported via the `@openagentsinc/khala-sync-server/capture` subpath so the
root entrypoint stays workerd/node-importable), changelog compaction
(KS-2.3), and the fleet cockpit scope projection + operator mutators
(KS-6.1, see below) landed; the remaining hub seams land per the KS-4
issues.

## Fleet cockpit scope (KS-6.1, #8302)

`src/fleet-projection.ts` + `src/fleet-mutators.ts` +
`migrations/0004_khala_sync_fleet.sql`:

- **Scope ownership** — `khala_sync_scope_owners` (scope PK,
  owner_user_id, created_at), written first-writer-wins on a fleet scope's
  first projection append (`ensureScopeOwner`). The read gate is the KS-7.1
  taxonomy-complete resolver (`src/scope-auth.ts` `resolveScopeRead` over
  injected capability callbacks; fleet scopes resolve through
  `readScopeOwner` here). The older `canReadScopeV1` helper remains only as
  the projection-local predicate superseded by that resolver.
- **Projection** — allowlist redaction mappings from raw row shapes into
  the fleet entity contracts (`fleetRunPostImage`,
  `fleetWorkerPostImage`, `fleetAssignmentPostImage`,
  `fleetAccountPostImage`; SPEC §7 invariant 9 — the contract ref patterns
  structurally refuse emails/paths, plus a serialized forbidden-material
  guard), `appendFleetEntityChange` for writer-scoped appends, and
  `projectFleetEntitiesBestEffort` — the FAIL-SOFT wrapper for the v1
  dual-write call site in the `openagents.com` Worker (assignment
  create/closeout project after the D1 business write; a projection
  failure NEVER fails the business write and returns a typed diagnostic).
  KS-8.1 (#8307) retires the dual-write by moving the business write into
  the same Postgres transaction.
- **Operator mutators** — `fleet.setDesiredSlots`, `fleet.pauseRun`,
  `fleet.resumeRun` (`fleetOperatorMutators`), registered in the Worker
  registry. Owner-gated via `khala_sync_scope_owners` (foreign user ⇒
  in-band `unauthorized_scope` rejection with zero writes); each applied
  mutation writes a durable intent row (`khala_sync_fleet_intents`) and
  appends the updated `fleet_run` post-image in one transaction. HONEST
  V1: intents are recorded and projected; Pylon-supervisor ENFORCEMENT of
  intents is a follow-up lane — an applied mutation is a durable request,
  not yet proof the fleet changed behavior.

**Writing a mutator? Read the authoring guide first:**
[`docs/khala-sync/MUTATORS.md`](../../docs/khala-sync/MUTATORS.md) —
single-transaction rule, replay-safety, in-band rejection discipline (never
4xx business validation), Hyperdrive session-state rules, ledger
idempotency, scope authorization inside the mutator, `canonicalJson`
post-images, Worker registry registration, and the testing checklist. The
acceptance rule "validation failures ack the mutation and report the error
in-band — they never 4xx/block the queue" is an enforced behavior contract
(`khala_sync.push.validation_never_blocks_queue.v1` in
`packages/behavior-contracts/src/khala-sync.ts`) whose oracle lives in
`src/push-engine.test.ts`.

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

## Capture worker (KS-4.1)

`src/capture.ts` + the `scripts/capture.ts` CLI implement capture from
SPEC §4: a supervised Bun process with a **direct** Postgres connection
(never Hyperdrive — transaction-mode pooling drops LISTEN/NOTIFY) that
tails `khala_sync_changelog` and pushes ordered batches to each scope's
`KhalaSyncHubDO` through the deployed Worker's internal append route
(`POST /api/internal/khala-sync/hub/append?scope=…`, admin bearer — the
KS-4.2 surface that exists exactly for this producer).

**Runtime home (v1 decision, #8294):** a supervised Bun process on our own
infrastructure (launchd on the operator Mac first; the oa GCE box later),
NOT inside the Worker — Workers cannot hold a LISTEN session through
Hyperdrive. The WAL/pgoutput upgrade (SPEC §4) swaps only this component.

Semantics:

- **Wake, then read.** `LISTEN khala_sync_changelog_append` (fired by the
  0001 trigger) is a wake signal only; every pass reads authoritative rows
  from Postgres with the KS-2.2 `logPage` query — whole version groups per
  batch, never splitting one transaction's entries across pushes. A
  configurable short-poll interval (default 5 s) is the fallback for
  dropped notifications and listener downtime; the postgres.js listener
  re-subscribes automatically after reconnects.
- **Checkpoints.** `khala_sync_capture_checkpoints` (migration 0002; scope
  PK, `pushed_through_version`, `updated_at`) records the highest version
  each hub acknowledged. The checkpoint advances ONLY on hub 2xx (including
  the idempotent-replay acknowledgment `appended: 0`) and is monotonic
  (GREATEST on write). Delivery is at-least-once; the hub dedupes by
  version. Startup discovers every scope with activity beyond its
  checkpoint in one query (absent rows behave as watermark 0).
- **Hub 409 version gap** (`khala_sync_hub_version_gap`): the hub's gap
  check protects ITS window edge; its 409 body carries
  `expectedFirstVersion`, so capture re-reads from
  `expectedFirstVersion - 1` and re-pushes forward (once per pass). If that
  expectation is already behind the Postgres retained window, capture logs
  the scope error and leaves the checkpoint — it never fabricates a
  partial log (SPEC invariant 6); the hub heals via client re-bootstrap.
- **Failure posture.** Hub 5xx/network → bounded in-pass retry with
  backoff, checkpoint unmoved, retried again next wake/poll. One scope's
  failure never blocks other scopes or crashes the daemon (per-scope
  isolation). Postgres connection loss → the daemon backs off (bounded
  exponential) and reconnects.

### Capture runbook

Config comes from env (CLI flags override; the token is env-only so it
never appears in process listings):

| Variable | Meaning |
|---|---|
| `KHALA_SYNC_DATABASE_URL` | direct Postgres URL as `khala_capture` (Cloud SQL Auth Proxy or authorized-network IP — never the Hyperdrive string) |
| `KHALA_SYNC_HUB_APPEND_URL` | Worker internal append route: prod `https://openagents.com/api/internal/khala-sync/hub/append`, staging the staging host's same path |
| `OPENAGENTS_ADMIN_API_TOKEN` | admin bearer for the internal route |
| `KHALA_SYNC_CAPTURE_POLL_INTERVAL_MS` | poll fallback interval (default 5000) |
| `KHALA_SYNC_CAPTURE_BATCH_VERSIONS` | distinct versions per append batch (default 200) |

Secrets live in the gitignored workspace files
(`~/work/.secrets/khala-sync-cloudsql.env` for the database role,
`OPENAGENTS_ADMIN_API_TOKEN` from the workspace admin-token secret); never
copy them into tracked files or logs.

```sh
# One pass (cron/test mode; exit 1 if any scope failed):
bun run --cwd packages/khala-sync-server capture -- --once

# Daemon (LISTEN wake + poll fallback; SIGINT/SIGTERM stop cleanly):
bun run --cwd packages/khala-sync-server capture
```

launchd supervision (owner Mac): fill in the placeholders in
`ops/com.openagents.khala-sync-capture.plist` (bun path, repo path, the
env vars above, log dir), copy it to
`~/Library/LaunchAgents/com.openagents.khala-sync-capture.plist`, then
`launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/com.openagents.khala-sync-capture.plist`.
`KeepAlive` restarts it on crash with a 10 s throttle. Installing/starting
this is a **KS-6 deploy step** — do not run it against production until the
KS-6 wiring lands.

Liveness checks:

- `SELECT scope, pushed_through_version, updated_at FROM
  khala_sync_capture_checkpoints ORDER BY updated_at DESC` — `updated_at`
  advancing while writes commit means capture is pushing.
- Lag: compare against `khala_sync_scopes.last_version` — `SELECT s.scope,
  s.last_version - COALESCE(c.pushed_through_version, 0) AS lag FROM
  khala_sync_scopes s LEFT JOIN khala_sync_capture_checkpoints c USING
  (scope) WHERE s.last_version > COALESCE(c.pushed_through_version, 0)`.
- Hub side: `GET /api/internal/khala-sync/hub/log?scope=…&cursor=…` (admin
  bearer) should serve the freshly pushed versions from the DO window.
- Process side: launchd stdout/err logs (`__LOG_DIR__` in the plist);
  `launchctl print gui/$(id -u)/com.openagents.khala-sync-capture`.

`src/capture.test.ts` proves the loop against real local Postgres + a fake
hub (Bun.serve replica of the DO /append contract): single-pass push +
checkpoint advance, restart resume with zero hub duplicates, 409-gap
healing from the hub's expectation, per-scope failure isolation,
version-group integrity at `batchVersions: 1`, NOTIFY-wake promptness with
the poll fallback idle, and clean daemon shutdown.

## Compaction (KS-2.3)

`src/compaction.ts` implements the scheduled retention job from SPEC §4:
advance each scope's retained-window watermark
(`khala_sync_scopes.retained_from_version`) and prune
`khala_sync_changelog` behind it. Tombstone GC falls out of the same pass
(SPEC §2.3: "compaction prunes both log and tombstones behind the retained
window").

### Semantics

- **One transaction per scope.** `compactScope(sql, { scope,
  maxRetainedEntries, maxRetainedAgeMs?, now?, dryRun? })` takes the
  scope-counter row lock (`FOR UPDATE` — the same lock version allocation
  takes), computes the new watermark, updates `retained_from_version`, and
  deletes the compactable rows — atomically. A failure rolls back the
  watermark AND the deletions together.
- **The watermark is the MINIMUM of three candidates** (each one a
  retention guarantee; an entry is compactable only once it clears ALL of
  them):
  1. *Entry count* — `max(1, last_version - maxRetainedEntries + 1)`:
     always keep the newest N version groups.
  2. *Age* — the smallest version whose `committed_at` is younger than
     `now - maxRetainedAgeMs`: entries under the age floor are never
     compacted (skipped when `maxRetainedAgeMs` is not configured).
  3. *Capture checkpoint* — `pushed_through_version + 1` from
     `khala_sync_capture_checkpoints`: never delete rows the KS-4 capture
     worker has not pushed to the per-scope hub. The bound is guarded by a
     table-existence check (`to_regclass`) because the capture lane lands
     concurrently: no table → no capture worker to protect → bound skipped;
     table present but no row for the scope → **fail closed** (treat
     `pushed_through_version` as 0, watermark held at 1).

  The result is clamped to never regress and never exceed
  `last_version + 1`, so the `khala_sync_scopes_retention` CHECK constraint
  holds by construction.
- **The watermark never splits a version group.** It is a version-boundary
  number: every group at `version >= retained_from_version` stays complete,
  which is what keeps `logPage`'s never-split-a-version contract intact.
- **Live entities' latest upsert rows are preserved** even behind the
  watermark. Bootstrap derives current entity states from the changelog
  (latest row per entity — see the read-service section), so compaction
  only deletes rows that are *superseded* (a newer row exists for the same
  entity) or *tombstones* (tombstone GC — a tombstone behind the watermark
  is either superseded or marks an entity that bootstrap omits anyway).
  Preserved rows are snapshot residue: they feed bootstrap but are never
  served as log pages (`logPage` refuses cursors behind the window).
- **`compactAll(sql, config)`** discovers compactable scopes with ONE cheap
  query (`GREATEST(1, last_version - N + 1) > retained_from_version` — the
  age/checkpoint bounds only ever hold the watermark back further, so the
  predicate is exact) and compacts each in its own transaction with
  per-scope error isolation; the summary reports results and failures.

### MustRefetch follows from the watermark (SPEC §7 invariant 6)

Compaction never talks to clients — advancing the watermark is the whole
mechanism. The read service already fails closed for any log cursor,
bootstrap stitch point, or bootstrap page token behind
`retained_from_version` (typed
`KhalaSyncCursorBehindRetainedWindowError`), the HTTP layer maps that to
`MustRefetch(cursor_behind_retained_window)`, and the hub DO's catch-up
endpoint returns `410 Gone` with the same code for cursors behind its
window. The client clears scope-local state and re-bootstraps; the seam
stays exact because the fresh bootstrap + stitch converges to head state.

### Compaction runbook

`scripts/compact.ts` is the cron/Cloud Scheduler entrypoint. Like
migrations, it runs over a **direct** Postgres connection (never
Hyperdrive):

```sh
# Plan only — prints the per-scope watermark move, binding bound, and
# would-delete counts; writes nothing:
KHALA_SYNC_DATABASE_URL="postgres://user@host:5432/db" \
  bun run --cwd packages/khala-sync-server compact -- --dry-run \
  --max-retained-entries 10000 --max-retained-age-ms 86400000

# Apply (same flags; exit code 1 if any scope failed):
KHALA_SYNC_DATABASE_URL="postgres://user@host:5432/db" \
  bun run --cwd packages/khala-sync-server compact -- \
  --max-retained-entries 10000 --max-retained-age-ms 86400000
```

Schedule guidance: run it periodically (daily is plenty at current write
volumes; hourly once fleets push sustained load) as the `khala_migrate`
role or a dedicated `khala_compact` role with DELETE on
`khala_sync_changelog` and UPDATE on `khala_sync_scopes`. Keep
`--max-retained-entries` comfortably larger than the hub DO's in-memory
window and the longest expected client offline gap; `--max-retained-age-ms`
(e.g. 24h) is the safety floor that guarantees a client offline for less
than that never gets force-refetched. Runs are idempotent — a rerun after a
partial failure just re-plans from the current watermark.

`src/compaction.test.ts` proves the semantics against real local Postgres:
entry-count windows keep exactly the newest N version groups; the age and
capture-checkpoint bounds hold the watermark back (including the fail-closed
missing-checkpoint-row case); version groups never split; tombstone GC;
preserved latest-upsert rows keep bootstrap whole; the post-compaction
MustRefetch chain (stale log cursors AND stale bootstrap page tokens fail
typed, fresh bootstrap + stitch converges); per-scope failure isolation with
atomic rollback; dry-run parity; and a seeded random write/compact
interleaving loop asserting the retention constraint and window invariants
never break.

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
