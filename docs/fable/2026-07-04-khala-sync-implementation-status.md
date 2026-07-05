# Khala Sync — Implementation Status (end of the 2026-07-04 build run)

**Lane:** Fable synthesis. **Epic:** [#8282](https://github.com/OpenAgentsInc/openagents/issues/8282).
**Origin:** [`2026-07-04-database-alternatives-and-postgres-sync-engine.md`](./2026-07-04-database-alternatives-and-postgres-sync-engine.md)
(the D1-overload root-cause analysis and design). **Spec:** `docs/khala-sync/SPEC.md`.
This doc records what was actually built and shipped in the ~24-hour
fleet-delegated run of 2026-07-04, and what deliberately remains.

## TL;DR

Khala Sync went from design doc to **live in production in one day**: Cloud
SQL Postgres 17 (HA, logical decoding on) behind Hyperdrive; the full sync
engine (contracts → substrate → mutators → capture → hub DOs → client
store/rebase/session) deployed on `openagents.com`; the public tokens-served
counter serving from a Postgres projection instead of the unbounded D1
`SUM()`; and **every domain from the D1 audit dual-writing to Postgres
twins**. A 40-writer staging load test at 2× the June-28/29 collapse shape
produced zero overload-class failures. The only remaining KS work is the
deliberately owner-gated destructive batch: per-domain read cutover to
Postgres serving and D1 table drops.

## Infrastructure (KS-0) — LIVE

- Cloud SQL instance `khala-sync-pg` (project `openagentsgemini`,
  us-central1): PostgreSQL 17.10, Enterprise, 8 vCPU / 52 GiB, REGIONAL HA,
  PITR, `wal_level=logical` (10 slots free for the future WAL-capture
  upgrade). Databases `khala_sync_prod` + `khala_sync_staging`; roles
  `khala_app` (Hyperdrive), `khala_migrate` (owner), `khala_capture`
  (REPLICATION). TLS enforced. Secrets in the gitignored workspace
  `.secrets/khala-sync-cloudsql.env`.
- Hyperdrive configs (prod `6cd88528…`, staging `63a375a2…`) bound as
  `KHALA_SYNC_DB`; live prod round trips ~70–230 ms cold, faster warm.
- Migration runner (`packages/khala-sync-server/scripts/migrate.ts`):
  hash-ledgered, idempotent; **migrations 0001–0028 applied to staging and
  prod**. (Ledger note: `0027_identity_auth_domain.sql` was renumbered to
  0028 by a concurrent lane after application; the ledger rows were renamed
  in both DBs — same sha — and the runner verified clean.)

## The engine — LIVE IN PROD (Worker deployed; all closed)

- **Contracts** `packages/khala-sync` — scopes/versions/cursors, changelog
  entries (canonical-JSON post-images), mutation envelopes, full wire
  protocol, golden fixtures + property tests. (#8286)
- **Substrate** `packages/khala-sync-server` — transactional outbox writer
  with row-locked per-scope dense versions (#8287), bootstrap/log reads with
  stitch-seam proofs (#8288), compaction + retention watermark (#8289),
  mutation-ledger idempotency (#8290).
- **Write path** — `POST /api/sync/push`: named server-authoritative
  mutators, one Postgres transaction each, in-band rejections that never
  block the queue (#8291, #8292 fleet mutators, #8293 guide + behavior
  contract). Live prod proof: mutation applied → exact changelog row →
  byte-identical read-back.
- **Delivery** — capture daemon (launchd `com.openagents.khala-sync-capture`
  on the operator Mac; direct-PG LISTEN + poll; checkpoints table) (#8294);
  `KhalaSyncHubDO` per scope with hibernating WebSockets (#8295); hub-first
  catch-up route (#8296); bootstrap + `/api/sync/connect` (#8297).
- **Client** `packages/khala-sync-client` — bun:sqlite store (#8298),
  overlay/rebase with 50-seed convergence property tests (#8299), session
  state machine + HTTP/WS transport (#8300), web store on SQLite-WASM
  opfs-sahpool with Web-Locks single-writer (#8301).
- **First consumers** — fleet cockpit projection + operator mutators
  (#8302), Khala Code desktop Fleet screen behind `KHALA_SYNC_FLEET=1`
  (#8303), and the public tokens-served projection: backfilled to exactly
  8,370,108,795 (== D1 SUM), honest 2 s staleness, D1 fallback fail-open —
  **the unbounded full-table SUM is off the hot path** (#8304).
- **Auth** — full scope-taxonomy resolver + access-change refetch with a
  real revocation e2e (#8305); CVR read-set diffing behind `KHALA_SYNC_CVR`
  with byte-equal equivalence proofs (#8306).
- **Verification** — staging load test: 40 writers × 5.5 min + burst =
  9,909 pushes, zero failures of any kind, DB waits 100% client-RTT
  (#8310, report `docs/khala-sync/2026-07-04-load-test-report.md`); synced-
  surface behavior contracts (#8311); the nine SPEC §7 invariants registered
  with per-test citations + ops RUNBOOK (#8312).
- **Fleet-intent enforcement** — operator pause/resume/stop/slots intents
  now steer the pylon supervisor loop exactly-once (#8332).

## Domain dual-write machinery — ALL LANDED

Every domain has: Postgres twin schema (indexes re-derived, not blind-ported),
a typed seam (read-back mirror / mirroring-D1Database / store-factory wrap —
chosen per domain shape), `*_DUAL_WRITE` default ON, `*_READS` default `d1`
(compare implemented), cursor-resumable idempotent backfill with domain-true
`--verify` (money to the cent, idempotency-key set equality, chain digests),
and contract tests against both engines:

| Domain | Lane | Read state |
|---|---|---|
| Pylon dispatch + control-plane + runner status | #8307, #8315 | **postgres (LIVE cutover)** |
| Token ledger | #8308 | d1 |
| Agent runtime metadata (+ remainder 0012) | #8316, #8334 | d1 (scheduler scans routable) |
| Artanis (20 tables, 6 crons) | #8317 | d1 |
| Billing/Stripe/pay-ins (money) | #8318 | d1 (flip epic-gated) |
| Treasury/payouts/tips (money) | #8319 | d1 (flip epic-gated) |
| Inference entitlements (29 tables, hot path) | #8320 | d1 |
| Forum content + remainder (PMs/ACLs/trust/work-requests) | #8321, #8338 | d1 |
| CRM/email (36 tables, PII discipline) | #8322 | d1 |
| Sites core + remainder (51 tables; env-value secrets excluded) | #8323, #8357 | d1 |
| Khala Code product state (scope adoption + tombstones) | #8324, #8356 | n/a (projection) |
| Business funnel + all order writers | #8325, #8359 | d1 |
| Training + gym/eval remainder | #8326, #8355 | d1 |
| Forge (16 tables, custody-safe) | #8327 | d1 |
| Supervision long-tail (29 tables, 3 crons re-homed) | #8328 | d1 |
| Identity/auth core (17 tables; most sensitive) | #8329 | d1 (owner-gated) |

Also landed along the way: the served-registry caveat PR #8314 (reviewed +
merged), a real production regression fix in the referral engagement feed
(1c323deac3), route-manifest coverage repairs, and zero-debt ratchet
restorations (63eb16da82 — no allowlist growth).

## What remains — ONE destructive, owner-gated batch

Open: #8330 (cron sweep + D1 retirement), #8335/#8336/#8337/#8358/#8360/
#8361/#8362 (per-domain read cutover + D1 decommission). All of these flip
read-serving to Postgres and then **drop D1 tables** — irreversible, spanning
money and auth (#8362 explicitly owner-gated). Deliberately not parallelized.
Sequence per domain: `_READS=compare` (serve D1, log drift) → zero-drift
soak → `postgres` serving → soak → drop D1. Low-risk domains first (Artanis,
entitlements, training); money and auth last with explicit owner sign-off.
Backfills must run before any flip (RUNBOOK order).

The ONE-UI epic (#8339: React + Tailwind + shadcn everywhere) runs in a
parallel lane and owns #8348/#8350/#8351/#8354. Standing rule: all new UI
uses the ONE-UI shadcn components (`apps/openagents.com/apps/start/src/components/ui`).

## Operational notes

- The capture daemon runs on the operator Mac via launchd with a
  secrets-sourcing wrapper (`~/.khala-sync-capture/run.sh`); re-home to the
  GCE box when convenient (RUNBOOK).
- Every migration must be applied to staging+prod **before** deploying a
  Worker that references its tables (grants flow automatically from
  `khala_migrate` default privileges — the KS-0.2 grant incident is
  documented in RUNBOOK).
- Known D1-era pathologies this build retires structurally: the per-item
  raw-event-chunk N+1, the 4-statement/13-index ledger fan-out (indexes
  re-derived on Postgres), the uncached counter SUM, and dispatch-gate reads
  sharing one SQLite writer with the telemetry firehose.
