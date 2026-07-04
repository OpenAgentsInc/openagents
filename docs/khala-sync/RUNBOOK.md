# Khala Sync — Ops Runbook (KS-9.3, #8312)

Operational procedures for the Khala Sync replication substrate: Cloud SQL
Postgres (authoritative) → per-scope `KhalaSyncHubDO` hubs in the
`openagents.com` Worker → SQLite clients.

- **Spec:** [`SPEC.md`](./SPEC.md) (invariants: §7, registered in
  `apps/openagents.com/INVARIANTS.md` "Khala Sync (SPEC §7 invariant set)").
- **Mutator authoring:** [`MUTATORS.md`](./MUTATORS.md).
- **Deep-dive runbooks (source of truth for command mechanics):**
  `packages/khala-sync-server/README.md` — "Connection reference",
  "Capture runbook", "Compaction runbook", "Migrations runbook". This file
  is the ops-level index; when mechanics change, update the package README
  and fix the pointers here.
- **Deploying the Worker (routes, hub DO, Hyperdrive bindings):**
  `docs/DEPLOYMENT.md` — the ONLY sanctioned path is
  `bun run --cwd apps/openagents.com/workers/api deploy:safe`.

## Topology at a glance

| Piece | Where | Connection |
|---|---|---|
| Cloud SQL instance | `khala-sync-pg`, GCP project `openagentsgemini`, region `us-central1`, PostgreSQL 17 | — |
| Databases | `khala_sync_prod`, `khala_sync_staging` (same instance) | — |
| Worker request paths (push/log/bootstrap) | `openagents.com` Worker, binding `KHALA_SYNC_DB` | Hyperdrive, transaction-mode pooling, role `khala_app` |
| Migrations | `packages/khala-sync-server/scripts/migrate.ts` | DIRECT connection, role `khala_migrate` — never Hyperdrive |
| Capture daemon | `packages/khala-sync-server/scripts/capture.ts` (launchd, owner Mac first) | DIRECT connection, role `khala_capture` — never Hyperdrive (LISTEN needs a session) |
| Compaction | `packages/khala-sync-server/scripts/compact.ts` (cron) | DIRECT connection — never Hyperdrive |
| Hub delivery | `KhalaSyncHubDO` (one per scope, DO SQLite window) | in-Worker |

## Secrets (names only — NEVER echo values)

All Khala Sync database secrets live in the gitignored workspace file
`~/work/.secrets/khala-sync-cloudsql.env` on the owner machine (mirrored per
the `docs/DEPLOYMENT.md` secrets convention). Keys present:

`KHALA_SYNC_CLOUDSQL_INSTANCE`, `KHALA_SYNC_CLOUDSQL_PROJECT`,
`KHALA_SYNC_CLOUDSQL_REGION`, `KHALA_SYNC_CLOUDSQL_ROOT_USER`,
`KHALA_SYNC_CLOUDSQL_ROOT_PASSWORD`, `KHALA_SYNC_CLOUDSQL_IP`,
`KHALA_SYNC_DB_PROD`, `KHALA_SYNC_DB_STAGING`, `KHALA_SYNC_APP_USER`,
`KHALA_SYNC_APP_PASSWORD`, `KHALA_SYNC_MIGRATE_USER`,
`KHALA_SYNC_MIGRATE_PASSWORD`, `KHALA_SYNC_CAPTURE_USER`,
`KHALA_SYNC_CAPTURE_PASSWORD`, `KHALA_SYNC_HYPERDRIVE_PROD_ID`,
`KHALA_SYNC_HYPERDRIVE_STAGING_ID`.

The capture daemon additionally needs `OPENAGENTS_ADMIN_API_TOKEN` (the
workspace admin-token secret). Never copy any value into tracked files,
commits, issue comments, logs, or filled-in plists that leave the machine.
Hyperdrive config ids are public references, not secrets (prod
`6cd885288e5b4b2f8fd2d76200c980bf`, staging
`63a375a24d6f475db60d9490f55c9102`).

## Cloud SQL monitoring

Where: GCP console → project `openagentsgemini` → SQL → instance
`khala-sync-pg` (Monitoring tab / Cloud Monitoring metrics), or
`gcloud sql instances describe khala-sync-pg --project openagentsgemini`.

What to watch:

- **Connections** (`cloudsql.googleapis.com/database/postgresql/num_backends`):
  the Worker side is pooled by Hyperdrive, so backend count should stay
  small and flat. A climb toward the instance's `max_connections` means a
  leaked direct-connection path (capture/compact/migrate not closing) or a
  misconfigured tool connecting directly instead of through Hyperdrive.
- **Storage / disk utilization**: `khala_sync_changelog` is the growth
  surface. If storage grows without bound, compaction is not running or its
  watermark is being held back (see "Compaction" below — the capture
  checkpoint bound holds compaction at a stalled scope's
  `pushed_through_version`; a dead capture daemon therefore freezes
  compaction, by design).
- **CPU / memory**: sustained CPU spikes usually mean unindexed scans; the
  changelog reads are index-only by design (scope + version, plus the
  entity index in migration `0003`). Investigate query insights before
  resizing.
- **Replication slots** (once the WAL/pgoutput capture upgrade of SPEC §4
  lands — NOT yet in use; v1 capture tails the changelog table): watch
  `pg_replication_slots` for retained WAL growth from an inactive slot. An
  abandoned slot on PG17 failover-slot capture would block WAL truncation
  and fill the disk. Today there are no slots to monitor; this bullet
  activates with the WAL capture lane.
- **Liveness from the edge**: `GET /api/internal/khala-sync/db-smoke`
  (admin bearer) proves a round-trip through the Hyperdrive binding from
  the deployed Worker and reports `{ ok, khalaSyncTables, latencyMs }`.

## Migration runner (staging → prod)

Mechanics: `packages/khala-sync-server/README.md` "Migrations runbook".
Ordered `NNNN_*.sql` files, one transaction per file plus its
`khala_sync_migrations` ledger insert, session advisory lock against
concurrent runners, idempotent reruns.

Procedure (always staging first, always dry-run first):

```sh
# 1. Staging plan (no writes at all):
bun run --cwd packages/khala-sync-server migrate -- --dry-run \
  --database-url "<direct staging URL as khala_migrate>"
# 2. Staging apply, then run the khala-sync test suites / staging smoke.
# 3. Prod plan, then prod apply — same commands against khala_sync_prod.
# 4. Record the run (applied files list) on the owning KS issue.
```

Hash-mismatch recovery: the runner REFUSES to run (dry or real) when an
applied migration's on-disk hash changed or an applied file disappeared.
This means someone edited or removed a landed migration. Do NOT edit the
ledger to force it through. Recovery: `git log -- packages/khala-sync-server/migrations/`
to find the drift, restore the exact landed file content (the ledger's
`sha256` must match again), and express the intended change as the next
`NNNN_*.sql` instead. Editing `khala_sync_migrations` by hand is a
last-resort owner decision and must be recorded on an issue.

## Compaction scheduling

Mechanics and semantics: `packages/khala-sync-server/README.md`
"Compaction runbook" + "Compaction (KS-2.3)". Entry point:
`packages/khala-sync-server/scripts/compact.ts` over a DIRECT connection.

- **Always dry-run first** on a new environment or changed bounds:
  `bun run --cwd packages/khala-sync-server compact -- --dry-run
  --max-retained-entries 10000 --max-retained-age-ms 86400000` — prints the
  per-scope watermark move and would-delete counts, writes nothing
  (dry-run parity is test-proven).
- **Checkpoint guard (do not disable):** the watermark never passes any
  scope's capture `pushed_through_version + 1`, and a scope with NO
  checkpoint row fails closed (no compaction at all). Consequence: a dead
  or lagging capture daemon halts compaction for its scopes and the
  changelog grows. The fix is to revive capture (below), never to relax the
  bound.
- **Schedule:** cron/Cloud Scheduler; daily is plenty at current write
  volumes, hourly once fleets push sustained load. Keep
  `--max-retained-entries` comfortably larger than the hub window and the
  longest expected client offline gap; `--max-retained-age-ms` (e.g. 24h)
  guarantees a client offline less than that is never force-refetched.
- **After compaction**, clients behind the window get
  `MustRefetch(cursor_behind_retained_window)` and re-bootstrap — that is
  the designed behavior (SPEC §7 invariant 6), not an incident. A spike of
  re-bootstraps right after a compaction run with aggressive bounds means
  the retained window is too small; widen the bounds.
- Runs are idempotent: a rerun after a partial failure re-plans from the
  current watermark. Exit code 1 = at least one scope failed (per-scope
  isolation; the others still compacted).

## Capture daemon operation

Mechanics: `packages/khala-sync-server/README.md` "Capture runbook".
Supervision template: `packages/khala-sync-server/ops/com.openagents.khala-sync-capture.plist`
(fill placeholders, install to `~/Library/LaunchAgents/`, `launchctl
bootstrap gui/$(id -u) …`; `KeepAlive` restarts on crash).

Env (token env-only, never a flag): `KHALA_SYNC_DATABASE_URL` (direct URL
as `khala_capture` — NEVER the Hyperdrive string),
`KHALA_SYNC_HUB_APPEND_URL` (prod
`https://openagents.com/api/internal/khala-sync/hub/append`),
`OPENAGENTS_ADMIN_API_TOKEN`, `KHALA_SYNC_CAPTURE_POLL_INTERVAL_MS`
(default 5000), `KHALA_SYNC_CAPTURE_BATCH_VERSIONS` (default 200).

Liveness check (the checkpoints table is the truth):

```sql
-- updated_at advancing while writes commit == capture is pushing:
SELECT scope, pushed_through_version, updated_at
FROM khala_sync_capture_checkpoints ORDER BY updated_at DESC;
-- lag per scope (anything persistently > 0 while updated_at is stale = stalled):
SELECT s.scope, s.last_version - COALESCE(c.pushed_through_version, 0) AS lag
FROM khala_sync_scopes s
LEFT JOIN khala_sync_capture_checkpoints c USING (scope)
WHERE s.last_version > COALESCE(c.pushed_through_version, 0);
```

Process side: `launchctl print gui/$(id -u)/com.openagents.khala-sync-capture`
plus the plist's stdout/err logs. Edge side: the hub internal log route
(`GET /api/internal/khala-sync/hub/log?scope=…&cursor=…`, admin bearer)
should serve freshly pushed versions.

Recovery:

- **Stalled scope / daemon death:** checkpoints only advance on hub 2xx and
  delivery is at-least-once, so recovery is just restarting the daemon (or
  `--once` for a single drain pass; exit 1 = some scope still failing). No
  dedupe cleanup needed — the hub ignores replays by version.
- **Hub 409 version gap** (`khala_sync_hub_version_gap`): the hub's window
  edge expected an earlier version (e.g. after a hub reset). The daemon
  heals this itself — the 409 body carries `expectedFirstVersion` and
  capture re-pushes from there (test: "hub 409 version gap heals by
  re-pushing from the hub's expectation"). If the hub's expectation is
  already behind the Postgres retained window, capture logs the scope error
  and leaves the checkpoint — it never fabricates a partial log; clients
  heal via re-bootstrap. Persistent 409 loops on one scope = inspect that
  scope's hub window and retained window for a mismatch.
- **One scope failing is isolated**: other scopes keep advancing; fix the
  failing scope without stopping the daemon.

## Hub DO reset procedure

`KhalaSyncHubDO` is a cache/fan-out layer ONLY — Postgres is authoritative
and no business writes originate in the hub. Losing or resetting a hub's
storage loses nothing durable.

Semantics after a reset (all test-proven in
`workers/api/src/khala-sync-hub-do.test.ts`):

- An **empty-window** hub answers ANY log cursor with the typed
  behind-window error, and the public `/api/sync/log` route falls through
  to Postgres — reads keep working during rehydration.
- Live sockets connecting to an empty hub stay open; the first append
  decides catch-up vs `MustRefetch`.
- The window **rehydrates from capture**: the hub's gap check 409s the
  first post-reset append with `expectedFirstVersion`, capture re-pushes
  from there, and the window rebuilds mid-stream. No manual backfill step
  exists or is needed.

Procedure: there is no routine "reset hub" command. If a scope's hub is
corrupt/wedged, the acceptable interventions are (a) redeploying the Worker
(DO code refresh; storage persists), or (b) deleting the DO's storage via a
deliberate admin change — after which the fall-through + capture-rehydrate
semantics above take over automatically. Verify recovery with the internal
hub log route and the capture checkpoint lag query.

## Access revocation (KS-7.1, #8305)

Scope-read authorization is LIVE-AT-READ: every `/api/sync/log`,
`/api/sync/bootstrap`, and `/api/sync/connect` request re-runs the KS-7.1
resolver (D1 team membership, agent_runs ownership, `khala_sync_scope_owners`
for fleet scopes), so a revoked user is denied on their very next request
with no operator action. What live-at-read does NOT cover is a socket that
is ALREADY connected to a scope's hub — that is the access-changed trigger's
job.

After ANY change that revokes scope access — removing/deactivating a
`team_memberships` row (there is no in-Worker removal route today;
memberships are operator-managed), or deleting a `khala_sync_scope_owners`
row — fire the trigger for each affected scope:

```
curl -sS -X POST https://openagents.com/api/internal/khala-sync/hub/access-changed \
  -H "Authorization: Bearer $ADMIN_API_TOKEN" \
  -H "content-type: application/json" \
  -d '{"scope":"scope.team.<teamId>"}'
```

Expected response: `{ "ok": true, "notified": <n>, "scope": ... }`. The
scope's hub broadcasts `MustRefetch(access_changed)` to EVERY connected
socket and closes them all (the hub holds no identity, so revocation is
scope-wide); sockets re-authenticate through the KS-7.1 resolver on
reconnect — still-authorized clients re-bootstrap and resume, revoked
clients get a 403, clear their scope-local durable state, and park in the
terminal `denied` phase (SPEC §7 invariant 7).

Worker write paths that revoke access in the future MUST call
`notifyKhalaSyncHubAccessChangedBestEffort` (fail-soft; a hub failure never
fails the revocation write) after their commit instead of relying on this
manual step. Trigger failure is degraded, not unsafe: live sockets keep the
old tail until their next reconnect/bootstrap, but no NEW read succeeds.

## Hyperdrive pool saturation

Symptoms: `/api/sync/push` and `/api/sync/log` returning 503
`storage_unavailable` (the typed retryable mapping for connection-class
failures), rising `latencyMs` on `/api/internal/khala-sync/db-smoke`,
Hyperdrive dashboard (Cloudflare → Hyperdrive → config
`khala-sync-prod`) showing origin connection pressure, while Cloud SQL
`num_backends` sits at its ceiling.

Response, in order:

1. Confirm it is pool pressure, not the origin being down: db-smoke +
   Cloud SQL instance status.
2. Look for a runaway direct-connection consumer eating backend slots
   (capture, compact, migrate, ad hoc psql) — direct paths and the
   Hyperdrive pool share the same instance `max_connections`.
3. Check for long-running transactions holding pooled connections
   (`SELECT * FROM pg_stat_activity WHERE state <> 'idle' ORDER BY
   xact_start`): mutators MUST be single-transaction and short (SPEC §4,
   MUTATORS.md); a slow mutator is a bug to fix, not a pool to grow.
4. Only then consider raising instance `max_connections` / tier, and record
   the change on an issue.

Clients treat 503 `storage_unavailable` as retryable with backoff (session
tests cover transient-failure retry with the queue intact), so a short
saturation window degrades to latency, not data loss.

## Pylon dispatch domain cutover (KS-8.1, #8307)

The first KS-8 domain migration: `pylon_api_assignments` /
`pylon_api_events` / `pylon_api_registrations` (D1) → `pylon_assignments` /
`pylon_assignment_events` / `pylon_registrations` (Postgres, khala-sync
migration `0005_pylon_dispatch.sql`). Machinery:
`apps/openagents.com/workers/api/src/pylon-dispatch-store.ts` (dual-write
wrapper + Postgres store) and
`packages/khala-sync-server/scripts/backfill-pylon.ts` (backfill + verify).

Flags (Worker vars; see `WorkerBindings`):

- `KHALA_SYNC_PYLON_DUAL_WRITE` — default **on** wherever `KHALA_SYNC_DB`
  exists; `off|0|false|disabled` disables the mirror.
- `KHALA_SYNC_PYLON_READS` — default `d1`; `compare` reads both, serves D1,
  logs `khala_sync_pylon_read_compare_mismatch`; `postgres` serves reads
  from Postgres with bounded retry (50/150ms) and D1 fallback on exhaustion.

Flag-flip order — never skip a step, each step soaks before the next:

1. **Dual-write on** (default after KS-8.1 lands + `0005` applied via the
   migration runner). Watch `khala_sync_pylon_dual_write_failed` in Worker
   logs — that event IS the drift metric; a nonzero steady rate blocks
   progression.
2. **Backfill**: from `packages/khala-sync-server/`,
   `KHALA_SYNC_DATABASE_URL=<direct-url> bun scripts/backfill-pylon.ts`
   (wrangler-auth'd; rowid-cursor resumable via
   `.pylon-backfill-state.json`). Run it a SECOND time (`--restart`) as the
   catch-up sweep once dual-write has covered the whole window.
3. **Verify**: `bun scripts/backfill-pylon.ts --verify` — exact row counts,
   per-state/kind/status tallies, newest-50 row-hash comparison. Post the
   output on the migration issue. Exact or explain; no cutover on a red
   verify.
4. **Compare reads**: set `KHALA_SYNC_PYLON_READS=compare`; soak until the
   mismatch log is silent over a representative window (include a fleet
   dispatch burst).
5. **Postgres reads**: set `KHALA_SYNC_PYLON_READS=postgres`. The dispatch
   gate (owner-registration + capacity reads — the June-29 503 victims) now
   reads Postgres with retry headroom; D1 remains the write authority and
   the fallback.
6. **Decommission LATER**: dropping the D1 tables (and moving write
   authority) is a separate follow-up issue on epic #8282 — never in the
   same change as a read cutover. Until then rollback is one flag flip
   back to `d1`.

Rollback at ANY step: set `KHALA_SYNC_PYLON_READS=d1` (reads) and/or
`KHALA_SYNC_PYLON_DUAL_WRITE=off` (writes). D1 authority is never behind.

## What this runbook does NOT cover

- Deploying the Worker/hub DO: `docs/DEPLOYMENT.md` (deploy:safe gate).
- Writing mutators: [`MUTATORS.md`](./MUTATORS.md).
- Invariant statuses and their enforcing tests:
  `apps/openagents.com/INVARIANTS.md` "Khala Sync (SPEC §7 invariant set)".
- Load testing (KS-9.1 #8310) and behavior-contract sweeps (KS-9.2 #8311)
  land their own procedures on those issues.
