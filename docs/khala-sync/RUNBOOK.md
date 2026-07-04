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
- `KHALA_SYNC_PYLON_READS` — committed deployment value `postgres` after the
  #8315 cutover; unset/unknown still fails closed to `d1` in code. `compare`
  reads both, serves D1, logs `khala_sync_pylon_read_compare_mismatch`;
  `postgres` serves reads from Postgres with bounded retry (50/150ms) and D1
  fallback on exhaustion.

Flag-flip order — never skip verification evidence before a read cutover:

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
5. **Postgres reads**: set `KHALA_SYNC_PYLON_READS=postgres`. As of the
   #8315 cutover config this is committed for production and staging. The
   dispatch gate, runner-status spine, and raw Codex proof metadata reads now
   read Postgres with retry headroom; D1 remains the write authority and
   fallback.
6. **Decommission in KS-8.19 only**: owner direction on 2026-07-04 skips
   the per-domain soak/drop tickets (#8331/#8333-style follow-ups) so the
   migration fanout keeps moving. Until the final D1 retirement sweep,
   rollback is one flag flip back to `d1`; do not destructively drop D1
   tables while fallback/compatibility paths still exist.

Rollback at ANY step: set `KHALA_SYNC_PYLON_READS=d1` (reads) and/or
`KHALA_SYNC_PYLON_DUAL_WRITE=off` (writes). D1 authority is never behind.

## Pylon control-plane remainder backfill (KS-8.4, #8315)

The KS-8.4 substrate migration is
`0009_pylon_control_plane_remainder.sql`. It creates Postgres twins for the
Pylon control-plane tables not owned by KS-8.1: quarantines, marketplace
intake/assignment/triage rows, provider job lifecycle, runner status,
capacity-funnel snapshots, Spark payout targets, raw Codex event metadata
indexes, runner sessions, and fleet alerts. Raw Codex event payload bodies
remain in R2; only metadata refs, ordering keys, sizes, and digests are copied
to Postgres.

Backfill/verify mechanics:

```sh
cd packages/khala-sync-server
KHALA_SYNC_DATABASE_URL="<direct-url>" \
  bun scripts/backfill-pylon-control-plane.ts

# catch-up sweep after the relevant dual-write mirrors have been on
KHALA_SYNC_DATABASE_URL="<direct-url>" \
  bun scripts/backfill-pylon-control-plane.ts --restart

# exact verification before any read cutover
KHALA_SYNC_DATABASE_URL="<direct-url>" \
  bun scripts/backfill-pylon-control-plane.ts --verify

# raw Codex metadata queue reconciliation:
# exact aggregate parity plus per-turn chunk-chain contiguity
KHALA_SYNC_DATABASE_URL="<direct-url>" \
  bun scripts/backfill-pylon-control-plane.ts --verify --raw-event-reconcile

# after deploying a source fix, prove only newer chunk chains while keeping
# the default all-history gate available for final historical classification
KHALA_SYNC_DATABASE_URL="<direct-url>" \
  bun scripts/backfill-pylon-control-plane.ts --verify --raw-event-reconcile \
    --raw-event-gap-latest-observed-since "2026-07-04T18:00:00.000Z"

# after classifying old source gaps, accept only duplicate-free historical
# chains observed before the cutoff; newer/unknown or duplicate gaps stay red
KHALA_SYNC_DATABASE_URL="<direct-url>" \
  bun scripts/backfill-pylon-control-plane.ts --verify --raw-event-reconcile \
    --raw-event-accept-historical-gaps-before "2026-07-04T00:00:00.000Z"
```

Use `--table <target-table>` for a single table and `--local` for a local D1
smoke. The target table names are the Postgres names, e.g.
`pylon_quarantines`, `pylon_codex_raw_event_chunks`, or `fleet_alerts`.

Verification output covers row counts, per-domain tallies, and newest-N row
hashes. Add `--raw-event-reconcile` after the raw-event metadata queue has
live traffic; it compares D1 and Postgres raw-event/chunk aggregates by
assignment/lease/pylon/turn and proves each recorded chunk chain is contiguous
for both stores. The first #8315 live mirror slices cover D1-first Worker
writes for
assignment-derived provider lifecycle, explicit provider lifecycle updates,
Pylon quarantines, Pylon marketplace intake/assignment/triage writes, raw
Spark payout target registrations, scheduled
`PylonCapacityFunnel.recordSnapshots` capacity-funnel snapshot upsert/prune
writes, registered-agent `pylon_agent_runner_status_events` ingest, and
Artanis Fleet tick `pylon_agent_runner_status_events` writes, plus
`fleet_alerts` cron alert rows from FleetBurnStallDetector and
ServingRateMonitor behind `KHALA_SYNC_PYLON_DUAL_WRITE`. Raw Codex event
payloads now stay on the request path only long enough to land in R2; metadata
rows are enqueued through `PYLON_CODEX_RAW_EVENT_METADATA_QUEUE`, then the
consumer writes the D1 index and fail-soft mirrors Postgres when the same
dual-write flag is armed. Runner-status spine reads now participate in
`KHALA_SYNC_PYLON_READS`: `compare` serves D1 and adds a Postgres shadow read
with drift diagnostics, while `postgres` serves the fleet-status spine from
Cloud SQL with bounded retry and D1 fallback. Pylon Codex proof and
trace-status closeout reads now use the same flag for their raw-event metadata
sections: `compare` serves D1 with Postgres-shadow source refs and drift logs,
and `postgres` serves those metadata sections from Cloud SQL with bounded D1
fallback. The live #8315 read cutover is the committed
`KHALA_SYNC_PYLON_READS=postgres` Worker var. Do not treat a green backfill or
read cutover alone as permission to drop D1 tables; destructive retirement is
deferred to KS-8.19, not a per-domain blocker.

Live raw Codex metadata note (2026-07-04): production row parity is established
for `pylon_codex_raw_events` and `pylon_codex_raw_event_chunks`, and aggregate
parity matches between D1 and Postgres. The raw-event reconcile command still
reports historical source chunk-chain gaps: 678 unique per-turn chains are
non-contiguous under start-at-1 semantics, mirrored identically into both
stores. This is not Postgres mirror drift. A Pylon runner fix now keeps a chunk
index unconsumed until its event-chunk reporter succeeds, so transient chunk
send failures retry under the same next index instead of leaving future holes.
The reconcile output separates D1, Postgres, shared, and unique gap counts so a
mirrored source gap is not mistaken for two independent failures. It also
classifies the unique gapped chains by whether a final turn-event row exists
and by shape: missing first chunk, internal missing chunk, or duplicate chunk
index. With start-at-1 semantics, the classified production set is 678 unique
chains: 511 have a turn-event row, 167 are live-stream-only/no-turn-row, 19 miss
the first chunk, 659 have an internal missing chunk, and 0 have duplicate
indexes. Use `--raw-event-gap-latest-observed-since` only to prove post-fix
traffic; use `--raw-event-accept-historical-gaps-before` only after posting the
classification evidence. That acceptance path still fails closed on
newer/unknown-observed gaps and duplicate chunk indexes.

## Agent runtime metadata domain cutover (KS-8.5, #8316)

The third KS-8 domain migration: the eight core agent-execution metadata
tables `agent_definitions` / `agent_definition_runs` /
`agent_definition_triggers` / `agent_runs` / `agent_run_events` /
`agent_traces` / `agent_goals` / `agent_goal_events` (D1) → same-named
Postgres twins (khala-sync migration `0010_agent_runtime.sql`).
Machinery: `apps/openagents.com/workers/api/src/agent-runtime-store.ts`
(row-level repository seam, fail-soft read-back mirror, `make*ForEnv`
call-site factories) and
`packages/khala-sync-server/scripts/backfill-agent-runtime.ts`
(backfill + verify). The issue's remaining tables (profiles, proposals,
owner claims, credentials, event ledger, acceptance jobs/verdicts) move
in the follow-up remainder lane — see MIGRATION_PLAN §3.2.

PRIVACY: `agent_traces` are owner-private. The Postgres twin carries
`visibility` / `owner_user_id` / consent columns verbatim; verify output
and Worker diagnostics reference trace_uuid keys and sha256 hashes ONLY
— never trajectory content. Do not paste raw rows into issues.

Flags (Worker vars):

- `KHALA_SYNC_AGENT_RUNTIME_DUAL_WRITE` — default **on** wherever
  `KHALA_SYNC_DB` exists; `off|0|false|disabled` disables the mirror.
- `KHALA_SYNC_AGENT_RUNTIME_READS` — default `d1`; routes the
  AgentDefinitionScheduler due-trigger scans (`listDueCronTriggers` /
  `listInboundWebhookTriggers`). `compare` reads both, serves D1, logs
  `khala_sync_agent_runtime_read_compare_mismatch`; `postgres` serves
  Postgres with bounded retry (50/150ms) and D1 fallback on exhaustion.
  All other domain reads stay on D1 until the decommission follow-up
  moves them with their own re-derived read paths.

Flag-flip order — never skip a step, each step soaks before the next:

1. **Dual-write on** (default after KS-8.5 lands + `0010` applied via the
   migration runner). Watch `khala_sync_agent_runtime_dual_write_failed`
   in Worker logs — that event IS the drift metric; a nonzero steady
   rate blocks progression.
2. **Backfill**: from `packages/khala-sync-server/`,
   `KHALA_SYNC_DATABASE_URL=<direct-url> bun
   scripts/backfill-agent-runtime.ts` (wrangler-auth'd; rowid-cursor
   resumable via `.agent-runtime-backfill-state.json`). Run it a SECOND
   time (`--restart`) as the catch-up sweep once dual-write has covered
   the whole window.
3. **Verify**: `bun scripts/backfill-agent-runtime.ts --verify` — exact
   row counts, per-run/per-goal EVENT-CHAIN comparison (count / distinct
   / min / max per parent — the KS-8.5 contiguity acceptance), trace
   content-hash sample + visibility/consent tallies, goal usage sums,
   newest-50 row hashes. Post the output on the migration issue. Exact
   or explain; no cutover on a red verify.
4. **Compare reads**: set `KHALA_SYNC_AGENT_RUNTIME_READS=compare`; soak
   until the mismatch log is silent over a window that includes real
   scheduler ticks (cron due-scans fire every minute).
5. **Postgres reads**: set `KHALA_SYNC_AGENT_RUNTIME_READS=postgres`.
   The scheduler due-trigger scan now reads Postgres with retry
   headroom; D1 remains the write authority and the fallback.
6. **Remainder/backfill separately, retire later**: the remaining
   profile/proposal/owner-claim/credential/event-ledger/acceptance tables
   have their own backfill lane (#8334, below). Dropping D1 tables and
   deleting flags is deferred to KS-8.19 (#8330), not a per-domain
   soak/drop gate. Until then rollback is one flag flip back to `d1`.

Rollback at ANY step: set `KHALA_SYNC_AGENT_RUNTIME_READS=d1` (reads)
and/or `KHALA_SYNC_AGENT_RUNTIME_DUAL_WRITE=off` (writes). D1 authority
is never behind.

## Agent runtime remainder backfill (KS-8.5 follow-up, #8334)

The first #8334 machinery slice is khala-sync migration
`0012_agent_runtime_remainder.sql` plus
`packages/khala-sync-server/scripts/backfill-agent-runtime-remainder.ts`.
It covers the tables deliberately left out of the first core metadata
slice: `agent_profiles`, `agent_credentials`, `agent_owner_claims`,
`agent_owner_x_claim_challenges`, `agent_proposals`,
`event_ledger_entries`, `khala_acceptance_jobs`, and
`khala_acceptance_verdicts`.

Privacy: `agent_credentials` is secret-bearing. The verifier hashes row
bytes for equality, but output must remain row-key/sha256 only. Do not
paste `token_hash`, job payloads, proposal bodies, event payload summaries,
or credential material into issues or logs.

Backfill from `packages/khala-sync-server/` over a direct Postgres URL:

```sh
KHALA_SYNC_DATABASE_URL="<direct-url>" \
  bun scripts/backfill-agent-runtime-remainder.ts

# Catch-up sweep after the mirror/cutover window, or when intentionally
# restarting from D1 rowid 0:
KHALA_SYNC_DATABASE_URL="<direct-url>" \
  bun scripts/backfill-agent-runtime-remainder.ts --restart
```

Verify:

```sh
KHALA_SYNC_DATABASE_URL="<direct-url>" \
  bun scripts/backfill-agent-runtime-remainder.ts --verify --verify-newest 50
```

The verify command checks exact row counts, scalar tallies per table,
newest-N row hashes, and per-owner `event_ledger_entries.ordering_sequence`
density (`count == distinct == max - min + 1`). It also checks the old
D1 rewrite artifact `event_ledger_entries_next`: absent or empty is clean;
any remaining rows are drift and must be explained before cutover.

Runtime mirror status: `event_ledger_entries` ingestion and handled-state
updates now use the #8334 fail-soft mirror seam when
`KHALA_SYNC_AGENT_RUNTIME_REMAINDER_DUAL_WRITE` is not disabled and the
`KHALA_SYNC_DB` binding exists. D1 remains authority; mirror failures emit
`khala_sync_agent_runtime_remainder_dual_write_failed` with row keys only
and never fail the request. The remaining profile, credential, owner-claim,
proposal, and acceptance job/verdict write seams are still #8334 work.

This lane does **not** drop D1 tables. Runtime write-authority movement,
read cutover, flag deletion, and destructive retirement remain explicit
follow-up work; D1 retirement is consolidated into KS-8.19 (#8330).

## Artanis supervision domain cutover (KS-8.6, #8317)

All twenty `artanis_*` tables (D1) → same-named Postgres twins
(khala-sync migration `0011_artanis_domain.sql`). Machinery:
`apps/openagents.com/workers/api/src/artanis-domain-store.ts` (the
`ArtanisDatabase` seam: registry-driven Postgres converge store,
`mirrorArtanisRows` fail-soft dual-write, `artanisRead` flag routing) and
`packages/khala-sync-server/scripts/backfill-artanis.ts` (backfill +
verify). Six of the ~23 every-minute cron tasks are Artanis ticks
(`ArtanisScheduledRunner.runTick`, `ArtanisResponder.scan`,
`ArtanisResponder.compose`, `ArtanisAdmin.tick`,
`ArtanisAdmin.closeoutVerifier`, `ArtanisFleet.tick`); they mirror on
every tick today and keep D1 authority until step 5.

Flags (Worker vars):

- `KHALA_SYNC_ARTANIS_DUAL_WRITE` — default **on** wherever
  `KHALA_SYNC_DB` exists; `off|0|false|disabled` disables the mirror.
- `KHALA_SYNC_ARTANIS_READS` — default `d1`; `compare` reads both, serves
  D1, logs `khala_sync_artanis_read_compare_mismatch`; `postgres` serves
  the seam-routed reads from Postgres with bounded retry (50/150ms) and
  D1 fallback on exhaustion.

Fail-soft invariant: `mirrorArtanisRows` NEVER throws — a Postgres outage
degrades to D1-only with `khala_sync_artanis_dual_write_failed`
diagnostics, preserving the operator-chat fail-soft precedent (2d46d808).
A tick, a chat turn, or a spend decision must never fail because the
mirror did.

Flag-flip order — never skip a step, each step soaks before the next:

1. **Dual-write on** (default after KS-8.6 lands + `0011` applied via the
   migration runner). Watch `khala_sync_artanis_dual_write_failed` in
   Worker logs — that event IS the drift metric; a nonzero steady rate
   blocks progression.
2. **Backfill**: from `packages/khala-sync-server/`,
   `KHALA_SYNC_DATABASE_URL=<direct-url> bun scripts/backfill-artanis.ts`
   (wrangler-auth'd; rowid-cursor resumable via
   `.artanis-backfill-state.json`). Run it a SECOND time (`--restart`) as
   the catch-up sweep once dual-write has covered the whole window.
   Optional pre-step: decide `artanis_health_snapshots` /
   `artanis_runtime_snapshots` retention (Analytics Engine or a bounded
   window) BEFORE the sweep — bounding retention first shrinks the port.
3. **Verify**: `bun scripts/backfill-artanis.ts --verify` — exact row
   counts, per-state tallies, newest-50 row-hash comparison across all
   twenty tables. Post the output on the migration issue. Exact or
   explain; no cutover on a red verify.
4. **Compare reads**: set `KHALA_SYNC_ARTANIS_READS=compare`; soak until
   the mismatch log is silent over a window that includes all six cron
   ticks firing (one full minute cadence is enough to touch every tick
   family; include an operator chat turn and a responder scan with real
   candidates).
5. **Postgres reads + tick re-homing**: set
   `KHALA_SYNC_ARTANIS_READS=postgres`. Landing requirement before this
   flip: one-tick shadow replay per tick family yields identical decisions
   from both stores, and tick-chain contiguity holds (the contract suite's
   double-fire and read-equivalence cases are the CI half of that
   evidence; the prod half is the compare-mode soak). The analytics JOIN
   readers (`artanis-tick-streak.ts`,
   `artanis-distillation-dataset-receipt.ts`) and dashboard aggregations
   are still D1-direct at this step and move with the decommission
   follow-up.
6. **Decommission LATER**: dropping the twenty D1 tables (and moving write
   authority) is the follow-up issue filed off #8317 — never in the same
   change as a read cutover. Until then rollback is one flag flip back to
   `d1`.

Rollback at ANY step: set `KHALA_SYNC_ARTANIS_READS=d1` (reads) and/or
`KHALA_SYNC_ARTANIS_DUAL_WRITE=off` (writes). D1 authority is never
behind.

## Public tokens-served projection (KS-6.3, #8304)

The public "Khala Tokens Served" counter
(`GET /api/public/khala-tokens-served`) serves the
`scope.public.tokens-served` projection (`khala_sync_public_counters`
through `KHALA_SYNC_DB`) instead of the old full-table D1 SUM. The ingest
write paths bump the counter exact-once per `token_usage_events` row (an
idempotency-key guard insert in the same Postgres transaction, invariant 8)
and the route fails OPEN to the live D1 SUM whenever the projection is
unavailable.

**Bring-up order (first deploy of this lane):**

1. **Migrate:** apply `0006_khala_sync_public_counters.sql` (staging →
   prod, normal migration procedure above).
2. **Deploy the Worker.** Safe in any order relative to the backfill: the
   increment path REFUSES (quietly, guard rolled back) while the counter
   row does not exist, and the public route keeps serving the live D1 SUM
   fallback — no behavior change until the backfill runs.
3. **Backfill (admin, once per environment):**
   `POST /api/internal/khala-sync/public-counters/tokens-served/reconcile`
   with body `{ "repair": true, "auditNote": "first bring-up backfill" }`
   (admin bearer). This sets projection = exact D1 SUM (audited,
   `source: backfill`), creates the counter row, and appends the first
   `public_counter` post-image to the scope. From this point increments
   apply.
4. **Verify:** `GET` the same route (read-only reconcile) — expect
   `inSync: true` (tiny transient drift can appear for events in flight
   during the backfill; re-check, then repair once if it persists), and
   `GET /api/public/khala-tokens-served` — expect the payload's staleness
   contract to read `rebuilt_on_transition` / `maxStalenessSeconds: 2`
   (fallback responses read `live_at_read` / `0`).

**Ongoing reconciliation (invariant 8):** the Worker cron runs a
detect-only reconcile every 15 minutes; drift logs the typed
`khala_sync_tokens_served_projection_drift` diagnostic and shows on the GET
reconcile route. The sweep NEVER overwrites the projection — repair is
always the explicit audited `POST { repair: true, auditNote }`, recorded in
`khala_sync_public_counter_repairs` with previous/new totals.

**Expected drift sources** (all self-heal at the next repair; the exact D1
SUM is always the truth): fail-soft producer misses while Postgres is
unreachable, and the two remaining low-volume direct-insert paths that do
not yet carry the producer hook
(`workers/api/src/builtin-compute-agent-grant.ts`,
`workers/api/src/provider-account-service-routes.ts`) — hook them when they
gain real volume. Persistent RE-GROWING drift after a repair means a hot
ingest path lost its producer wiring; check the
`khala_sync_tokens_served_projection_failed` diagnostics first.

## What this runbook does NOT cover

- Deploying the Worker/hub DO: `docs/DEPLOYMENT.md` (deploy:safe gate).
- Writing mutators: [`MUTATORS.md`](./MUTATORS.md).
- Invariant statuses and their enforcing tests:
  `apps/openagents.com/INVARIANTS.md` "Khala Sync (SPEC §7 invariant set)".
- Load testing (KS-9.1 #8310) and behavior-contract sweeps (KS-9.2 #8311)
  land their own procedures on those issues.
