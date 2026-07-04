# Database Alternatives and an Owned Postgres Sync Engine

**Date:** 2026-07-04
**Lane:** Fable synthesis (docs/fable)
**Trigger:** Recurring `D1 DB is overloaded` errors during internal-only fleet
runs, plus the owner's direction to explore traditional databases on our
Google Cloud infrastructure (where a **$70,000 credit** is available) and to
design a full **owned sync engine on Postgres** that is fully in our control.

This doc is grounded in four research passes run 2026-07-04: a repo-wide D1
usage audit of `apps/openagents.com/workers/api`, a verified survey of D1's
architecture/limits/error classes, a costed survey of GCP database options
against the $70k credit, and a primary-source survey of the 2025–2026 sync
engine landscape plus concrete build guidance. Repo citations are to current
`main`.

---

## 0. Executive summary

1. **The D1 overload is structural, not bad luck.** One D1 database
   (`openagents-autopilot`) is a **single-threaded SQLite instance inside a
   single Durable Object**. Its throughput is roughly `1 / average query
   duration`, requests queue behind the single writer, and a full queue
   returns exactly the error we see: `D1 DB is overloaded. Requests queued
   for too long.` Our workload — hundreds of unbatched per-item writes per
   Codex turn, a 4-statement ledger batch per token event maintained across
   13+ indexes, a 25-task cron every minute, and an **uncached full-table
   `SUM()`** on the public counter — is the canonical overload profile.
   Community reports show this error at far smaller scale than ours.
2. **We can buy immediate relief without moving anything** (§2): cache the
   counter read against rollups, batch/queue the raw-event-chunk writes, add
   bounded retry-with-jitter on transient D1 errors, and move heartbeat
   telemetry off the relational path. These are days of work and they matter
   even if we migrate, because they fix write amplification we would
   otherwise carry to Postgres.
3. **For the primary database, Cloud SQL for PostgreSQL is the right GCP
   target.** It is real Postgres with confirmed logical replication
   (`pgoutput` + `wal2json`) — the exact primitive an owned sync engine
   needs. An 8 vCPU / 52 GB regional-HA instance is ~$1,119/mo list, i.e.
   **~5 years of runway on the $70k credit**; the 4 vCPU tier is ~9.5 years.
   Cloudflare Workers reach it through **Hyperdrive** (edge connection
   pooling; ~1 RTT per uncached query). AlloyDB is the upgrade path; Spanner
   is ruled out (its Postgres interface has **no logical decoding**, which
   kills the sync engine); self-managed Postgres on GCE saves money we don't
   need to save while the credit pays the bill.
4. **We should build the sync engine ourselves, and we are not starting from
   zero.** `apps/openagents.com/packages/sync-worker` already implements the
   embryo: per-scope monotonic sequences (`sync_scopes.last_seq`), a
   `sync_outbox` mutation table, cursored snapshots, and
   `SyncRoomDurableObject` fan-out. §4 gives the full design — transactional
   outbox changelog, per-scope server-assigned versions, named
   server-authoritative mutators with client rebase, one hibernating
   WebSocket Durable Object per scope as the sync hub, SQLite client stores —
   and the graduation path to WAL/logical-replication capture on Postgres 17.
   The convergent architecture across Linear, Figma, Replicache/Zero,
   PowerSync, and LiveStore is well documented; the components are
   individually specified; a minimal viable tier is weeks-to-months, not
   years.
5. **Recommended end state** (§5): Postgres (Cloud SQL HA) as the
   authoritative relational core reached via Hyperdrive; the owned sync
   engine ("the Khala," fittingly) projecting scoped state to desktop/web/
   mobile clients through per-scope Durable Objects; high-volume telemetry
   (raw event chunks, heartbeats, stress burns) **off** the relational
   database entirely (R2 + Analytics Engine + DO buffers); hot public
   counters served from sync projections instead of live aggregates. D1
   remains only as a bounded edge cache/staging layer, or is retired from
   the hot path altogether.

---

## 1. Why we see "D1 overloaded" with zero outside users

### 1.1 What the error actually is

Each D1 database is backed by **one Durable Object** running SQLite **in the
same thread** — "each individual D1 database is inherently single-threaded,
and processes queries one at a time" (Cloudflare D1 limits doc). Cloudflare's
own math: at 1 ms average query time you get ~1,000 queries/sec; at 100 ms,
10/sec. Excess concurrent queries are queued; when the queue fills or waits
too long, D1 returns:

- `D1 DB is overloaded. Too many requests queued.`
- `D1 DB is overloaded. Requests queued for too long.`

Official guidance for these two is **not** "retry" — it is "optimize the
queries, spread the load, or shard." (The adjacent Durable Objects doc is
explicit that `.overloaded` errors should not be retried because retrying
worsens the overload.) The retryable class is different: `Network connection
lost.`, `storage caused object to be reset`, `D1 DB reset because its code
was updated.` Since 2025-09 D1 auto-retries **read-only** statements up to 2
times on those; **writes are never auto-retried**.

Other structural facts that matter for us:

- **10 GB max database size** on paid plans. Our single DB carries ~330+
  live tables including an append-forever token ledger and raw-event-chunk
  metadata; we will eventually hit this wall even if overload never bites.
- **The primary lives in one region**; every write from every Worker isolate
  worldwide serializes into it.
- **Read replication (Sessions API, beta)** offloads reads only, and only
  for queries routed through `withSession()` — our code uses plain
  `prepare()` everywhere, so replication as-configured would change nothing.
- Cloudflare's scaling posture for D1 is explicit: **scale out across many
  small per-tenant/per-entity databases**, not up. A single monolith DB
  shared by ~30 product surfaces is the anti-pattern.
- Independent benchmarking found ~**280 unbatched INSERTs/sec** before
  latency degrades non-linearly (single source), and community reports show
  overload on a **443-row** database with a chatty auth flow, an idle 40 MB
  database with one indexed SELECT per minute, and per-minute-cron workloads
  — i.e., "internal-only scale" does not protect you when the write pattern
  is chatty. There are also acknowledged platform-side D1 incidents
  (Jan 2025, and a multi-day 2026 long-tail-latency degradation acknowledged
  by Cloudflare engineering leadership), so some blips are not our fault at
  all — which is itself an argument for owning the primary store.

### 1.2 Our workload is the canonical overload profile (repo audit)

Everything except the Verse world runs through **one** D1 database:
`openagents-autopilot` (`apps/openagents.com/workers/api/wrangler.jsonc:305`,
binding `OPENAGENTS_DB`). 295 migration files, ~330+ live logical tables
across `pylon_*`, `token_usage_*`, `agent_*`, `artanis_*`, `forum_*`,
`site_*`, `crm_*`, `billing_*`, `inference_*`, `sync_*`, and more. The
measured hot paths:

**Write amplifiers, ranked:**

1. **Raw event chunks, per item, unbatched.** The Pylon executor flushes an
   event chunk on **every Codex `item.completed`**
   (`apps/pylon/src/codex-agent-executor.ts:1134`); the server store does a
   dedupe SELECT + R2 put + `INSERT OR IGNORE` per chunk
   (`pylon-codex-turn-ingest-routes.ts:1231-1332`). A turn that runs 30
   commands ⇒ ~60+ D1 statements, × N parallel fleet workers. This is a
   classic N+1 write loop on the request path — no queue, no batching.
2. **Token ledger fan-out.** Every token event is a 4-statement batch
   (insert + 3 rollup upserts, `token-usage-ledger.ts:1736-1790`) preceded by
   a dedupe SELECT, into a table carrying **13+ secondary indexes**
   (migrations 0137/0232/0262/0263/0269) — every index is maintained on every
   insert. Full cost of one `/api/pylon/codex/turns` ingest ≈ **10–11 D1
   statements** (auth SELECT, assignment SELECT, ledger batch, raw-events
   SELECT+INSERT, trace SELECT+INSERT).
3. **A second independent insert path:** every public/free chat completion
   writes a `token_usage_events` row on the serving path
   (`public-khala-chat-served-tokens.ts:132-170`).
4. **A 25-task cron every minute** (`triggers.crons: ["* * * * *"]`,
   handler `index.ts:14674`): GLM pool heartbeat alone ingests one ledger
   event per replica per minute (~40 statements/min at 10 replicas), plus
   serving-rate monitor, fleet-burn stall detector, Artanis ticks, capacity
   funnel snapshots, treasury reconciles — a fixed contention floor under
   everything.
5. **Presence heartbeats:** SELECT + wide full-row UPDATE with ~10
   JSON-serialized columns per pylon per heartbeat (`pylon-api.ts:2907-3002`).

**Read amplifiers, ranked:**

1. **`GET /api/public/khala-tokens-served` runs an unbounded
   `SELECT SUM(CASE…) FROM token_usage_events WHERE 1=1`** — a full
   table/index aggregate over the entire ever-growing ledger, explicitly
   live-at-read with `no-store` (`token-usage-ledger.ts:2949-2966`, `:312`).
   Homepage, CLI, and every runbook before/after check re-scan the table.
   One slow aggregate stalls the whole single-threaded queue — including
   assignment accepts.
2. History/model-mix/channel-mix raw fallbacks scan the ledger when rollups
   don't cover the requested window (`token-usage-ledger.ts:1443,1542,2996`).
3. Per-minute monitor aggregates over the same table (serving-rate monitor,
   stall detector).
4. Dedupe SELECT before nearly every hot write (~2× statement count).
5. Closeout/proof scans over `pylon_codex_raw_event*` and assignment/event
   tables during fleet supervision.

**And the kicker: there is no D1 retry anywhere.** `d1Effect` is a bare
`Effect.tryPromise` (`token-usage-ledger.ts:1119-1126`) and the ~49 other
local `d1Effect` wrappers are the same shape. Every "overloaded"/retryable
handler in the repo is for upstream inference providers, not D1. A transient
blip surfaces instantly as `pylon_api_storage_error` — which is exactly what
the fleet felt. **No Cache API or KV sits in front of any D1 read.** The
Queues bindings exist (`RUNNER_EVENTS`, adjutant, inference-batch) but none
buffer the hot writes; the two active consumers are `max_batch_size: 1`.

### 1.3 We already lived the failure (June 28–29 after-action)

`docs/afteraction/2026-06-29-codex-fleet-throughput-collapse-after-action.md`
documents the full cascade with the exact error string: simultaneous
assignment-accept bursts produced `pylon_api_storage_error` / `D1 DB is
overloaded. Requests queued for too long.`; 16 parallel Vertex burn loops
writing per-completion ledger rows **starved the codex dispatch gate's D1
reads** (503 "could not read linked owner registration", 500s, even 401s on
presence); staggering accepts by ~2 seconds was the workaround; a
retry-transient-accepts patch and a 2-second public-stats cache were the
local fixes. The Cloudflare dashboard spike the owner saw was confirmed to be
this same database id (`9644ea09-…`). The after-action's own P0 list —
supervisor fast-retry (#6987), server-side gate D1-read resilience, bounded
burns — is the operational echo of the structural problem this report
addresses at the root: **dispatch control-plane reads, the token ledger,
telemetry firehose, forum, CRM, and public counters all contend for one
single-writer SQLite queue.**

---

## 2. Immediate mitigations on Cloudflare (do these regardless of migration)

Ordered by leverage-per-effort. Items 1–3 are each roughly a day.

1. **Serve the public counter from rollups + cache, kill the full-table
   SUM.** Maintain a single running-total row (or read the existing daily
   rollups + today's delta), snapshot to KV/Cache with a 2s TTL, and declare
   the staleness honestly in the promise copy (the after-action already
   started this; owner approved 2s). The `SYNC_ROOM` push path already
   handles live homepage movement — the scalar read does not need to scan
   the ledger.
2. **Queue + batch the raw-event-chunk writes.** Producer: the ingest route
   enqueues chunk metadata (R2 put can stay synchronous). Consumer: drain in
   ≤100-message batches into one `db.batch()` insert. Queues take 5,000
   msg/s with 25 GB backlog — this converts our chattiest N+1 loop into a
   handful of batched statements per second. The `RUNNER_EVENTS` producer
   pattern already exists in the codebase.
3. **Wrap `d1Effect` in bounded `Effect.retry` with exponential backoff +
   jitter** for the transient class (`Network connection lost`, `storage
   caused object to be reset`, `reset because its code was updated`) on
   idempotent statements — our writes are idempotency-keyed, so they
   qualify. For `overloaded` specifically: at most 1–2 retries with a long
   jittered delay, plus a metric/alarm, since official guidance is load
   reduction, not retry. One shared helper in one place; all ~50 `d1Effect`
   call sites inherit it.
4. **Move heartbeat/monitor telemetry off the relational path.** GLM replica
   heartbeats and capacity snapshots are time-series telemetry, not ledger
   truth — Workers Analytics Engine (`writeDataPoint()` is non-blocking,
   absorbs unbounded rates by sampling) or a DO buffer that flushes 1-minute
   summaries. This also aligns with our "prefer Cloudflare primitives"
   stance. The exact-token ledger stays exact — only ops telemetry moves.
5. **Split the one D1 into 2–3 by workload class** (ledger+dispatch /
   telemetry metadata / forum+CRM+sites) so a burn can no longer starve the
   dispatch gate. Cheap insurance, buys time, but it is triage — the 10 GB
   ceiling and single-writer-per-DB physics remain.
6. **Trim `token_usage_events` indexes.** Audit the 13+ secondary indexes
   against actual query patterns; every dropped index is a permanent write
   discount.

These mitigations are worth doing even on the way out the door: they
eliminate write amplification and hot-read patterns that would degrade
**any** database, including the Postgres we move to.

---

## 3. Database alternatives

### 3.1 Stay-on-Cloudflare options (for completeness)

- **Shard D1 per-entity** (Cloudflare's intended model, 50k DBs/account):
  fixes contention but multiplies migration/ops surface across 330 tables
  and forfeits cross-surface SQL (ledger ⋈ assignments ⋈ forum), which we
  use constantly for proofs and reconciliation. Poor fit for our relational
  core.
- **Durable Object SQLite per hot entity** (per-assignment event
  accumulator, per-agent session store): genuinely excellent for
  write-local, read-local state — microsecond same-thread queries, 10 GB per
  object — and it features in the sync-engine design (§4). Not a home for
  the global relational core.
- **Analytics Engine** for telemetry: yes (already in §2).
- Verdict: Cloudflare primitives remain our edge/delivery layer, but the
  **authoritative relational core has outgrown the D1 model.** The owner's
  instinct to move it to a traditional database is correct.

### 3.2 GCP options against the $70k credit

All prices are list, us-central1, verified against official pricing pages
2026-07-04; re-verify in the GCP calculator before committing. Note credits
typically carry an expiration (commonly 12–24 months) — **check our credit's
terms first**, because runway beyond expiry is theoretical, and committed-use
discounts rarely make sense against expiring credits.

| Option | Config | Est. $/mo (list) | Months on $70k | Sync-engine fit (logical decoding) |
|---|---|---|---|---|
| **Cloud SQL Postgres (Enterprise, HA)** | 4 vCPU / 26 GB + 250 GB SSD | ~$612 | ~114 | ✅ `pgoutput` + `wal2json` confirmed |
| **Cloud SQL Postgres (Enterprise, HA)** | 8 vCPU / 52 GB + 250 GB | ~$1,119 | ~62 | ✅ |
| **Cloud SQL Postgres (Enterprise, HA)** | 16 vCPU / 104 GB + 250 GB | ~$2,133 | ~33 | ✅ |
| Cloud SQL 8 vCPU HA + 1 read replica | | ~$1,690 | ~41 | ✅ |
| **AlloyDB (HA)** | 4 vCPU / 32 GB | ~$915 | ~76 | ✅ now supported (verify slot survival across failover — undocumented) |
| Spanner (Enterprise, 1 node) | + 250 GB | ~$793 | ~88 | ❌ **no WAL/logical decoding; PG interface is dialect-only** |
| GCE self-managed (single n2-highmem-8 + PITR) | 8 vCPU / 64 GB + 500 GB | ~$483 | ~145 | ✅ unrestricted, plus any extension |
| GCE Patroni HA pair + witness | 2× n2-highmem-8 | ~$968 | ~72 | ✅ + etcd/ops burden |
| Firestore / Bigtable | — | — | — | ❌ not relational; no SQL joins / no general ACID; wrong shape |

Key findings per option:

- **Cloud SQL for PostgreSQL — the recommendation.** Real Postgres. Logical
  replication is enabled with the flag `cloudsql.logical_decoding=on`;
  officially supported decoders are `pgoutput`, `wal2json` (v1/v2), and
  `test_decoding`; you create publications/slots with a `REPLICATION IN ROLE
  cloudsqlsuperuser` user — the identical path Datastream and every CDC
  engine uses. HA is synchronous cross-zone with ~60s failover (note: the
  standby serves **no** reads; read capacity = replicas at full instance
  price). `max_connections` defaults scale with RAM (500–800 in our range).
  Managed connection pooling requires Enterprise Plus (~+30%); on Enterprise
  we lean on Hyperdrive's pooling, which we want anyway. Caveat for later:
  HA instances can't be logical **subscribers** (irrelevant — we publish).
- **AlloyDB** — contrary to its historical reputation, now supports logical
  decoding (`alloydb.logical_decoding` flag; `wal2json`/`pglogical` in the
  extension list; documented as publisher to external subscribers). Adds a
  columnar engine and up-to-20-node read pools with a 99.99%
  maintenance-inclusive SLA. ~60% pricier per vCPU than Cloud SQL. The right
  **second** move if in-DB analytics or read scaling bite; the open risk is
  that replication-slot survival across HA failover is undocumented — test
  before trusting the sync engine to it.
- **Spanner** — ruled out for this program. The PostgreSQL interface is
  dialect compatibility, not Postgres: no extensions, no replication tools,
  no WAL, no logical decoding (CDC = change streams via
  Dataflow/Datastream), no monotonic sequences, PGAdapter sidecar required.
  Buy Spanner for multi-region five-nines or >10 TiB OLTP; we have neither
  problem, and it breaks the sync engine's capture layer.
- **Self-managed Postgres on GCE** — full control, any extension, and where
  our unrestricted-superuser instinct points. But while the credit pays the
  bill, self-managing saves nothing and costs real attention (Patroni +
  etcd + WAL-G + restore drills + upgrades; a credible TCO analysis prices
  steady-state ops at ~$1k/mo of engineer time — more than the infra delta
  at our size). Reserve it for the case where we need an extension Cloud SQL
  won't allow. "Fully in our control" is satisfied at the **sync-engine
  layer** (all app-visible semantics ours) without owning `initdb`.
- **Firestore/Bigtable** — wrong shape (document/wide-column; no joins/
  general relational ACID); noted only for completeness.

### 3.3 Latency reality: Workers → GCP Postgres via Hyperdrive

- **Hyperdrive** (free with Workers Paid) keeps warm pooled connections in
  the Cloudflare location nearest the origin DB, eliminating the 5–7
  round-trip TCP+TLS+auth setup per request. Cloudflare's published
  benchmark: 1,200 ms cold direct → ~500 ms pooled → ~320 ms cached; warm
  uncached queries cost ~1 RTT Worker→DB-region (docs quote 20–30 ms from a
  distant region, 1–3 ms colocated). Realistic brackets for us: **~10–60 ms
  per uncached query for US users → us-central1**, ~100–250 ms EU/APAC
  (inference from published figures, not a single benchmark).
- **Mitigations:** Smart Placement moves DB-heavy request paths next to the
  database (multi-query requests collapse to single-digit ms per query);
  Hyperdrive read caching (60s max-age / 15s SWR, cached at every edge
  location); batch statements per request; and — the deep fix — **the sync
  engine itself pushes read state to the edge**, so interactive reads never
  cross the ocean at all.
- **Plumbing constraints to design around:** transaction-mode pooling
  (~100 origin connections per config), no `LISTEN/NOTIFY`, no session
  `PREPARE`, no advisory locks through Hyperdrive; **no GCP Private Service
  Connect support** — connectivity is public IP + TLS or Cloudflare
  Tunnel / Workers VPC (Hyperdrive support added Apr 2026). The no-NOTIFY
  constraint matters for the sync engine: the capture worker that tails the
  changelog should be a **long-lived process with a direct connection**
  (Container/DO-adjacent service or a small GCE sidecar), not a per-request
  Worker through Hyperdrive.

### 3.4 The verdict on the primary database

**Cloud SQL for PostgreSQL, Enterprise edition, 8 vCPU / 52 GB regional HA,
us-central1, ~$1,119/mo (~5 years on the credit).** Rationale for sizing up
from the 4 vCPU floor: the credit makes cost a non-issue inside our decision
horizon, and headroom is precisely what we lack today; a single beefy HA
Postgres with 600 pooled connections and real concurrent writers absorbs the
entire measured workload (which one SQLite thread was carrying!) with an
order of magnitude to spare. Add a read replica (~+$560/mo) when analytics
or the sync engine's snapshot reads warrant it. Revisit AlloyDB at the point
where columnar analytics over the ledger becomes a product surface.

---

## 4. The owned sync engine

The owner's call: a full sync engine, on Postgres, built by us, fully in our
control. This section is the design. The name proposes itself: **the Khala
is the psychic link that connects all Protoss** — the sync engine is the
Khala, and `@openagentsinc/sync-worker` is its embryo.

### 4.1 What we already have

`apps/openagents.com/packages/sync-worker` (used via `sync-notifier.ts` and
`SyncRoomDurableObject`, seeded in migration `0001_openagents_sync.sql`)
already implements, on D1:

- **per-scope monotonic sequences** — `sync_scopes.last_seq` claimed with an
  atomic `INSERT … ON CONFLICT … RETURNING last_seq` (index.ts:438-456);
- **a mutation outbox** — `sync_outbox` rows keyed by mutation id, with
  scope/actor/status/result;
- **cursored snapshots** per scope (`SyncSnapshot { scope, cursor }`);
- **scope taxonomy** — personal workroom, team, agent-run, thread, public
  scopes (`sync-notifier.ts:46-57`);
- **DO fan-out** — `notifySyncScopes` wakes `SYNC_ROOM` objects per scope.

This is the same skeleton every serious engine has: a scoped, ordered
changelog with cursors and a notifier. What's missing is the client half
(local store, optimistic mutators, rebase), the delivery protocol
(resumable offset streams rather than notify-then-refetch), permission-change
fanout, compaction, and a Postgres-grade backing store. That is the build.

### 4.2 The landscape, as design reference (what to steal)

Ten systems reviewed against primary sources; full details in the research
appendix refs. The distilled takeaways:

- **ElectricSQL** (Apache-2.0, Elixir; read-path only over Postgres logical
  replication). Steal: the **HTTP, cache-shaped, offset-resumable log
  protocol** (shape handle + offset + ETags; long-poll with CDN request
  collapsing — maps beautifully onto Cloudflare's cache with a DO as
  origin); `must-refetch` as a first-class protocol message; **gatekeeper
  auth** (JWT embedding the exact authorized shape); restricting filter
  grammar to index-able predicate classes so change→subscriber matching
  stays O(1). Production proof: Trigger.dev syncs 20k updates/sec to
  browsers this way.
- **PowerSync** (FSL; service + your-backend-owns-writes). Steal the
  **consistency discipline**: checkpoints applied atomically across all
  buckets; a client with unacknowledged writes does not advance past its own
  **write checkpoint**; per-bucket checksums to detect divergence; and two
  hard-won write-path rules — backend write acceptance must be synchronous
  with the DB write, and validation failures must never poison the client's
  FIFO upload queue (ack + signal in-band).
- **Zero/Rocicorp** (Apache-2.0). Steal the **CDC topology**: Postgres
  logical replication → one single-writer SQLite replica → N view-syncer
  readers running standing queries, so sync read load never touches
  Postgres. Also its honest scoping: writes are rejected offline
  (online-optimistic only) — a legitimate, much cheaper v1 contract than
  full offline.
- **LiveStore** (Apache-2.0, **built on Effect**, by the Prisma co-founder).
  The closest artifact to our target: its `@livestore/sync-cf` provider is
  literally a Worker routing by `storeId` to a **per-store Durable Object
  persisting an event log in DO SQLite**, with WebSocket/HTTP-poll
  transports and git-like pull-rebase-push semantics. Read this source
  before writing ours.
- **Replicache** (Apache-2.0, maintenance mode) — the canonical
  **push/pull + mutation-rebase spec**, and its "backend strategies" doc is
  effectively a build roadmap: Reset → Global Version → **Per-Space
  Version** → Row-Version/CVR. Per-space version ≈ our per-scope
  `last_seq` — we are already on step 3 of their ladder.
- **Linear** (in-house exemplar): decorated TypeScript model graph,
  per-workspace IndexedDB, a **global server-assigned monotonic
  `lastSyncId`**, **sync groups** (user/team/role) scoping delta fan-out,
  transaction queue with rebase and undo, delta packets over WebSocket with
  `/sync/delta?lastSyncId=` catch-up — on **Cloud SQL Postgres**. Also the
  honest cost signal: a founder with a decade of sync experience, still
  reworking it 4+ years in.
- **Figma**: split the problem — per-document authoritative process
  (server-ordered per-property LWW, fractional indexing, "we don't need true
  CRDTs… our server is the central authority") for high-frequency state, and
  **LiveGraph** (WAL-tailing query-subscription invalidation with live
  permission checks) for relational metadata. Lesson: don't force one engine
  to serve both the canvas-grade firehose and the relational graph.
- **Automerge/Yjs**: CRDTs are the right tool for collaborative
  text/document *fields* (store Yjs blobs in Postgres columns, sync as
  opaque values), and the wrong tool for relational business data — no
  cross-row invariants, no server-enforceable permissions in a merge
  function. Every production system with a server (Figma, Linear, Zero)
  converged on server-ordered mutations.
- **SpacetimeDB / Convex**: subscription-as-query with server-computed
  deltas; Convex's coarse **invalidate-and-re-execute on recorded read
  sets** is far simpler to build than incremental view maintenance — the
  correct v1/v2 split for our query-subscription tier. Convex's "A Map of
  Sync" (nine axes) is the best requirements checklist to run our scopes
  through.

### 4.3 The Khala design

**Architecture (the convergent shape, adapted to our stack):**

```
Postgres (Cloud SQL HA)                      Cloudflare edge                Clients
┌────────────────────────┐    ┌──────────────────────────────┐   ┌──────────────────────┐
│ business tables         │    │ Khala Hub DOs (per scope)     │   │ Khala Code desktop   │
│ + khala_changelog       │───▶│  - recent log window in       │──▶│  (native SQLite)     │
│   (outbox, in-txn,      │    │    DO SQLite                  │   │ web (SQLite-WASM/    │
│    per-scope versions)  │    │  - hibernating WebSockets     │   │  opfs-sahpool)       │
│ + khala_mutations       │    │  - offset-resumable HTTP      │   │ mobile (SwiftUI +    │
│   (per-client mutation  │    │    catch-up (cacheable)       │   │  SQLite)             │
│    ids, results)        │    │ capture worker tails changelog│   │ optimistic mutators  │
└────────────────────────┘    │ (direct conn, not Hyperdrive) │   │ + rebase             │
      ▲ Worker writes via      └──────────────────────────────┘   └──────────────────────┘
      │ Hyperdrive (mutators)
```

1. **Changelog: transactional outbox first, WAL decoding later.** Every
   mutator writes its change rows **in the same Postgres transaction** as
   the business write, into `khala_changelog(scope, version, entity,
   entity_id, op, patch, tombstone, committed_at)`. The per-scope `version`
   is assigned under a row lock on the scope's counter row — ordering is
   correct **by construction**, which sidesteps the notorious outbox
   sequence-gap trap (`nextval()` doesn't roll back, so sequence order ≠
   commit order; a `max(seq)` poller permanently skips rows from
   still-committing transactions). This is exactly our existing
   `sync_scopes.last_seq` pattern, kept. Why outbox before WAL: it runs
   identically on D1 today and Postgres tomorrow (the migration can ship
   client-visible sync **before** the database moves), needs no replication
   privileges or slot operations, and gives an intentional event contract
   instead of raw table deltas. **Graduation path:** when outbox write
   amplification measurably hurts, switch capture to logical replication —
   `pgoutput` on Cloud SQL, Postgres 17 **failover slots**
   (`failover=true`, `sync_replication_slots=on`) so a failover doesn't
   orphan the stream, `max_slot_wal_keep_size` as the disk-fill kill switch,
   `REPLICA IDENTITY` defaults (not FULL), and alerts on slot lag. The app
   contract doesn't change; only the capture layer does.
2. **Write path: named server-authoritative mutators with rebase (not
   CRDTs).** Replicache/Zero/Linear model: clients invoke named mutators
   optimistically against the local store, tag them with per-client
   sequential mutation ids, and push batches; the server executes **its
   own** implementation in a transaction (validation, permissions,
   side-effects), records `lastMutationId` per client, appends to the
   changelog; on each delta the client rewinds to server state, applies the
   patch, replays unconfirmed mutations, reveals atomically. Server outcome
   wins; mutators must be replay-safe. Two rules imported from PowerSync's
   scar tissue: acceptance is synchronous with the DB transaction, and
   validation failures ack-and-signal rather than blocking the queue. Our
   Effect Schema culture is a perfect fit — every mutator is a typed,
   versioned contract in a registry, satisfying the semantic-routing and
   behavior-contract invariants by construction. **v1 offline contract:**
   online-optimistic (Zero's position) — reads work offline, writes reject;
   full offline queue-and-replay is a later, deliberate upgrade.
3. **Delivery: one hibernating Durable Object per scope.** The Khala Hub DO
   holds the recent changelog window in DO SQLite, accepts WebSockets via
   the Hibernation API (10k mostly-idle connections ≈ $10/mo hibernated),
   and serves **offset-resumable HTTP catch-up** (`GET
   /khala/log?scope=…&cursor=…`) that Cloudflare's cache can absorb
   Electric-style. The cursor, not the connection, is the source of truth —
   sockets flap, tabs suspend, and resume is always `(scope, cursor)`.
   Bootstrap = snapshot query + the cursor at which the snapshot was taken,
   stitched exactly (the classic silent-bug seam; make apply idempotent so
   at-least-once is safe). A capture worker (small always-on process with a
   **direct** Postgres connection — not through Hyperdrive, which drops
   LISTEN/NOTIFY) tails the changelog and pushes to the scope DOs. Per-DO
   throughput (~1k msg/s) caps hot-scope fan-out; shard read-replica DOs
   behind the primary DO if a scope ever runs that hot.
4. **Scopes = unit of sync, auth, and fan-out** (Linear's sync groups; our
   existing scope taxonomy). Keep scope predicates to index-able classes.
   Permission changes are a first-class event: revocation must **retract
   already-synced rows** — v1 handles membership/role change by forcing a
   scope re-bootstrap (Linear's partial bootstrap); v2 graduates to
   CVR-style read-set diffing (Replicache row-version strategy), which
   makes hard deletes and permission fanout structural.
5. **Client stores:** native SQLite on desktop (Khala Code) — schema +
   materializer layer shared with web; on web, official SQLite-WASM on the
   `opfs-sahpool` VFS (no COOP/COEP headers) with a Notion-style
   SharedWorker single-writer-tab election; `navigator.storage.persist()`
   on first write. Rich-text fields ride as Yjs blobs in ordinary columns.
6. **Compaction + escape hatch:** soft deletes with version bumps; the
   changelog window in each DO is bounded; a client whose cursor predates
   the window gets `must-refetch` → fresh bootstrap, never a guess. Schema
   version rides every push/pull; stale clients get `VersionNotSupported` →
   clear-and-rebootstrap (never in-place client data migration).
7. **Public counters become sync projections.** `khala-tokens-served` is
   just a public-scope entity whose value the ledger mutator bumps — pushed
   through the same hub, cached at the edge, reconciled against exact
   ledger rows exactly as today's invariants demand. The full-table SUM
   dies permanently.

**Failure modes to design for from day one** (each has a documented
precedent): rebase correctness (never write optimistic results to the
durable local store — Linear mutates in-memory only, the local DB gets
server-confirmed deltas); permission-change fanout (above); snapshot/stream
stitching; replication lag/backpressure (outbox converts this into ordinary
table-scan backpressure we control); log compaction vs stale cursors;
migration coordination.

**Effort calibration, honestly.** The cautionary tier is real: Linear is 4+
years of continuous investment by sync veterans; Figma's LiveGraph
re-architecture credited ~12 engineers; Zero took a dedicated expert team ~2
years to 1.0. **But we are not signing up for that tier on day one.** The
minimal-viable tier — outbox + per-scope versions (already built) +
push/pull mutators with rebase + one DO hub + SQLite clients — is composed
of individually well-specified components, and the Replicache strategy docs
plus LiveStore's `sync-cf` source are effectively build specs. For a team
that ships with our fleet cadence: **the v1 tier is weeks-to-a-couple-months
of focused lanes; the CVR/partial-sync/permission-fanout tier is where the
real months live** — schedule it as its own workstream, not a footnote. The
strategic payoff justifies it: the sync engine becomes the substrate under
Khala Code desktop (fleet cockpit state, chat, assignments), the mobile
companion's E2EE-paired projection (ROADMAP WS on the Orca adoption path),
Artanis's one-status-spine, and the QA evidence boards — one typed,
verified, owned replication layer instead of N bespoke polling loops.

---

## 5. Recommended target architecture and phased plan

**End state:** Cloud SQL Postgres HA (authoritative relational core) ⟶
Hyperdrive (Worker write/read path) ⟶ Khala sync engine (outbox → capture →
per-scope DOs → SQLite clients) ⟶ all interactive reads served at the edge
from sync projections. Telemetry firehose (raw event chunks, heartbeats,
burns) on R2 + Analytics Engine + DO buffers, never on the relational core.
D1 retired from the hot path (kept, if at all, as bounded staging).

- **Phase 0 — stop the bleeding (days).** §2 items 1–4: counter off the
  full-table SUM; raw-event chunks through a Queue; `d1Effect` retry; GLM
  heartbeats to Analytics Engine. Success metric: zero `overloaded` denials
  during a 20-worker fleet accept burst without staggering.
- **Phase 1 — stand up Postgres + Hyperdrive (week).** Provision Cloud SQL
  Enterprise HA 8 vCPU us-central1 with `cloudsql.logical_decoding=on` from
  day one; Hyperdrive config + Smart Placement on the API Worker;
  connectivity via public IP + TLS (Hyperdrive has no GCP PSC support) or
  Workers VPC. Port one bounded, high-pain domain first as the pilot — the
  **pylon assignment/dispatch tables** (the June 29 victim), dual-written
  behind a flag, then cut over. This proves latency, pooling, and migration
  tooling on a surface with exact-verification culture already in place.
- **Phase 2 — Khala sync v1 (weeks, parallel lanes).** Promote `sync-worker`
  to the outbox+mutator contract on Postgres; capture worker + per-scope Hub
  DOs (read LiveStore `sync-cf` first); desktop client store in Khala Code
  (its fleet cockpit is the ideal first consumer — it currently polls);
  public-counter projection replaces the SUM permanently. v1 contract:
  online-optimistic writes, scope re-bootstrap on permission change.
- **Phase 3 — migrate the core, domain by domain (weeks→months).** Ledger +
  traces, then forum, CRM, sites, billing — each domain: dual-write →
  backfill → verify (exact row-count/token-total reconciliation, the
  after-action's own methodology) → cut reads → drop D1 tables. Cron
  consolidation rides along (25 tasks/min re-homed onto Postgres or the
  scheduler DO).
- **Phase 4 — graduate (when metrics demand).** CVR read-set diffing for
  partial sync + permission fanout; WAL/pgoutput capture with PG17 failover
  slots if outbox amplification hurts; AlloyDB or read replicas if
  analytics/read load bite; full offline mutation queue if the product
  needs it.

Each phase lands as issues in the standard fleet-delegation pipeline
(EXECUTION.md); Phase 0 items are singles, Phase 2 decomposes cleanly into
the contracts/capture/hub/client lanes.

## 6. Owner decision points

1. **Confirm the $70k credit's expiry window** — it determines whether we
   size for comfort (8–16 vCPU HA) or runway. Recommendation stands at
   8 vCPU HA either way.
2. **Region:** us-central1 assumed (matches `openagentsgemini` footprint and
   US-centric usage). EU/APAC latency is mitigated by the sync engine, not
   by DB placement.
3. **v1 offline contract:** online-optimistic (recommended) vs full offline
   queue — changes client scope materially.
4. **Pilot domain for Phase 1:** assignments/dispatch (recommended — highest
   pain, best verification culture) vs ledger-first.
5. **Name:** this doc says **Khala sync engine**; the package would grow
   from `@openagentsinc/sync-worker`.

---

## Appendix: source highlights

- Repo ground truth: `wrangler.jsonc:305` (single D1),
  `token-usage-ledger.ts:1736-1790,2949-2966` (ledger batch, full-table
  SUM), `pylon-codex-turn-ingest-routes.ts:1231-1332` (per-chunk store),
  `apps/pylon/src/codex-agent-executor.ts:1134` (per-item flush),
  `index.ts:14674` (25-task cron),
  `apps/openagents.com/packages/sync-worker/src/index.ts` (existing engine
  embryo), `docs/afteraction/2026-06-29-codex-fleet-throughput-collapse-after-action.md`
  (the incident).
- D1: developers.cloudflare.com/d1/platform/limits, /observability/debug-d1,
  /best-practices/retry-queries, /best-practices/read-replication;
  blog.cloudflare.com/sqlite-in-durable-objects,
  /d1-read-replication-beta; community overload reports (443-row DB,
  40 MB idle DB, per-minute crons) linked in the research pass.
- Hyperdrive: developers.cloudflare.com/hyperdrive (how-it-works, limits,
  query-caching, connect-to-private-database);
  blog.cloudflare.com/how-hyperdrive-speeds-up-database-access.
- GCP: cloud.google.com/sql/pricing, /sql/docs/postgres/replication/
  configure-logical-replication (pgoutput/wal2json), /alloydb/pricing,
  docs.cloud.google.com/alloydb/docs/reference/alloydb-flags,
  /spanner/docs/postgresql-interface (no logical decoding),
  compute pricing pages; GCP calculator for re-verification.
- Sync engines: electric.ax (shapes, HTTP API, auth, benchmarks);
  docs.powersync.com (consistency, protocol, writing-client-changes);
  zero.rocicorp.dev (connecting-to-postgres, mutators, offline);
  github.com/livestorejs/livestore (sync-cf provider, event sourcing);
  doc.replicache.dev (how-it-works, strategies ladder);
  linear.app/now/scaling-the-linear-sync-engine +
  github.com/wzhudev/reverse-linear-sync-engine;
  figma.com/blog multiplayer + LiveGraph posts;
  martin.kleppmann.com CRDT-hard-parts; stack.convex.dev/a-map-of-sync;
  postgresql.org logical-decoding + PG17 logical-replication-failover;
  morling.dev replication-slots + outbox; sequinstream.com
  sequence-commit-order; notion.com WASM-SQLite post.
