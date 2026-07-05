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
- [`RUNBOOK.md`](./RUNBOOK.md) — the ops runbook (KS-9.3): Cloud SQL
  monitoring, migration runner staging→prod + hash-mismatch recovery,
  compaction scheduling (dry-run first, checkpoint guard), capture daemon
  operation + liveness/recovery, hub DO reset semantics, Hyperdrive pool
  saturation, and secrets locations (names only). The SPEC §7 invariant
  set is registered in `apps/openagents.com/INVARIANTS.md` ("Khala Sync
  (SPEC §7 invariant set)") with per-invariant test pointers and honest
  statuses.
- [`CVR_DESIGN.md`](./CVR_DESIGN.md) — CVR read-set diffing (KS-7.2, v2,
  flag-gated `KHALA_SYNC_CVR=1`): per-(clientGroup, scope) Client View
  Records in Postgres, the diff-pull flow (puts/dels fall out of a set
  difference — permission-driven retraction without re-bootstrap), the
  drift soundness argument for our hybrid live path, compaction interplay,
  cost bounds, and the byte-equality verification plan.
- [`MIGRATION_PLAN.md`](./MIGRATION_PLAN.md) — the rolling D1 → Cloud SQL
  migration plan (KS-8.3): table census, per-domain risk/verification
  notes, wave sequencing after KS-8.1/8.2, cron consolidation, and the D1
  retirement checklist.
- Design rationale + database-alternatives analysis:
  [`../fable/2026-07-04-database-alternatives-and-postgres-sync-engine.md`](../fable/2026-07-04-database-alternatives-and-postgres-sync-engine.md)

## Packages

- `packages/khala-sync` — wire/domain contracts (Effect Schema), including
  the KS-6.1 fleet cockpit entity contracts (`src/fleet.ts`) and MC-1
  owner-private chat entity contracts (`src/chat.ts`). Landed.
- `packages/khala-sync-server` — Postgres substrate + mutator engine +
  capture + compaction + `KhalaSyncHubDO` + the KS-6.1 fleet scope
  projection/mutators (`fleet-projection.ts`, `fleet-mutators.ts`,
  `khala_sync_scope_owners` / `khala_sync_fleet_intents` in
  `migrations/0004`) + MC-1 chat mutators (`chat-mutators.ts`,
  `migrations/0018_owner_private_chat.sql`). Landed through MC-1 server-side.
- `packages/khala-sync-client` — local store + overlay/rebase + session.
  Contracts landed.
- `clients/khala-code-desktop` — FIRST CONSUMER WIRED (KS-6.2, #8303):
  bun-side `KhalaSyncService` (`src/bun/khala-sync-service.ts`) opens the
  local store under `~/.khala-code/`, runs the client session against the
  OpenAgents base URL with the user's agent token, and exposes
  `khalaSyncFleetState` / `khalaSyncFleetMutate` over the desktop RPC; the
  Fleet screen renders the synced `fleet_run` scope and routes
  pause/resume/setDesiredSlots through the fleet mutators. Default-on as of
  #8383; `KHALA_SYNC_FLEET=0`/`false`/`off` is the explicit opt-out and the
  local status/list reads remain only as a degraded fallback for missing auth,
  disconnected sync, or intentional disablement.

## Worker routes (SPEC §3 — complete)

All four wire-protocol surfaces are live in the `openagents.com` Worker
(`apps/openagents.com/workers/api/src/khala-sync-*-routes.ts`), each
authenticated via the standard actor auth (session or agent bearer) and
scope-gated by the KS-7.1 taxonomy-complete resolver (`resolveScopeRead`
from `packages/khala-sync-server`, Worker capabilities in
`khala-sync-scope-auth.ts`: personal self-only, public, live D1 team
membership, agent_run ownership, legacy thread ownership, owner-private
chat thread scope owners, fleet_run scope owners; unknown kinds and failed
lookups fail CLOSED — SPEC §3):

| Route | Lane | Serving |
|---|---|---|
| `POST /api/sync/push` | KS-3.1 #8291 | transactional mutator batches over the `KHALA_SYNC_DB` Hyperdrive binding |
| `GET /api/sync/log` | KS-4.3 #8296 | offset-resumable `LogPage` catch-up, hub-window-first with authoritative Postgres fallthrough; ETag on non-`upToDate` pages |
| `POST /api/sync/bootstrap` | KS-4.4 #8297 | consistent snapshot pages (self-contained page tokens), final page carries the stitch `cursor`; always no-store |
| `GET /api/sync/connect` | KS-4.4 #8297 | WebSocket upgrade proxied to the per-scope `KhalaSyncHubDO` `/connect` (auth + scope gate BEFORE the upgrade) |
| `POST /api/sync/cvr-pull` | KS-7.2 #8306 | **flag-gated (`KHALA_SYNC_CVR=1`; 404 unflagged)** CVR read-set diff pull — the v2 `must_refetch` recovery path (puts/dels vs the stored per-(clientGroup, scope) CVR; permission-driven retraction is structural). Design: [`CVR_DESIGN.md`](./CVR_DESIGN.md) |

The admin-bearer internal hub surface
(`/api/internal/khala-sync/hub/{append,log,connect,access-changed}`)
remains for the capture daemon and operators only; `access-changed` is the
KS-7.1 revocation trigger (`POST { scope }` → hub broadcasts
`MustRefetch(access_changed)` and closes every socket — see RUNBOOK). The end-to-end stitch seam — bootstrap
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
| KS-3 Mutator engine (push route, registry, guide+contract) | #8291 #8292 (full fleet mutator catalog + intent-consumption seam landed — see the catalog in [`MUTATORS.md`](./MUTATORS.md); supervisor enforcement wiring is the follow-up) #8293 (guide+contract landed: [`MUTATORS.md`](./MUTATORS.md), `packages/behavior-contracts/src/khala-sync.ts`) |
| KS-4 Capture + Hub DO (capture, hub, catch-up, bootstrap/seam) | #8294 #8295 #8296 #8297 |
| KS-5 Client engine (store, rebase, session, web lane) | #8298 #8299 #8300 #8301 |
| KS-6 First consumers (fleet projection, desktop, tokens-served) | #8302 (server-side projection + operator mutators landed; supervisor intent enforcement is follow-up) #8303 (desktop fleet cockpit default-on through Khala Sync; `KHALA_SYNC_FLEET=0`/`false`/`off` is opt-out) #8304 |
| KS-7 Permissions (scope auth, CVR v2) | #8305 (scope-auth resolver + access-change refetch landed) #8306 (CVR read-set diffing landed behind `KHALA_SYNC_CVR=1` with real-Postgres equivalence tests — design: [`CVR_DESIGN.md`](./CVR_DESIGN.md)) |
| KS-8 Domain migration (assignments, ledger, rolling plan) | #8307 #8308 #8309 (plan: [`MIGRATION_PLAN.md`](./MIGRATION_PLAN.md)); per-domain waves #8315–#8330 |
| KS-9 QA/ops (load test, behavior contracts, invariants+runbook) | #8310 #8311 #8312 (invariants+runbook landed: [`RUNBOOK.md`](./RUNBOOK.md) + the "Khala Sync (SPEC §7 invariant set)" section in `apps/openagents.com/INVARIANTS.md`) |

Critical path to the first live surface: KS-0.1 → KS-0.2/0.3 → KS-2.1 →
KS-3.1 + KS-4.2 → KS-2.2/KS-4.4 → KS-5.1→5.3 → KS-6.1→6.2 (Khala Code
desktop fleet cockpit live on Khala Sync).
