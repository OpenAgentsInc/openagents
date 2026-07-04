# docs/khala-sync

**Khala Sync** — the owned replication substrate: Cloud SQL Postgres
(authoritative) → per-scope Durable Object hubs on Cloudflare → SQLite
clients with server-authoritative mutators and rebase.

> Naming: always the two-word compound **Khala Sync**. Bare "Khala" is the
> collective-intelligence product (Episode 242, `docs/transcripts/242.md`).

- [`SPEC.md`](./SPEC.md) — the normative v0.1 specification: system shape,
  scopes/versions/cursors, changelog + mutation model, wire protocol,
  Postgres substrate, hub DO, client engine, invariants, verification plan.
- [`MUTATORS.md`](./MUTATORS.md) — the mutator authoring guide (KS-3.3):
  single-transaction rule, replay-safety, in-band rejection discipline,
  Hyperdrive session-state rules, ledger idempotency, scope authorization,
  canonical post-images, Worker registry registration, testing checklist,
  and the enforced `khala_sync.push.validation_never_blocks_queue.v1`
  behavior contract.
- Design rationale + database-alternatives analysis:
  [`../fable/2026-07-04-database-alternatives-and-postgres-sync-engine.md`](../fable/2026-07-04-database-alternatives-and-postgres-sync-engine.md)

## Packages

- `packages/khala-sync` — wire/domain contracts (Effect Schema), including
  the KS-6.1 fleet cockpit entity contracts (`src/fleet.ts`). Landed.
- `packages/khala-sync-server` — Postgres substrate + mutator engine +
  capture + compaction + `KhalaSyncHubDO` + the KS-6.1 fleet scope
  projection/mutators (`fleet-projection.ts`, `fleet-mutators.ts`,
  `khala_sync_scope_owners` / `khala_sync_fleet_intents` in
  `migrations/0004`). Landed through KS-6.1 server-side.
- `packages/khala-sync-client` — local store + overlay/rebase + session.
  Contracts landed.

## Worker routes (SPEC §3 — complete)

All four wire-protocol surfaces are live in the `openagents.com` Worker
(`apps/openagents.com/workers/api/src/khala-sync-*-routes.ts`), each
authenticated via the standard actor auth (session or agent bearer) and
scope-gated by the v1 `canReadScopeV1` predicate (own personal scope +
`scope.public.*`; membership scopes arrive with KS-7):

| Route | Lane | Serving |
|---|---|---|
| `POST /api/sync/push` | KS-3.1 #8291 | transactional mutator batches over the `KHALA_SYNC_DB` Hyperdrive binding |
| `GET /api/sync/log` | KS-4.3 #8296 | offset-resumable `LogPage` catch-up, hub-window-first with authoritative Postgres fallthrough; ETag on non-`upToDate` pages |
| `POST /api/sync/bootstrap` | KS-4.4 #8297 | consistent snapshot pages (self-contained page tokens), final page carries the stitch `cursor`; always no-store |
| `GET /api/sync/connect` | KS-4.4 #8297 | WebSocket upgrade proxied to the per-scope `KhalaSyncHubDO` `/connect` (auth + scope gate BEFORE the upgrade) |

The admin-bearer internal hub surface
(`/api/internal/khala-sync/hub/{append,log,connect}`) remains for the
capture daemon and operators only. The end-to-end stitch seam — bootstrap
under concurrent writes, catch-up from the snapshot cursor, live
DeltaFrames, byte-equal convergence with apply idempotence — is verified by
`apps/openagents.com/workers/api/src/khala-sync-stitch-seam.e2e.test.ts`
against local Postgres + the real hub DO + the real client SQLite store.

## Issue map (epic [#8282](https://github.com/OpenAgentsInc/openagents/issues/8282))

| Workstream | Issues |
|---|---|
| KS-0 Infrastructure (Cloud SQL, Hyperdrive, migration runner) | #8283 #8284 #8285 |
| KS-1 Contracts hardening | #8286 |
| KS-2 Postgres substrate (outbox writer, reads, compaction, idempotency) | #8287 #8288 #8289 #8290 |
| KS-3 Mutator engine (push route, registry, guide+contract) | #8291 #8292 #8293 (guide+contract landed: [`MUTATORS.md`](./MUTATORS.md), `packages/behavior-contracts/src/khala-sync.ts`) |
| KS-4 Capture + Hub DO (capture, hub, catch-up, bootstrap/seam) | #8294 #8295 #8296 #8297 |
| KS-5 Client engine (store, rebase, session, web lane) | #8298 #8299 #8300 #8301 |
| KS-6 First consumers (fleet projection, desktop, tokens-served) | #8302 (server-side projection + operator mutators landed; supervisor intent enforcement is follow-up) #8303 #8304 |
| KS-7 Permissions (scope auth, CVR v2) | #8305 #8306 |
| KS-8 Domain migration (assignments, ledger, rolling plan) | #8307 #8308 #8309 |
| KS-9 QA/ops (load test, behavior contracts, invariants+runbook) | #8310 #8311 #8312 |

Critical path to the first live surface: KS-0.1 → KS-0.2/0.3 → KS-2.1 →
KS-3.1 + KS-4.2 → KS-2.2/KS-4.4 → KS-5.1→5.3 → KS-6.1→6.2 (Khala Code
desktop fleet cockpit live on Khala Sync).
