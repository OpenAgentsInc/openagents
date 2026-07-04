# Khala Sync load test — fleet-burst simulation against staging (KS-9.1, #8310)

Date: 2026-07-04. Epic: #8282. Depends on KS-6.2 (#8303, closed).

Harness: `packages/khala-sync-server/scripts/load-test.ts`
(library `src/load-test.ts`, tests `src/load-test.test.ts` — config parsing,
percentile math, metrics taxonomy, and a 2-worker local-Postgres smoke that
also proves cleanup removes every synthetic row).

## What was simulated

The June 28–29 failure shape (docs/fable/
2026-07-04-database-alternatives-and-postgres-sync-engine.md §1.3 and
docs/afteraction/2026-06-29-codex-fleet-throughput-collapse-after-action.md):
~20 concurrent fleet workers, simultaneous assignment-accept bursts, per-turn
ingest of ~10–11 D1 statements, plus public-counter reads — the profile that
produced `D1 DB is overloaded. Requests queued for too long.`, 503s on the
dispatch gate, and even 401s on presence.

This run drove **2× that shape** at the Khala Sync substrate on staging
Cloud SQL (`khala_sync_staging`, instance `khala-sync-pg`, PG17,
`us-central1`), direct over TLS from the owner Mac:

- **40 concurrent writers** (2× the ~20-worker June peak), each its own
  client group pushing `sync.debugEcho`-shaped batches into its own personal
  scope through the real `executePush` engine — one Postgres transaction per
  mutation envelope (client-state row lock → idempotency gate → mutator
  changelog append → ledger record), exactly the Worker's transaction shape.
- **Batch 2 mutations/push ⇒ ~12–14 statements per push**, matching the
  June per-turn ingest statement count (~10–11) with margin.
- Target 2 pushes/sec/worker, paced closed-loop, sustained **5.5 minutes**.
- **10 concurrent readers**: log catch-up (`logPage`) + periodic `bootstrap`
  snapshot pages + a public-counter-style single-row read every second
  (the KS-6.3/#8304 `scope.public.tokens-served` projection has not landed
  on staging yet, so the counter read is a single-row `khala_sync_scopes`
  read — same shape, noted by the harness).
- Connection pool 52 (postgres.js, `prepare: false` to mirror Hyperdrive's
  transaction-mode discipline). The harness refuses pools above 25% of the
  server's `max_connections` (600 here; staging and prod share the
  instance).

## Headline numbers (staging substrate run, runId `ks9-1-main`)

Started 2026-07-04T14:31:11Z, duration 331.5s, 40 writers × batch 2 + 10
readers. **Errors: zero, across every operation class.**

| op | n | err | p50 | p95 | p99 | max | mean | rate |
|---|---|---|---|---|---|---|---|---|
| push (2 mutations) | 8,362 | 0 | 1554 ms | 1753 ms | 2432 ms | 3116 ms | 1581 ms | 25.2/s |
| log_read (catch-up page) | 3,297 | 0 | 307 ms | 365 ms | 881 ms | 1431 ms | 322 ms | 10.0/s |
| counter_read (single row) | 3,297 | 0 | 101 ms | 128 ms | 161 ms | 731 ms | 104 ms | 10.0/s |
| bootstrap (snapshot page) | 330 | 0 | 308 ms | 361 ms | 392 ms | 444 ms | 311 ms | 1.0/s |
| write→read visibility | 4,169 | 0 | 1810 ms | 2606 ms | 2907 ms | 3830 ms | 1821 ms | — |

- Sustained **25.2 pushes/s = 50.4 committed mutation transactions/s**
  (16,724 changelog rows + 16,724 ledger rows written and then cleaned up).
- Visibility = writer's pre-push stamp → reader observation on the direct
  Postgres poll path; it includes the full push latency plus the 1s reader
  poll interval by construction. The hub fan-out path (KS-4) is a separate
  seam and was not measured here.

### Burst probe (runId `ks9-1-burst`, 60s, pacing effectively removed)

Same 40 writers running closed-loop back-to-back (target 50 pushes/s/worker
so the pacing sleep never fires), 10 readers at 500ms. This is the
"simultaneous accept burst" shape. Started 2026-07-04T14:37:11Z, 61.6s.
**Errors: zero.**

| op | n | err | p50 | p95 | p99 | max | rate |
|---|---|---|---|---|---|---|---|
| push | 1,547 | 0 | 1553 ms | 1746 ms | 2010 ms | 2061 ms | 25.1/s |
| log_read | 1,134 | 0 | 307 ms | 365 ms | 494 ms | 808 ms | 18.4/s |
| counter_read | 1,134 | 0 | 101 ms | 125 ms | 150 ms | 181 ms | 18.4/s |
| bootstrap | 120 | 0 | 303 ms | 355 ms | 370 ms | 376 ms | 2.0/s |

Removing the pacing changed **nothing**: throughput stayed at ~25 pushes/s
(the closed loop is bound by the WAN round trips per push:
40 workers ÷ 1.55 s/push ≈ 25.8/s) and tail latency actually tightened
(p99 2010 ms vs 2432 ms) — no queueing, no degradation, no errors under the
burst shape. DB-side headroom above this client's ceiling is untouched;
pushing the database itself to saturation requires either a closer client
or many more concurrent workers, both out of scope for the 2×-June-shape
acceptance.

## Database-side observations (mid-run)

Sampled `pg_stat_activity` on `khala_sync_staging` during the sustained run:

- Backends: 51 (the harness pool) on top of a 9-backend idle baseline;
  `max_connections` 600 — no pressure on backend slots.
- Active backends: 15–28, **every one waiting on `ClientRead`** — the
  database was waiting on the client's next statement over the ~100 ms WAN
  round trip. Zero lock waits, zero IO waits, zero buffer contention.
  Longest in-flight transaction observed: 0.75 s.
- CPU/latency on the single-row read: p50 101 ms ≈ exactly one network
  round trip — server-side execution time is sub-millisecond noise.

**The bottleneck in every number above is the measurement client's WAN RTT
(owner Mac → us-central1, ~100 ms), not the database.** A push of batch 2
executes ~14 sequential statements ⇒ ~14 RTTs ≈ 1.4–1.6 s of pure network
time, which is the entire push p50. The production path (Worker →
Hyperdrive → Cloud SQL, single-digit-ms proximity per SPEC §3.3 of the
rationale doc) removes that amplification; DB-side capacity was never
approached.

## Comparison to the June 28–29 failure profile

| | June 28–29 (D1) | This run (Khala Sync staging) |
|---|---|---|
| Concurrent writers | ~20 (16-slot burn + accepts) | 40 (2×) |
| Per-turn/push ingest | ~10–11 statements, no retry | ~12–14 statements, per-envelope transactions |
| Counter reads | unbounded `SUM` over full ledger on the hot path | single-row scope-counter read, p99 161 ms |
| Sustained window | collapsed within minutes | 5.5 min, flat latency, zero errors |
| Failure mode | `pylon_api_storage_error` / "D1 overloaded", 503/500/401 cascade | **no overload-class failures of any kind** |
| Workaround required | stagger accepts ~2 s, 2 s stats cache | none |

The June cascade was a single-writer SQLite queue where one slow aggregate
stalled dispatch reads. Here, 40 concurrent writer streams serialized only
on their own per-scope counter rows; readers ran on MVCC snapshots and never
queued behind writers. The structural fix the rationale doc predicted is
what the run shows.

**Pass/fail against "no overload-class failures at 2× June peak": PASS.**

## Bottleneck notes → KS-2 / KS-4 tuning

1. **Statement-count per push is the real lever.** Each envelope costs ~6
   round trips (BEGIN, client-state upsert, checkAndReserve, version
   allocation + changelog insert, ledger record, COMMIT) plus one
   `lastMutationId` read per push. On Hyperdrive each round trip is cheap,
   but Worker CPU-time and pool occupancy scale with it. KS-2 candidates:
   fold `upsertClientState` + `checkAndReserve` into one statement, and
   piggyback `lastMutationId` on the final envelope's transaction. That is
   ~30% fewer round trips per push before any batching.
2. **Per-scope writer serialization never showed.** Distinct personal
   scopes ⇒ no counter-row contention. A fleet writing ONE hot scope (e.g.
   a busy `scope.fleet_run.*`) serializes on that scope's counter row by
   design; if a future shape needs >~200 tx/s into a single scope, KS-2
   should look at counter-row batching. Not a current risk.
3. **Reads are index-only and flat.** `logPage` p50 ≈ 3 RTTs (~307 ms from
   the WAN client) regardless of changelog size during the run; `bootstrap`
   the same. The KS-2.2 page-shape design held under concurrent write load.
4. **Delta visibility on the poll path is poll-interval-bound.** p50 1.8 s
   ≈ push latency + 1 s poll. The KS-4 hub (LISTEN-driven capture → DO
   WebSocket fan-out) is what turns this into sub-second delivery; this run
   deliberately measured the Postgres-authoritative fallback path.
5. **Backend budget.** 52 direct backends for the harness was 8.7% of
   `max_connections` — but the RUNBOOK's "backends should stay small and
   flat" invariant is about the production topology (Hyperdrive pools,
   direct paths bounded). The harness enforces a 25%-of-max cap; keep that
   when re-running.

## HTTP-mode run: attempted, skipped honestly

The harness has a full `--mode=http` path (POST `/api/sync/push`,
GET `/api/sync/log`, identity probe via GET `/api/agents/me`). The staging
Worker (`https://openagents-staging.openagents.workers.dev`, per
`wrangler.jsonc` `env.staging`) was probed with one request using the local
agent bearer (`~/.pylon-fable/auth/openagents-agent-token`): **401** —
agent registrations live in the production D1, and staging has its own
(`openagents-autopilot-staging`), so the token does not authenticate there.
Running http mode against production was out of scope for a load test.
The http path stays exercisable the moment a staging-registered agent token
exists: `KHALA_LOAD_TOKEN=… bun scripts/load-test.ts --mode http
--base-url https://openagents-staging.openagents.workers.dev`.

## Cleanup

All synthetic rows are namespaced (`scope.user.loadtest.<runId>.%` scopes,
`cg-loadtest-<runId>-%` client groups) and were deleted post-run by the
harness (plain `DELETE`s across `khala_sync_changelog`,
`khala_sync_scopes`, `khala_sync_mutations`, `khala_sync_client_state`,
`khala_sync_capture_checkpoints`, `khala_sync_scope_owners` — volumes are
bounded by the run and the scopes are disjoint from real scopes by
construction, so no compaction pass is needed). Main run deletion counts:
16,724 changelog + 16,724 mutations + 40 scopes + 40 client-state rows;
post-run verification query returned zero remaining `loadtest` rows.

## Reproduction

```sh
# substrate mode against staging (creds via env; never printed):
KHALA_LOAD_DATABASE_URL="postgres://khala_app:…@<ip>:5432/khala_sync_staging" \
bun run --cwd packages/khala-sync-server load-test -- \
  --mode substrate --workers 40 --pushes-per-second 2 --batch 2 \
  --readers 10 --duration-sec 330 --pool 52 --json-out /tmp/report.json
```
