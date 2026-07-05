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
updates, `agent_profiles` / `agent_credentials` registration and
credential-touch paths, and `agent_owner_claims` /
`agent_owner_x_claim_challenges` claim/verification paths, plus
`agent_proposals` submission and transition paths, acceptance job
enqueue/lease/ack paths, and acceptance verdict backfills now use the #8334
fail-soft mirror seam when
`KHALA_SYNC_AGENT_RUNTIME_REMAINDER_DUAL_WRITE` is not disabled and the
`KHALA_SYNC_DB` binding exists. D1 remains authority; mirror failures emit
`khala_sync_agent_runtime_remainder_dual_write_failed` with row keys only
and never fail the request. Credential diagnostics stay key-only in logs:
token hashes are copied only as private row data and are not printed.
Delivered acceptance-job acknowledgements delete the mirrored Postgres queue
row after the D1 queue deletes it, while retryable acknowledgements mirror
the returned-pending row.

Live closeout evidence (2026-07-04, #8334): Worker deploy `deploy:safe`
completed and produced production version
`afaf8272-a654-4dd6-ba1c-69418f12dcae`; `curl -fsSI
https://openagents.com/` returned HTTP 200; the served concrete asset
`/assets/index-DWcdsn2N.js` returned HTTP 200; the deployed Worker
Hyperdrive smoke `/api/internal/khala-sync/db-smoke` returned `ok: true`
with 12 Khala Sync tables; production backfill touched 416
`agent_profiles`, 418 `agent_credentials`, 14 `agent_owner_claims`, 5
`agent_owner_x_claim_challenges`, and 1 `agent_proposals` row, while
`event_ledger_entries`, `khala_acceptance_jobs`, and
`khala_acceptance_verdicts` were empty; `--verify --verify-newest 50`
reported clean counts/hashes for all eight tables and
`event_ledger_entries_next` absent-or-empty. Do not paste private direct
database URLs, credential token hashes, payloads, or bearer tokens into this
evidence trail.

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

## Treasury settlement domain cutover (KS-8.8, #8319)

All 27 live money tables (treasury_transactions, the six `nexus_*`
payout-authority ledgers, the forum money half — money actions, payment
events, receipts, L402 challenges/redemptions, direct tips + webhook
events, recipient wallets, settlement claims — `x_claim_reward_ledger`,
`agent_claim_reward_ledger`, `agent_balances`, `labor_escrows` +
`labor_escrow_receipts`, partner/site-referral payout ledgers +
`partner_agreements`, `revenue_event_provenance`, and the two
`mpp_*_replay` guards) → same-named Postgres twins (khala-sync migration
`0016_treasury_domain.sql`). Machinery:
`apps/openagents.com/workers/api/src/treasury-domain-store.ts` (the
`TreasuryDatabase` seam: registry-driven converge store,
`mirrorTreasuryRows` fail-soft dual-write, `treasuryRead` flag routing,
plus the `LedgerStatement.mirror` annotations in `payments-ledger.ts`)
and `packages/khala-sync-server/scripts/backfill-treasury.ts` (backfill +
money-exact verify). Six money crons mirror on every tick today and keep
D1 authority until step 5: `TipsSweep.runTick`,
`TipsBuffer.reconcileForwarding`, `TipsBuffer.backingInvariant`,
`TreasuryTransactions.reconcilePending`,
`XClaimRewardTreasuryDispatcher.runTick`,
`ForumDirectTips.archiveStaleRecoveries`.

**THIS IS THE HIGHEST-STAKES DOMAIN.** Non-negotiables that hold at every
step: D1 is the SOLE payout/settlement authority during dual-write; the
Postgres twin is a best-effort mirror that copies resolved D1 rows and can
never invent an amount, settlement state, idempotency key, or receipt;
every side-effect-bearing scan (payout dispatch, sweep candidates,
pending-transaction reconcile) reads exactly ONE store — they carry no
Postgres twin, so no flag value can double-dispatch a payout; and public
receipt endpoints (`/direct-tips` evidence, partner/site payout receipts,
nexus payment-authority receipts) must stay continuously servable through
every flip.

Flags (Worker vars) — **every flip below is an EPIC-GATED ops decision on
[#8282](https://github.com/OpenAgentsInc/openagents/issues/8282), never a
code default**:

- `KHALA_SYNC_TREASURY_DUAL_WRITE` — default **on** wherever
  `KHALA_SYNC_DB` exists; `off|0|false|disabled` disables the mirror.
- `KHALA_SYNC_TREASURY_READS` — default `d1`; `compare` reads both, serves
  D1, logs `khala_sync_treasury_read_compare_mismatch` (and turns the
  every-tick TipsBuffer backing-invariant SUM into a continuously-running
  msat reconciliation probe); `postgres` serves the seam-routed reads from
  Postgres with bounded retry (50/150ms) and D1 fallback on exhaustion.

Fail-soft invariant: `mirrorTreasuryRows` NEVER throws — a Postgres outage
degrades to D1-only with `khala_sync_treasury_dual_write_failed`
diagnostics (row KEYS only; replay-guard payment identifiers are
redacted). A payout, a tip, or a settlement cron must never fail because
the mirror did.

Flag-flip order — never skip a step, each step soaks before the next:

1. **Dual-write on** (default after KS-8.8 lands + `0016` applied via the
   migration runner). Watch `khala_sync_treasury_dual_write_failed` in
   Worker logs — that event IS the drift metric; a nonzero steady rate
   blocks progression.
2. **Backfill**: from `packages/khala-sync-server/`,
   `KHALA_SYNC_DATABASE_URL=<direct-url> bun scripts/backfill-treasury.ts`
   (wrangler-auth'd; rowid-cursor resumable via
   `.treasury-backfill-state.json`). Run it a SECOND time (`--restart`) as
   the catch-up sweep once dual-write has covered the whole window.
3. **Verify — money reconciliation is the acceptance**:
   `bun scripts/backfill-treasury.ts --verify` — exact row counts,
   per-(state, rail) tallies WITH exact money-column SUMs
   (millisat/sat/cent/minor-unit, compared as bigint), and newest-50
   row-hash comparison across all 27 tables. This is the payout-intent set
   equality + settled-totals-to-the-millisat + replay-guard key-set
   equality evidence the issue requires. Post the output on the migration
   issue. Exact or explain; NO cutover on a red verify.
4. **Compare reads**: set `KHALA_SYNC_TREASURY_READS=compare`; soak until
   the mismatch log is silent over a window that includes all six money
   crons firing, at least one live tip settling end-to-end (submit →
   webhook reconcile → settlement claim → public receipt), and one payout
   intent reaching `settled`.
5. **Postgres reads + cron re-homing**: set
   `KHALA_SYNC_TREASURY_READS=postgres`. Landing requirement before this
   flip: shadow-compared public receipts byte-identical (modulo
   timestamps) under compare mode, AND the dispatcher/sweep/reconcile
   scans gain their Postgres twins in a dedicated change (they are
   deliberately D1-only in the KS-8.8 lane) — those scans re-home
   atomically with this flip, never before it.
6. **Decommission LATER**: dropping the 27 D1 tables (and moving write
   authority) is consolidated into KS-8.19
   [#8330](https://github.com/OpenAgentsInc/openagents/issues/8330) —
   never in the same change as a read cutover. Until then rollback is one
   flag flip back to `d1`.

Rollback at ANY step: set `KHALA_SYNC_TREASURY_READS=d1` (reads) and/or
`KHALA_SYNC_TREASURY_DUAL_WRITE=off` (writes). D1 authority is never
behind.

Live closeout (2026-07-04, #8319): source commit `87d16a6ee7` was deployed
through `deploy:safe` after full `check:deploy`; staging Worker version
`a423fc15-16a6-492b-9559-bb78e26160ed`, production Worker version
`1bcd048d-de0d-4a1e-a108-f79b4ba5e33f`. The direct migration runner
dry-ran then applied `0016_treasury_domain.sql` in staging and production
(`1 pending, 16 already applied` before apply; `applied 1, already
applied 16` after apply). Production smokes: homepage HTTP 200, concrete
asset `/assets/index-DWcdsn2N.js` HTTP 200, and internal Khala Sync
Hyperdrive smoke `{ ok: true, khalaSyncTables: 12, latencyMs: 127 }`.
Backfill copied the production corpus, then the required restart sweep
scanned the same rows with zero inserts. Production verify with
`--verify --verify-newest 50` ended `VERIFY OK: exact counts, per-state
money sums, and newest-N hashes match.` Read cutover remains epic-gated by
#8282, and destructive D1 retirement remains consolidated into #8330.

## Inference entitlements domain cutover (KS-8.9, #8320)

The free-tier/entitlement accounting on the inference serving path — the
15 `inference_*` tables, `builtin_compute_agent_quota_events`,
`orange_check_entitlements`, `agent_rate_limit_*` (4), and
`agent_search_*` (8; `agent_search_metric_events` is an Analytics Engine
candidate and is NOT migrated) — D1 → Postgres (khala-sync migration
`0013_inference_entitlements.sql`). Machinery:
`apps/openagents.com/workers/api/src/inference-entitlements-store.ts`
(fire-safe mirror + routed enforcement gate reads) and
`packages/khala-sync-server/scripts/backfill-inference-entitlements.ts`
(backfill + verify).

THIS CUTOVER CHANGES WHICH STORE **ENFORCES** allow/deny on every free /
public completion: a lost quota increment is a free-tier leak, a doubled
one is a false denial. Do the read flip in a LOW-TRAFFIC window and only
on zero-divergence compare evidence.

Flags (Worker vars):

- `KHALA_SYNC_ENTITLEMENTS_DUAL_WRITE` — default **on** wherever
  `KHALA_SYNC_DB` exists; `off|0|false|disabled` disables the mirror. The
  mirror is fire-safe: it never delays or fails a completion; failures log
  `khala_sync_entitlements_dual_write_failed` (the drift metric).
- `KHALA_SYNC_ENTITLEMENTS_READS` — default `d1` (gates run their inline
  D1 reads, zero added latency); `compare` serves D1 and schedules a
  shadow Postgres decision comparison OFF the response path, logging
  `khala_sync_entitlements_read_compare_mismatch`; `postgres` serves the
  six enforcement gate reads from Postgres with single-attempt D1
  fallback (`khala_sync_entitlements_postgres_read_fallback`).

Flag-flip order — never skip a step, each step soaks before the next:

1. **Dual-write on** (default after KS-8.9 lands + `0013` applied via the
   migration runner). Watch `khala_sync_entitlements_dual_write_failed`;
   a nonzero steady rate blocks progression.
2. **Backfill**: from `packages/khala-sync-server/`,
   `KHALA_SYNC_DATABASE_URL=<direct-url> bun
   scripts/backfill-inference-entitlements.ts` (wrangler-auth'd;
   rowid-cursor resumable via
   `.inference-entitlements-backfill-state.json`). Run it a SECOND time
   (`--restart`) as the catch-up sweep once dual-write has covered the
   whole window — the second sweep also converges the two
   non-event-keyed counters (`inference_free_key_mints`,
   `agent_search_cache_entries`).
3. **Verify**: `bun scripts/backfill-inference-entitlements.ts --verify`
   — exact row counts per table, per-group ("per-plan") tallies,
   newest-50 row hashes, AND the enforcement invariant
   tally = SUM(events) per key for free-tier usage / free-usage pool /
   earned allowance. Post the output on the migration issue. Exact or
   explain; no cutover on a red verify.
4. **Compare reads**: set `KHALA_SYNC_ENTITLEMENTS_READS=compare`; soak
   until the mismatch log is silent over a representative window that
   includes a free-tier burst (the §3.6 denial-decision shadow
   comparison: same request → same allow/deny). Zero divergence is the
   acceptance bar — these reads deny requests.
5. **Postgres reads** (LOW-TRAFFIC WINDOW): set
   `KHALA_SYNC_ENTITLEMENTS_READS=postgres`. The six enforcement gate
   reads now serve from Postgres; every gate stays fail-closed on error
   (premium/exemption/free deny; privacy fails closed TO PRIVATE) and
   falls back to the still-authoritative D1 on a Postgres fault.
6. **Decommission LATER**: dropping the D1 tables, moving write
   authority, routing the non-gate reads (admin lists, agent-search
   request/cache/quota reads, batch-job reads, receipts routes), and the
   `agent_search_metric_events` → Analytics Engine move are a separate
   follow-up issue on epic #8282 — never in the same change as a read
   cutover. Until then rollback is one flag flip back to `d1`.

Rollback at ANY step: set `KHALA_SYNC_ENTITLEMENTS_READS=d1` (reads)
and/or `KHALA_SYNC_ENTITLEMENTS_DUAL_WRITE=off` (writes). D1 authority is
never behind.
## Forum content domain cutover (KS-8.10, #8321)

The KS-8.10 domain migration: the thirteen forum content-core tables
`forum_boards` / `forum_categories` / `forum_forums` / `forum_topics` /
`forum_posts` / `forum_post_bodies` / `forum_post_revisions` /
`forum_actor_follows` / `forum_watches` / `forum_bookmarks` /
`forum_reports` / `forum_moderation_events` / `forum_context_links` (D1)
→ same-named Postgres twins (khala-sync migration
`0014_forum_content.sql`). Machinery:
`apps/openagents.com/workers/api/src/forum/forum-content-store.ts` (the
mirroring D1Database `forumContentDatabaseForEnv` — the forum
repository's `db: D1Database` parameter IS the seam; repository SQL is
untouched) and
`packages/khala-sync-server/scripts/backfill-forum-content.ts`
(backfill + verify). The issue's remaining tables (private messages, ACL
grants, trust edges/scores, score snapshots, notification reads, work
requests) move in the follow-up remainder lane — see MIGRATION_PLAN
§3.7. The forum MONEY tables belong to KS-8.8 and are not part of this
procedure.

Diagnostics are keys-and-hashes only (never post bodies): the drift
metric is `khala_sync_forum_dual_write_failed`; a write shape the
statement classifier cannot key logs
`khala_sync_forum_write_unclassified` — treat a nonzero rate of EITHER
as drift and re-run the backfill sweep after fixing.

Flags (Worker vars):

- `KHALA_SYNC_FORUM_DUAL_WRITE` — default **on** wherever
  `KHALA_SYNC_DB` exists; `off|0|false|disabled` disables the mirror.
- `KHALA_SYNC_FORUM_READS` — default `d1`. `compare` shadow-runs every
  scoped-table SELECT against the Postgres twin, SERVES D1, and logs
  `khala_sync_forum_read_compare_mismatch` /
  `khala_sync_forum_read_compare_failed` — this is the "public thread
  pages shadow-compared" cutover evidence. `postgres` serving is
  DEFERRED to the read-cutover follow-up (the forum read surface is
  domain-wide): setting it today behaves as `compare` and logs
  `khala_sync_forum_postgres_reads_deferred` once, so a premature flip
  can never serve an unproven read path.

Flag-flip order — never skip a step, each step soaks before the next:

1. **Dual-write on** (default after KS-8.10 lands + `0014` applied via
   the migration runner). Watch `khala_sync_forum_dual_write_failed` and
   `khala_sync_forum_write_unclassified` in Worker logs; a nonzero
   steady rate blocks progression.
2. **Backfill**: from `packages/khala-sync-server/`,
   `KHALA_SYNC_DATABASE_URL=<direct-url> bun
   scripts/backfill-forum-content.ts` (wrangler-auth'd; rowid-cursor
   resumable via `.forum-content-backfill-state.json` — post bodies are
   the long pole, safe to interrupt/resume). Run it a SECOND time
   (`--restart`) as the catch-up sweep once dual-write has covered the
   whole window.
3. **Verify**: `bun scripts/backfill-forum-content.ts --verify` — exact
   row counts, domain tallies (counter sums, state tallies, body byte
   totals), PER-TOPIC post-chain comparison (count / distinct / min /
   max post_number per topic), per-thread spot hashes over the 25 most
   recently bumped topics (`--verify-threads` to widen), and newest-50
   row hashes. Post the output on the migration issue. Exact or explain;
   no cutover on a red verify.
4. **Compare reads**: set `KHALA_SYNC_FORUM_READS=compare`; soak until
   the mismatch log is silent over a window that includes real forum
   traffic (agents poll the forum continuously, so a few hours is a real
   soak).
5. **Read cutover + remainder LATER**: serving reads from Postgres,
   migrating the remainder tables (private messages / ACLs / trust /
   score snapshots / notification reads / work requests — with the
   set-membership referential checks against KS-8.1 assignments and
   KS-8.8 tips), moving write authority, and dropping the D1 tables is
   the follow-up issue on epic #8282 — never in the same change as this
   lane. Until then rollback is one flag flip back to `d1`.

Rollback at ANY step: set `KHALA_SYNC_FORUM_READS=d1` (reads) and/or
`KHALA_SYNC_FORUM_DUAL_WRITE=off` (writes). D1 authority is never
behind.

## Sites content domain cutover (KS-8.12, #8323)

The KS-8.12 domain migration CORE: the fifteen sites content/builder
tables `site_projects` / `site_versions` / `site_deployments` /
`site_deployment_attempts` / `site_access_grants` / `site_events` /
`site_builder_sessions` / `site_builder_messages` / `site_builder_events`
/ `site_builder_phase_runs` / `site_builder_file_snapshots` /
`site_builder_previews` / `site_builder_artifacts` /
`site_builder_repair_attempts` / `site_builder_saved_versions` (D1) →
same-named Postgres twins (khala-sync migration `0020_sites_core.sql`).
Machinery:
`apps/openagents.com/workers/api/src/sites-content-store.ts` (the
mirroring D1Database `sitesContentDatabaseForEnv` — the sites modules'
`db: D1Database` parameter IS the seam; module SQL is untouched; unlike
the forum classifier this one also mirrors PARENT-keyed transitions —
`UPDATE site_deployments … WHERE site_id = ?` rollback/disable and the
site-library archival batch — by reading back all rows for the parent
key) and `packages/khala-sync-server/scripts/backfill-sites-content.ts`
(backfill + verify). The issue's remaining ~36 tables (content
satellites, `site_environment_values` — may carry SECRETS, invariant-9
handling — site commerce/`site_mdk_*` money tables which reference the
KS-8.7/KS-8.8 rails by ID and must never fork them, `targeted_site_*`,
`tenant_custom_hostnames`, legacy `deployments`/`deployment_events`)
move in the follow-up remainder lane #8357 — see MIGRATION_PLAN §3.9.

Diagnostics are keys-and-hashes only (never prompts, message bodies, or
snapshot preview text): the drift metric is
`khala_sync_sites_dual_write_failed`; a write shape the statement
classifier cannot key logs `khala_sync_sites_write_unclassified` — treat
a nonzero rate of EITHER as drift and re-run the backfill sweep after
fixing.

Flags (Worker vars):

- `KHALA_SYNC_SITES_DUAL_WRITE` — default **on** wherever
  `KHALA_SYNC_DB` exists; `off|0|false|disabled` disables the mirror.
- `KHALA_SYNC_SITES_READS` — default `d1`. `compare` shadow-runs every
  scoped-table SELECT against the Postgres twin, SERVES D1, and logs
  `khala_sync_sites_read_compare_mismatch` /
  `khala_sync_sites_read_compare_failed`. `postgres` serving is DEFERRED
  to the read-cutover follow-up (the sites read surface is domain-wide
  and live SITE SERVING reads must be inventoried first): setting it
  today behaves as `compare` and logs
  `khala_sync_sites_postgres_reads_deferred` once, so a premature flip
  can never serve an unproven read path.

Flag-flip order — never skip a step, each step soaks before the next:

1. **Dual-write on** (default after KS-8.12 lands + `0020` applied via
   the migration runner). Watch `khala_sync_sites_dual_write_failed` and
   `khala_sync_sites_write_unclassified` in Worker logs; a nonzero
   steady rate blocks progression.
2. **Backfill**: from `packages/khala-sync-server/`,
   `KHALA_SYNC_DATABASE_URL=<direct-url> bun
   scripts/backfill-sites-content.ts` (wrangler-auth'd; rowid-cursor
   resumable via `.sites-content-backfill-state.json` — builder message
   bodies, 4000-char snapshot preview text, and version asset manifests
   are the long pole, safe to interrupt/resume; `--batch-size` down if
   wrangler JSON pages get heavy). Run it a SECOND time (`--restart`) as
   the catch-up sweep once dual-write has covered the whole window.
3. **Verify**: `bun scripts/backfill-sites-content.ts --verify` — exact
   row counts, domain tallies (project/deployment status tallies,
   builder sequence sums, snapshot byte totals), PER-PROJECT VERSION
   CHAINS (count / distinct / min / max created_at per site — the
   KS-8.12 version-chain acceptance), the DEPLOYMENT STATE-MACHINE
   census (per-site per-status counts), BUILDER SEQUENCE CHAINS per
   session (messages / events / phase runs), and newest-50 row hashes.
   Post the output on the migration issue. Exact or explain; no cutover
   on a red verify.
4. **Compare reads**: set `KHALA_SYNC_SITES_READS=compare`; soak until
   the mismatch log is silent over a window that includes real builder
   traffic (an active site build exercises the hot satellite writes).
5. **Read cutover + remainder LATER**: serving reads from Postgres
   (AFTER inventorying live site-serving reads — most hit R2/KV
   already), migrating the remainder tables (satellites / environment
   values / commerce with money discipline / targeted sites / custom
   hostnames / legacy deployments), moving write authority, and
   dropping the D1 tables is follow-up #8357 on epic #8282 — never
   in the same change as this lane. Until then rollback is one flag
   flip back to `d1`.

Rollback at ANY step: set `KHALA_SYNC_SITES_READS=d1` (reads) and/or
`KHALA_SYNC_SITES_DUAL_WRITE=off` (writes). D1 authority is never
behind.
## CRM / email / enrichment domain cutover (KS-8.11, #8322)

The KS-8.11 domain migration: the 36 canonical CRM/email/enrichment tables
— `crm_*` (13), `email_*` (11), `subscriber_lists` + `list_subscribers`,
`business_outreach_*` (4), `exa_enrichment_*` (6) — D1 → Postgres
(khala-sync migration `0022_crm_email_domain.sql`). Machinery: the
`CrmEmailDatabase` union handle in
`apps/openagents.com/workers/api/src/crm-email-domain-store.ts`
(read-back fail-soft mirror + flag-routed reads) and
`packages/khala-sync-server/scripts/backfill-crm-email.ts` (resumable
backfill + PII-safe verify).

THIS DOMAIN CARRIES TWO COMPLIANCE GATES: (1) the send path must read
exactly ONE authoritative suppression/preference store at every moment of
the cutover — the seam's flag is consulted exactly once per read, so the
flip is atomic per-read; (2) campaign-send dedupe (enrollment × step
idempotency key) ports as the SAME unique constraint on Postgres, so no
store can double-email a real person. And it is a PII domain: rows carry
names/emails/notes. Postgres stores exactly what D1 stores; every
diagnostic and every verify line is keys/hashes/counts only —
email-valued keys appear as `sha256:<12 hex>` prefixes, never raw.

Flags (Worker vars):

- `KHALA_SYNC_CRM_DUAL_WRITE` — default **on** wherever `KHALA_SYNC_DB`
  exists; `off|0|false|disabled` disables the mirror. The mirror is
  fail-soft: an email send, a webhook ack, or a CRM import never fails
  because the mirror did; failures log `khala_sync_crm_dual_write_failed`
  (the drift metric).
- `KHALA_SYNC_CRM_READS` — default `d1` (all reads stay inline D1, zero
  added latency); `compare` serves D1 and logs
  `khala_sync_crm_read_compare_mismatch` (keys/hashes only); `postgres`
  serves seam-routed reads from Postgres with bounded retry (50/150ms)
  and D1 fallback (`khala_sync_crm_postgres_read_fallback`). Unknown
  values fall back to `d1` — the suppression gate never fails open on a
  typo.

Flag-flip order — every flip is an EPIC-GATED ops decision on
[#8282](https://github.com/OpenAgentsInc/openagents/issues/8282), never a
code default; each step soaks before the next:

1. **Dual-write on** (default after KS-8.11 lands + `0019` applied via
   the migration runner). Watch `khala_sync_crm_dual_write_failed`; a
   nonzero steady rate blocks progression.
2. **Backfill**: from `packages/khala-sync-server/`,
   `KHALA_SYNC_DATABASE_URL=<direct-url> bun
   scripts/backfill-crm-email.ts` (wrangler-auth'd; rowid-cursor
   resumable via `.crm-email-backfill-state.json`; `--table <name>` to
   scope, `--restart` to resweep). Run it a SECOND time (`--restart`) as
   the catch-up sweep once dual-write has covered the whole window.
3. **Verify**: `bun scripts/backfill-crm-email.ts --verify` — exact row
   counts per table, per-status tallies over non-PII columns, newest-50
   row hashes, and WHOLE-SET digests for the compliance-bearing tables
   (`crm_contacts`, `email_preferences`, `email_suppression_entries`,
   `list_subscribers`, `business_outreach_suppressions`) — suppression
   set equality proven without printing a single address. Exits non-zero
   on ANY mismatch. Post the (PII-safe) output on the migration issue.
   Exact or explain; no cutover on a red verify.
4. **Compare reads**: set `KHALA_SYNC_CRM_READS=compare`; soak until the
   mismatch log is silent over a window that includes real sends. The
   staging acceptance MUST include a deliberately suppressed send
   attempt: enter a test address into `email_suppression_entries`,
   attempt the send under compare mode, and confirm it is refused with
   zero compare mismatch on the gate read.
5. **Postgres reads** (LOW-TRAFFIC WINDOW): set
   `KHALA_SYNC_CRM_READS=postgres`. Repeat the suppressed-send probe
   FIRST thing after the flip — the gate must still refuse. Any Postgres
   fault falls back to the still-authoritative D1.
6. **Cron re-home + decommission LATER**: `EmailCampaignDispatcher.dispatchDue`
   already rides the seam (its claim/skip/suppress/sent writes mirror);
   moving write authority and dropping the D1 tables is consolidated into
   KS-8.19 [#8330](https://github.com/OpenAgentsInc/openagents/issues/8330)
   — never in the same change as a read cutover. Until then rollback is
   one flag flip back to `d1`.

Rollback at ANY step: set `KHALA_SYNC_CRM_READS=d1` (reads) and/or
`KHALA_SYNC_CRM_DUAL_WRITE=off` (writes). D1 authority is never behind.

## Khala Code product-state domain cutover (KS-8.13, #8324)

The KS-8.13 domain migration is the product-state lane where migration
also means Khala Sync adoption. Same-named Postgres twins are introduced
by khala-sync migration `0017_khala_code_product_state.sql` for
`thread_messages`, `thread_files`, `thread_file_message_refs`, `teams`,
`team_memberships`, `team_chat_messages`, `team_projects`,
`team_workspace_invites`, `prefilled_*`, `workroom_*`, `cloud_*`,
`khala_feedback`, `khala_head_to_head_snapshots`,
`khala_unsupported_requests`, Khala Code download/outside-run/
trace-plugin receipt tables, and `share_projections` /
`share_projection_recipients`.

Machinery:

- Worker seam:
  `apps/openagents.com/workers/api/src/khala-code-product-state-store.ts`.
  It wraps the existing D1 handle, lets the authoritative D1 write commit
  first, reads back the accepted row, converge-upserts the Cloud SQL twin,
  and appends Khala Sync changelog entries for `scope.team.<id>` and
  `scope.thread.<id>`.
- Shared registry:
  `packages/khala-sync-server/src/khala-code-product-state-tables.ts`
  owns column order and natural keys.
- Typed post-image projection:
  `packages/khala-sync-server/src/khala-code-product-state-projection.ts`
  owns scope routing and allowlist-maps each mirrored row into the
  PUBLIC-SAFE entity contracts in `@openagentsinc/khala-sync`
  (`khala-code.ts`: `KhalaCodeTeamEntity`,
  `KhalaCodeTeamMembershipEntity`, `KhalaCodeTeamChatMessageEntity`,
  `KhalaCodeThreadMessageEntity`, `KhalaCodeThreadFileEntity`, invites,
  projects, prefilled workspaces, share projections, file↔message refs).
  Raw D1 rows never ride a changelog post-image: invite `token_hash` and
  invitee emails, R2 `object_key`s, `metadata_json` blobs, team `credits`,
  and share `projection_json` payloads are structurally absent from the
  contracts, and a forbidden-material scan over the structural fields
  backstops the allowlist. The golden fixtures in
  `packages/khala-sync/fixtures/KhalaCode*.json` are the
  cross-implementation conformance contract for these shapes.
- Backfill/verifier:
  `packages/khala-sync-server/scripts/backfill-khala-code-product-state.ts`.
  The verifier also covers product-state tables that are currently
  backfill-only because no Worker writer is registered in source yet, such
  as workroom template rows and Khala Code download events.

Diagnostics are row-key only: `khala_sync_khala_code_state_dual_write_failed`
is the drift metric; `khala_sync_khala_code_state_write_unclassified`
means a D1 write touched a product-state table but the classifier could
not prove the row key. A nonzero steady rate blocks read/sync cutover.
`khala_sync_khala_code_state_projection_skipped` means a mirrored row
could not be allowlist-mapped into its contract entity (schema drift or
redaction-guard match) — the Cloud SQL twin still converged, only the
scope changelog entry was withheld; investigate before shadow evidence.

Flags:

- `KHALA_SYNC_KHALA_CODE_STATE_DUAL_WRITE` — default **on** wherever
  `KHALA_SYNC_DB` exists; `off|0|false|disabled|no` disables the mirror.

Cutover order:

1. **Apply migration `0017`** through the khala-sync migration runner,
   staging first, then production. Do not use Hyperdrive for migrations.
2. **Dual-write on** (default after the Worker with the seam is deployed
   and `KHALA_SYNC_DB` is present). Watch both diagnostics above.
3. **Backfill** from `packages/khala-sync-server/`:
   `KHALA_SYNC_DATABASE_URL=<direct-url> bun
   scripts/backfill-khala-code-product-state.ts`. The sweep is resumable
   via `.khala-code-product-state-backfill-state.json`.
4. **Catch-up sweep**: rerun with `--restart` after dual-write has been
   active across the window.
5. **Verify**: `bun scripts/backfill-khala-code-product-state.ts --verify`
   — exact row counts, newest-50 row hashes, active membership set
   equality, and ordered message-chain fingerprints for `team_chat_messages`
   and `thread_messages`. Post the JSON report on #8324. Exact or explain.
6. **Synced-scope shadow**: run a desktop/web client on the relevant
   `scope.team.<id>` and `scope.thread.<id>` scopes and compare its
   confirmed local store to the still-authoritative D1 reads for the same
   messages/files/memberships. A mismatch blocks read/poll retirement.
7. **Read/poll retirement later**: once the shadow evidence is green,
   the chat/sidebar clients may consume Khala Sync scopes as their primary
   source. D1 rollback/fallback remains explicit until final destructive
   retirement in KS-8.19 (#8330).

Rollback at ANY step: set `KHALA_SYNC_KHALA_CODE_STATE_DUAL_WRITE=off`.
Read authority is still D1, and the sync changelog can be repaired by
rerunning the backfill plus catch-up sweep after the bug is fixed.

## Training domain cutover (KS-8.15 core, #8326)

The Wave D training CORE: the seven `training_*` tables —
`training_runs` / `training_windows` / `training_window_events` /
`training_window_leases` / `training_verification_challenges` /
`training_verification_events` / `training_trace_contributions` (D1) →
same-named Postgres twins (khala-sync migration
`0019_training_domain.sql`). Machinery:
`apps/openagents.com/workers/api/src/training-domain-store.ts`
(row-level seam over the shared registry, fail-soft read-back mirror
wrapped around the three existing D1 stores at every write call site)
and `packages/khala-sync-server/scripts/backfill-training.ts`
(cursor-resumable backfill + verify). The gym/mullet/blueprint/replay/
mirrorcode remainder (~22 tables) moves in the follow-up lane #8355.

CORRECTNESS NOTE (window leases): double-lease = double-payout risk
upstream. In this lane the lease claim stays a D1-authoritative write
and Postgres is a byte-exact mirror. At full write cutover the claim
becomes a real `SELECT ... FOR UPDATE` row-lock transaction — port the
lock protocol deliberately then; never emulate the D1 dance in
Postgres mid-migration. Training receipts feed PUBLIC claims: verify
must be hash-exact, and the public run-summary / proof-replay /
activity-timeline reads stay on D1 authority until cutover so public
projections never regress mid-cutover.

Flags (Worker vars):

- `KHALA_SYNC_TRAINING_DUAL_WRITE` — default **on** wherever
  `KHALA_SYNC_DB` exists; `off|0|false|disabled` disables the mirror.
- `KHALA_SYNC_TRAINING_READS` — default `d1`; routes ONE scan:
  `listClaimableWindows` (the SelfServeWindowProducer.topUp cron this
  domain re-homes). `compare` reads both, serves D1, logs
  `khala_sync_training_read_compare_mismatch`; `postgres` serves
  Postgres with bounded retry (50/150ms) and D1 fallback. All other
  domain reads stay on D1 until the decommission follow-up.

Flag-flip order — never skip a step, each step soaks before the next:

1. **Dual-write on** (default after #8326 lands + `0019` applied via the
   migration runner). Watch `khala_sync_training_dual_write_failed` in
   Worker logs — that event IS the drift metric; a nonzero steady rate
   blocks progression.
2. **Backfill**: from `packages/khala-sync-server/`,
   `KHALA_SYNC_DATABASE_URL=<direct-url> bun scripts/backfill-training.ts`
   (wrangler-auth'd; rowid-cursor resumable via
   `.training-backfill-state.json`). Run it a SECOND time (`--restart`)
   as the catch-up sweep once dual-write has covered the whole window.
3. **Verify**: `bun scripts/backfill-training.ts --verify` — exact row
   counts, newest-50 full-row hashes, per-window window-event chain
   fingerprints + per-window lease-set fingerprint (the double-lease
   guard), per-challenge verification-event chain fingerprints (the
   contiguity acceptance), and challenge/contribution state tallies.
   Post the output on the migration issue. Exact or explain; no cutover
   on a red verify.
4. **Compare reads**: set `KHALA_SYNC_TRAINING_READS=compare`; soak
   until the mismatch log is silent over a window that includes real
   SelfServeWindowProducer ticks (the cron fires every minute).
5. **Postgres reads**: set `KHALA_SYNC_TRAINING_READS=postgres`. The
   claimable-window scan now reads Postgres with retry headroom; D1
   remains the write authority and the fallback.
6. **Remainder separately, retire later**: gym/mullet/blueprint/replay/
   mirrorcode move in #8355 (with the `gym_harbor_full_trace_archives`
   R2-split check and leaderboard recomputation). Dropping D1 tables,
   deleting flags, and the row-lock lease-claim port are deferred to
   KS-8.19 (#8330), not a per-domain soak/drop gate. Until then rollback
   is one flag flip back to `d1`.

Rollback at ANY step: set `KHALA_SYNC_TRAINING_READS=d1` (reads) and/or
`KHALA_SYNC_TRAINING_DUAL_WRITE=off` (writes). D1 authority is never
behind.

## Forge domain cutover (KS-8.16, #8327)

The KS-8.16 domain migration: ALL SIXTEEN `forge_*` tables —
coordination issues/PRs/status, dispatch leases, merge-queue ledger,
packfile archives (metadata only; raw bytes stay in R2), tenants, git
access tokens (+scopes), verification receipts, promotion decisions,
receive-pack intakes, canonical refs, object tips, ref locks, GitHub
mirror receipts (D1) → same-named Postgres twins (khala-sync migration
`0021_forge_domain.sql`). Machinery:
`apps/openagents.com/workers/api/src/forge-domain-store.ts` (the five
`makeForge*StoreForEnv` store-factory drop-ins — the forge stores ARE
the seam; their D1 SQL is untouched, and every write method read-back
mirrors its rows by composite key) and
`packages/khala-sync-server/scripts/backfill-forge.ts` (backfill +
verify).

SECRETS (SPEC invariant 9): `forge_git_access_tokens` carries token
HASHES/prefixes only on BOTH engines (no widening); diagnostics and
backfill/verify output reference row keys and sha256 hashes only — the
one mirror path keyed on `token_hash` (the authenticate-path expiry /
last-used transitions) redacts its diagnostic refs. If any log line ever
shows a token hash or prefix, treat it as an incident, not drift.

REF LOCKING: D1 remains the SOLE lock authority in this phase. The
Postgres twin only receives resolved lock rows via read-back; the
held-lock protocol is deliberately NOT emulated in Postgres. The
read/write cutover step re-ports the protocol onto real
`SELECT ... FOR UPDATE` row locks and re-adds the partial uniques
(active-lease-per-work, held-lock-per-ref, token-hash, packfile digest,
mirror destination tuple) that were intentionally left off the twins.

Diagnostics: the drift metric is `khala_sync_forge_dual_write_failed`
(keys only). Treat a nonzero steady rate as drift — fix, then re-run the
backfill sweep.

Flags (Worker vars):

- `KHALA_SYNC_FORGE_DUAL_WRITE` — default **on** wherever
  `KHALA_SYNC_DB` exists; `off|0|false|disabled` disables the mirror.
- `KHALA_SYNC_FORGE_READS` — default `d1`. `compare` shadow-runs the
  canonical `listRefs` ref advertisement (the ref-set surface the §3.13
  acceptance keys on) against the Postgres twin, SERVES D1, and logs
  `khala_sync_forge_read_compare_mismatch` /
  `khala_sync_forge_read_compare_failed`. `postgres` serving is DEFERRED
  to the cutover follow-up (the forge read surface is protocol-wide):
  setting it today behaves as `compare` and logs
  `khala_sync_forge_postgres_reads_deferred` once, so a premature flip
  can never serve an unproven read path.

Flag-flip order — never skip a step, each step soaks before the next:

1. **Dual-write on** (default after KS-8.16 lands + `0021` applied via
   the migration runner). Watch `khala_sync_forge_dual_write_failed` in
   Worker logs; a nonzero steady rate blocks progression.
2. **Backfill**: from `packages/khala-sync-server/`,
   `KHALA_SYNC_DATABASE_URL=<direct-url> bun scripts/backfill-forge.ts`
   (wrangler-auth'd; rowid-cursor resumable via
   `.forge-backfill-state.json` — safe to interrupt/resume). Run it a
   SECOND time (`--restart`) as the catch-up sweep once dual-write has
   covered the whole window.
3. **Verify**: `bun scripts/backfill-forge.ts --verify` — exact row
   counts, per-state tallies, per-(tenant, repository) REF-SET digests,
   per-(tenant, queue) merge-queue LEDGER REPLAY digests, newest-50 row
   hashes. Post the output on the migration issue (it is secret-safe by
   construction). Exact or explain; no cutover on a red verify.
4. **Ground-truth cross-check**: for each live tenant repo, run
   `git ls-remote` against the Forge intake surface and diff the
   advertised (ref, tip) set against BOTH stores' `forge_git_refs`
   active rows — git itself is the §3.13 acceptance authority, the
   verify digests only prove D1 ≡ Postgres.
5. **Compare reads**: set `KHALA_SYNC_FORGE_READS=compare`; soak until
   the mismatch log is silent over a window that includes real push +
   mirror traffic.
6. **Read/write cutover LATER**: serving reads from Postgres, porting
   the ref-lock protocol onto `SELECT ... FOR UPDATE`, re-adding the
   uniques, moving write authority, and dropping the D1 tables is the
   follow-up #8358 on epic #8282 — never in the same change as this
   lane. Until then rollback is one flag flip back to `d1`.

Rollback at ANY step: set `KHALA_SYNC_FORGE_READS=d1` (reads) and/or
`KHALA_SYNC_FORGE_DUAL_WRITE=off` (writes). D1 authority is never
behind.

## Billing/Stripe/pay-ins domain cutover (KS-8.7, #8318)

The FIRST money domain in the KS-8 sequence: the 22 live
billing/credits/Stripe/pay-ins/buyer-payment tables (khala-sync migration
`0015_billing_pay_ins.sql`). Machinery:
`apps/openagents.com/workers/api/src/billing-store.ts` (fail-soft
READ-BACK mirror + routed balance read) and
`packages/khala-sync-server/scripts/backfill-billing.ts` (backfill +
money verify).

**MONEY DISCIPLINE (overrides the generic recipe where they differ):**

- D1 is the SOLE authority for this domain for the entire life of this
  lane. The Postgres side is a best-effort mirror; amounts and
  idempotency keys are COPIED from accepted D1 rows, never recomputed.
- Side-effectful evaluators (auto-top-up charging, sweeps, Stripe API
  calls) and gate/receipt balance reads ALWAYS read D1 — only the
  display billing summary opts into read routing
  (`billingRuntimeForEnv(env, { routeReads: true })`).
- **The production flip of `KHALA_SYNC_BILLING_READS` (compare →
  postgres) is an EPIC-GATED ops decision recorded on
  [#8282](https://github.com/OpenAgentsInc/openagents/issues/8282)** —
  it requires a green money `--verify` (below) posted as evidence and an
  explicit owner-visible decision entry. Never flip it as part of a
  routine deploy.

Flags (Worker vars; structural — absent means default):

- `KHALA_SYNC_BILLING_DUAL_WRITE` — default **on** wherever
  `KHALA_SYNC_DB` exists; `off|0|false|disabled` disables the mirror.
- `KHALA_SYNC_BILLING_READS` — default `d1`; `compare` reads both,
  serves D1, logs `khala_sync_billing_read_compare_mismatch` with the
  cent delta; `postgres` serves the routed balance read from Postgres
  with bounded retry (50/150ms) and D1 fallback. Only the display
  summary read routes in this lane.

Cutover order — never skip a step, each step soaks before the next:

1. **Dual-write on** (default after this lane lands + `0010` applied via
   the migration runner). Watch `khala_sync_billing_dual_write_failed` —
   that event IS the drift metric; a nonzero steady rate blocks
   progression.
2. **Backfill**: from `packages/khala-sync-server/`,
   `KHALA_SYNC_DATABASE_URL=<direct-url> bun scripts/backfill-billing.ts`
   (wrangler-auth'd; rowid-cursor resumable via
   `.billing-backfill-state.json`). Run a SECOND sweep with `--restart`
   once dual-write has covered the whole window — the second sweep also
   re-converges rows UPDATEd on D1 after the first sweep copied them
   (webhook statuses, checkout fulfillment, pay-in states, policies).
3. **Verify (the money acceptance)**:
   `bun scripts/backfill-billing.ts --verify` — exact row counts per
   table, the FULL per-user balance map (balance = SUM(amount_cents) to
   the cent, every account), per-(currency, source) cents, pay-in msat
   sums per (type, state) and legs per (direction, kind), the
   `stripe_webhook_events` event-id SET digest (identical dedupe key
   sets), buyer receipt/debit minor-unit sums, paid-plan intent
   cents/sats sums, and newest-50 row hashes. Post the output on #8318 /
   the epic. Exact or explain; money reads NEVER cut on a red verify.
4. **Compare reads**: set `KHALA_SYNC_BILLING_READS=compare`; soak until
   the mismatch log is silent over a representative window that includes
   a live Stripe checkout and an auto-top-up evaluation.
5. **Postgres reads — EPIC-GATED**: set
   `KHALA_SYNC_BILLING_READS=postgres` only per the #8282 decision entry
   (green verify + silent compare soak attached). This routes ONLY the
   display balance read; D1 remains the write authority, every evaluator
   input, and the fallback.
6. **Decommission LATER**: a separate follow-up issue tracks moving the
   remaining D1-only writers/readers, verifying the
   `billing_ledger_entries_next` artifact stays absent, stopping
   dual-write, snapshotting to R2, and dropping the D1 tables — never in
   the same change as a read cutover.

Rollback at ANY step: `KHALA_SYNC_BILLING_READS=d1` (reads) and/or
`KHALA_SYNC_BILLING_DUAL_WRITE=off` (writes). D1 authority is never
behind.

**Mirror coverage (what dual-writes today):** billing.ts credit/debit/
policy/notification ops (wired at billing-routes, operator routes,
index.ts cron/policy sites, omni-runs metering), the FULL Stripe webhook
ingest (`stripe_webhook_events` insert + status updates, checkout
fulfillment, customers, sessions, saved payment methods, auto-top-up),
Khala Code paid-plan intents (both rails), the buyer-payment ledger store
(all six create paths + site-checkout challenges), and annotated pay-in
plans through `runLedgerStatements` on the tip-ladder (forum + pylon
tips), tips-sweep + forwarding reconcile, USD-credit bridge, and MPP/
Lightning mints. **Still D1-only pending the decommission follow-up**
(converged by every backfill sweep until then):
`first_batch_payment_policies` (operator triage), codex-usage debits from
`OmniRunStore` constructors that do not pass `billingRuntime`, and the
low-volume `runLedgerStatements` consumers (labor-escrow, metering-hook,
batch-job-metering, inference-abuse-controls, serving-node-payout,
cloud-metering, product-promises, business-starter-credit). A final
`--restart` sweep + `--verify` immediately before any read cutover is
therefore MANDATORY, not optional.

## Business funnel domain cutover (KS-8.14, #8325)

The business funnel / orders / referrals domain: the 32 live tables from
khala-sync migration `0023_business_funnel.sql` (business signup /
fulfillment / pipeline / commitments / affiliates, funnel events, service
promises + fulfillment-loop receipts + escalation pages, checkout
kickoffs, starter credits, software orders + triage + fulfillment
artifacts + GitHub write-authority receipts, the referral spine's
consumption side, workflow events, viral funnel, QA-swarm engagements,
promise transition receipts, buy-mode, customer-one cohort). Machinery:
`apps/openagents.com/workers/api/src/business-domain-store.ts` (the
mirroring D1Database, `businessDomainDatabaseForEnv`) and
`packages/khala-sync-server/scripts/backfill-business.ts`.

**DOMAIN DISCIPLINE (overrides the generic recipe where they differ):**

- D1 is the SOLE authority for this domain for the entire life of this
  lane. Referral attribution uniqueness keys feed payouts (KS-8.8): the
  consume-once decision (INSERT OR IGNORE on the attribution PKs /
  UNIQUEs) is made ONCE, on D1, and the mirror only copies accepted rows.
- The fulfillment-loop escalation pager and the starter-credit
  window-cap trigger evaluate against exactly ONE store (D1). Nothing in
  Postgres feeds an evaluator — dual-write can never double-page.
- `promise_transition_receipts` backs the PUBLIC product-promises
  registry: it must stay continuously servable, so its verify acceptance
  is FULL row-hash set equality, not just counts.
- **The production flip of `KHALA_SYNC_BUSINESS_READS` is an EPIC-GATED
  ops decision recorded on
  [#8282](https://github.com/OpenAgentsInc/openagents/issues/8282)** —
  and in this lane `postgres` intentionally behaves as `compare` (logs
  `khala_sync_business_postgres_reads_deferred`); actual Postgres read
  serving lands with the read-cutover follow-up.

Flags (Worker vars; structural — absent means default):

- `KHALA_SYNC_BUSINESS_DUAL_WRITE` — default **on** wherever
  `KHALA_SYNC_DB` exists; `off|0|false|disabled` disables the mirror.
- `KHALA_SYNC_BUSINESS_READS` — default `d1`; `compare` shadow-runs
  scoped-table SELECTs against Postgres, SERVES D1, and logs
  `khala_sync_business_read_compare_mismatch`; `postgres` defers to
  compare (above).

Cutover order — never skip a step, each step soaks before the next:

1. **Dual-write on** (default after this lane lands + `0024` applied via
   the migration runner). Watch `khala_sync_business_dual_write_failed`
   (the drift metric) AND `khala_sync_business_write_unclassified` (a
   scoped write statement the classifier does not recognize — new writer
   code must either classify or be added to the remainder list); nonzero
   steady rates block progression.
2. **Backfill**: from `packages/khala-sync-server/`,
   `KHALA_SYNC_DATABASE_URL=<direct-url> bun scripts/backfill-business.ts`
   (resumable; state in `.business-backfill-state.json`).
3. **Catch-up sweep**: rerun with `--restart` after dual-write has been
   on across the whole window — this also re-converges rows UPDATEd on
   D1 (pipeline stages, fulfillment status, buy-mode counters,
   attribution policy_state) after the first sweep copied them.
4. **Verify**: `bun scripts/backfill-business.ts --verify` — exact
   counts, ATTRIBUTION SET DIGESTS (the payout-feeding tuples across the
   five attribution tables + workflow-event/QA idempotency sets),
   PROMISE-RECEIPT full-row hash equality, funnel counts per cohort,
   money sums, newest-N row hashes. Exact or explain; attribution reads
   NEVER cut on a red verify.
5. **Compare reads** (`KHALA_SYNC_BUSINESS_READS=compare`), soak on the
   funnel dashboard + capture routes, then the read-cutover follow-up
   moves reads WITH their re-derived indexes.
6. **Remainder + decommission follow-ups**: wire the still-D1-only
   writer boundaries (see the §3.11 status list in
   [`MIGRATION_PLAN.md`](./MIGRATION_PLAN.md) — customer-orders /
   triage / adjutant / github-writeback `software_orders`+`order_*`
   paths, stripe-billing-fed checkout kickoffs + engagement-feed funnel
   writes, site-referral-policy workflow events, onboarding
   consumption), then drop the D1 tables. A final `--restart` sweep +
   `--verify` immediately before any read cutover is MANDATORY, not
   optional (the unwired boundaries are backfill-converged until then).

Rollback at ANY step: set `KHALA_SYNC_BUSINESS_READS=d1` (reads) and/or
`KHALA_SYNC_BUSINESS_DUAL_WRITE=off` (writes). D1 authority is never
behind.
## Supervision long-tail cutover (KS-8.17, #8328)

The KS-8.17 domain migration: 29 D1 tables — `adjutant_*` (10), `omni_*`
(9), `autopilot_*` (6), `relay_health_*` (2), `backend_incident_events`,
`hygiene_debt_receipts` — → same-named Postgres twins (khala-sync migration
`0024_supervision_longtail.sql`). Machinery:
`apps/openagents.com/workers/api/src/supervision-longtail-domain-store.ts`
(the row-level converge store + fail-soft read-back mirror + the four
`make*ForEnv` store-factory drop-ins) and
`packages/khala-sync-server/scripts/backfill-supervision-longtail.ts`
(backfill + verify).

WRITE-DEAD AUDIT (do this per table before trusting a twin): each family
had a last-write freshness check. `autopilot_token_usage` is NOT write-dead
(one live writer, `omni-runs.ts:tokenUsageInsert`) — it dual-writes.
`omni_idempotency_keys` has no writer today — the twin is a verified copy
only (backfill once + key-set-equality verify; no live mirror needed until a
writer returns).

SECRETS (SPEC invariant 9): every column is a public-safe ref/path/digest/
count or JSON of the same. Custody columns (transcript/metadata/entries/
result/receipt JSON — declared in the registry `custodyColumns`) are
mirrored as column values but NEVER printed in a diagnostic or in
backfill/verify output: row KEYS and sha256 hashes only. Any log line
showing a custody JSON value is an incident, not drift.

LIVE WIRING IN THIS LANE (the acceptance-critical seams): the three re-homed
crons + the funded-hygiene store, wired as store-factory drop-ins:
`RelayHealth.probeTick` (`makeRelayHealthStoreForEnv` — probes/transitions
mirror on insert, the retention prunes converge onto the twin),
`AutopilotContinuationPolicy.sweep` (`makeAutopilotContinuationStoreForEnv`),
`AutopilotScheduledLaunches.dispatchDue` (`makeAutopilotWorkStoreForEnv` —
every work-order write mirrors by `work_order_ref`, closeout receipts by
`closeout_ref`), and `makeHygieneDebtReceiptStoreForEnv`. The scattered
`adjutant_*` / `omni_*` writers, the Effect onboarding store,
`autopilot_token_usage`, and `backend_incident_events` have their twins +
backfill + verify here but plug their per-site live mirror into
`makeSupervisionLongtailMirrorForEnv` in the decommission follow-up #8361.

Flags (Worker vars):

- `KHALA_SYNC_SUPERVISION_DUAL_WRITE` — default **on** wherever
  `KHALA_SYNC_DB` exists; `off|0|false|disabled|no` disables the mirror.
- `KHALA_SYNC_SUPERVISION_READS` — default `d1`. `compare`/`postgres`
  serving is DEFERRED to the read-cutover follow-up (reads stay on D1 in
  this lane); a premature flip can never serve an unproven read path.

Flag-flip order — never skip a step, each soaks before the next:

1. **Dual-write on** (default after KS-8.17 lands + `0024` applied via the
   migration runner). Watch `khala_sync_supervision_dual_write_failed`; a
   nonzero steady rate blocks progression.
2. **Backfill**: from `packages/khala-sync-server/`,
   `KHALA_SYNC_DATABASE_URL=<direct-url> bun scripts/backfill-supervision-longtail.ts`
   (wrangler-auth'd; rowid-cursor resumable via
   `.supervision-longtail-backfill-state.json`). Run a SECOND time
   (`--restart`) as the catch-up sweep once dual-write has covered the whole
   window.
3. **Verify**: `bun scripts/backfill-supervision-longtail.ts --verify` —
   exact row counts, per-state/sum tallies, **idempotency-key-set equality**
   (`omni_idempotency_keys`), **public proof-bundle digests**
   (`omni_public_proof_bundles`, the §3.14 shadow-compared projection
   surface), newest-50 row hashes. Post the output on the migration issue
   (secret-safe by construction). Exact or explain; no cutover on a red
   verify.
4. **Shadow-compare the public proof-bundle endpoints**: diff the servable
   `omni_public_proof_bundles` projection against BOTH stores until silent.
5. **Read/write cutover LATER** (follow-up #8361): wire the remaining
   scattered writers' live mirror, serve reads from Postgres, re-add the
   uniques left off the twins, move write authority, and drop the D1 tables.
   Until then rollback is one flag flip back.

Rollback at ANY step: `KHALA_SYNC_SUPERVISION_DUAL_WRITE=off`. D1 authority
is never behind.

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
