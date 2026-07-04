# Khala Sync — Specification v0.1

**Status:** Approved direction (owner, 2026-07-04): Cloud SQL Postgres via
Hyperdrive, no intermediate D1 steps — build the full engine.
**Design rationale:** `docs/fable/2026-07-04-database-alternatives-and-postgres-sync-engine.md` (§4).
**Naming:** always the two-word compound **Khala Sync** (`khala-sync`,
`khala_sync_*`). Bare "Khala" is the collective-intelligence product
(Episode 242) and is never used for this engine.

Khala Sync is the owned replication substrate that carries scoped state
between the authoritative Postgres store and every client surface (Khala
Code desktop, web, mobile, Worker-internal projections). It replaces ad hoc
polling loops and live-at-read aggregates with one typed, verified,
offset-resumable log.

## 1. System shape

```
Cloud SQL Postgres (HA)          Cloudflare edge                    Clients
─ business tables                ─ openagents.com Worker            ─ desktop / web / mobile
─ khala_sync_changelog              (mutator + read routes,         ─ local SQLite store
─ khala_sync_scopes                 via Hyperdrive)                 ─ optimistic apply + rebase
─ khala_sync_mutations           ─ KhalaSyncHubDO (per scope)       ─ cursor checkpointing
─ khala_sync_client_state        ─ capture worker (direct PG)
```

Five components, five packages/homes:

| Component | Home | Role |
|---|---|---|
| Contracts | `packages/khala-sync` | Effect Schema types for scopes, versions, changelog entries, mutations, wire protocol, errors. The single source of truth; server and clients depend on it. |
| Server substrate | `packages/khala-sync-server` | Postgres schema + outbox writer + version allocator + mutator engine + bootstrap/catch-up queries + compaction. Runs inside the `openagents.com` Worker via Hyperdrive. |
| Hub delivery | `packages/khala-sync-server` (DO class) | `KhalaSyncHubDO`: one Durable Object per scope; recent log window in DO SQLite; hibernating WebSockets; offset-resumable HTTP catch-up. |
| Capture | `packages/khala-sync-server` | Tail `khala_sync_changelog` (direct Postgres connection — NOT Hyperdrive, which drops LISTEN/NOTIFY) and push frames to scope hubs. |
| Client engine | `packages/khala-sync-client` | Local store (SQLite), transport, bootstrap/catch-up/live state machine, optimistic mutators, rebase, cursor persistence. |

## 2. Core model

### 2.1 Scopes

A **scope** is the unit of sync, authorization, and fan-out. Scope ids are
structured strings compatible with the existing `@openagentsinc/sync-worker`
taxonomy:

```
scope.user.<userId>            personal workroom
scope.team.<teamId>            team
scope.agent_run.<runId>        one agent run
scope.thread.<threadId>        one thread
scope.fleet_run.<fleetRunId>   one fleet run (cockpit)
scope.public.<channel>         public projections (e.g. tokens-served)
```

Scope-selection predicates are restricted to index-able classes (exact key
match) so change→scope routing is an index lookup, never a scan.

### 2.2 Versions and cursors

- Every scope has a **monotonic version** (`bigint`, starts at 1) allocated
  by the server **inside the writing transaction** under a row lock on the
  scope's counter row (`khala_sync_scopes.last_version`). Ordering is
  correct by construction; the outbox sequence-gap trap cannot occur.
- A **cursor** is `(scope, version)`. Clients persist cursors durably and
  resume from them. Delivery is at-least-once; **apply must be idempotent**
  (entries keyed by `(scope, version)`).

### 2.3 Changelog entries

One row per changed entity per transaction per scope
(`khala_sync_changelog`): scope, version, entity type, entity id, op
(`upsert | delete`), the **full post-image** as canonical JSON (v1 choice:
post-image, not diffs — simpler rebase, self-healing), tombstone flag,
mutation ref, committed-at. Deletes are soft (tombstone entries) so they
replicate; compaction prunes both log and tombstones behind the retained
window.

### 2.4 Mutations

Writes are **named, server-authoritative mutators** (Replicache/Zero/Linear
model), never raw row writes and never CRDT merge (Yjs blobs may ride as
opaque field values):

- Client: applies the mutator optimistically to the local store **in memory
  overlay only** (the durable local DB holds server-confirmed state only),
  records it in a FIFO push queue with a per-client sequential
  `mutationId`, pushes in batches.
- Server: executes its own implementation of the mutator in one Postgres
  transaction — permission check, validation, business write, changelog
  append(s), `khala_sync_mutations` upsert (per-client `lastMutationId` +
  result) — all atomic. Idempotent by `(clientGroupId, mutationId)`.
- Acceptance rules (imported from PowerSync scar tissue): acceptance is
  synchronous with the transaction; **validation failures ack the mutation
  and report the error in-band** — they never 4xx/block the queue.
- Rebase: on every delta the client rewinds its overlay to confirmed state,
  applies new entries, re-applies still-unconfirmed mutations, reveals
  atomically. Mutators must be replay-safe; server outcome wins.
- v1 offline contract: **online-optimistic** — reads work offline, pushes
  wait for connectivity; mutations queued while offline are bounded and
  expire honestly.

## 3. Wire protocol

All frames are Effect Schema types in `packages/khala-sync`
(`protocolVersion: 1` rides on every request/response).

- `POST /api/sync/push` — `PushRequest { clientGroupId, clientId, scope?,
  mutations: [{ mutationId, name, argsJson }] }` →
  `PushResponse { results: [{ mutationId, status: applied | rejected |
  duplicate, errorCode?, errorMessageSafe? }] }`.
- `POST /api/sync/bootstrap` — `BootstrapRequest { scope, schemaVersion }` →
  snapshot pages (entities at a consistent point) + `cursor` at which the
  snapshot was taken. The client stitches: apply snapshot, then catch up
  **from exactly that cursor**.
- `GET /api/sync/log?scope=…&cursor=…&limit=…` — offset-resumable catch-up:
  `LogPage { entries, nextCursor, upToDate }`. Cacheable; served from the
  hub's DO SQLite window when possible, from Postgres otherwise.
- `WS /api/sync/connect?scope=…&cursor=…` — live tail. Frames:
  `DeltaFrame { scope, entries, cursor }`,
  `MutationAck { clientId, lastMutationId }`,
  `MustRefetch { scope, reason }`, `Ping`.
- `MustRefetch` is a first-class protocol citizen: compaction passed the
  client's cursor, schema version unsupported, or scope membership changed
  → client clears scope-local state and re-bootstraps. Never guess.

Auth: requests carry the normal OpenAgents auth; the server resolves scope
access on every bootstrap/connect and re-checks on membership change
(revocation ⇒ hub sends `MustRefetch(reason: access_changed)` and the
re-bootstrap returns nothing). Gatekeeper-style scope tokens (JWT embedding
the authorized scope) are the v2 fast path.

## 4. Postgres substrate

Schema (`packages/khala-sync-server/migrations/`):

```sql
khala_sync_scopes        (scope PK, last_version, retained_from_version, updated_at)
khala_sync_changelog     (scope, version, entity_type, entity_id, op,
                          post_image_json, mutation_ref, committed_at,
                          PRIMARY KEY (scope, version, entity_type, entity_id))
khala_sync_mutations     (client_group_id, client_id, mutation_id, name,
                          status, result_json, scope, committed_at,
                          PRIMARY KEY (client_group_id, client_id, mutation_id))
khala_sync_client_state  (client_group_id PK, user_id, schema_version,
                          last_seen_at)
```

- Version allocation: `UPDATE khala_sync_scopes SET last_version =
  last_version + 1 WHERE scope = $1 RETURNING last_version` inside the
  mutator transaction (insert-on-conflict bootstrap for new scopes).
- Bootstrap snapshot: per-entity-type queries at a single transaction
  snapshot, paired with the scope version read in the same transaction.
- Compaction: scheduled job advances `retained_from_version` (window: max
  entries or age), deletes older changelog rows; hubs relay `MustRefetch`
  to any cursor behind the window.
- Capture: v1 tails `khala_sync_changelog` (`WHERE (scope, version) >
  last-pushed`, LISTEN/NOTIFY wake + short poll fallback) over a direct
  Postgres connection. The WAL/pgoutput upgrade (PG17 failover slots) swaps
  only this component; the log contract does not change.

Connectivity: the Worker reaches Cloud SQL through **Hyperdrive**
(transaction-mode pooling; no LISTEN/NOTIFY, no session state — mutators
must be single-transaction). The capture worker and migrations use a direct
connection path.

## 5. Hub Durable Object

`KhalaSyncHubDO` (one per scope, `idFromName(scope)`):

- Holds the recent log window in DO SQLite (`entries(version, payload)`,
  bounded count/bytes); appends arrive from capture (or same-Worker push
  after commit as the fast path).
- WebSockets via the Hibernation API; per-socket cursor in
  `serializeAttachment`. On append: fan out `DeltaFrame` to sockets at the
  window edge; sockets behind the window get `MustRefetch`.
- Serves `GET log` pages from its window; falls through to Postgres for
  older-but-retained ranges.
- The DO is a **cache and fan-out layer only** — Postgres is authoritative;
  a reset DO re-hydrates from Postgres. No business writes originate in
  the hub.

## 6. Client engine

- Local store: SQLite (`bun:sqlite` on desktop; SQLite-WASM/`opfs-sahpool`
  with a SharedWorker single-writer on web — later lane). Tables:
  `entities(scope, entity_type, entity_id, post_image_json, version)`,
  `cursors(scope, version)`, `pending_mutations(mutation_id, name, args,
  state)`, `meta(schema_version, client_id, client_group_id)`.
- State machine per scope: `idle → bootstrapping → catching_up → live`,
  with `must_refetch` from any state. Reconnect = resume from durable
  cursor; the cursor, not the connection, is the source of truth.
- Read API: typed queries over confirmed state + optimistic overlay, with
  change subscription for UI (Effect Stream / signal adapter).

## 7. Invariants (to register in the owning INVARIANTS.md when the engine
carries production surfaces)

1. Per-scope versions are dense, monotonic, and server-assigned in-txn.
2. A client never persists optimistic effects into its durable store.
3. Every changelog entry is attributable to a mutation ref (or a named
   system writer).
4. Delivery is at-least-once; apply is idempotent by `(scope, version,
   entity)`.
5. Mutation execution and changelog append are one Postgres transaction.
6. A cursor behind the retained window MUST receive `MustRefetch`, never a
   partial log.
7. Scope access is checked at bootstrap/connect and re-checked on
   membership change; revocation retracts synced state via re-bootstrap.
8. Public-scope projections (e.g. tokens-served) reconcile to exact source
   rows — the sync path never invents counter deltas.
9. Raw private material (prompts, tokens, wallet, local paths) never enters
   changelog post-images for scopes broader than the owner.

## 8. Verification plan

- Contracts: schema round-trip + property tests (`packages/khala-sync`).
- Rebase correctness: property/model-based tests — random mutation
  interleavings; invariant: client converges to server state, no
  optimistic residue.
- Stitching: bootstrap-vs-log seam tests (snapshot at v, entries (v, v+k]).
- Behavior contracts: register owner-stated expectations
  (`packages/behavior-contracts`) for indicator truthfulness of synced
  surfaces.
- Load: fleet-burst simulation (N workers × mutators/sec) against staging
  Cloud SQL before any production cutover.

## 9. Issue map

Epic: [#8282](https://github.com/OpenAgentsInc/openagents/issues/8282).
Workstream issues KS-0 … KS-9 are #8283–#8312; the live table is in
`docs/khala-sync/README.md`.
