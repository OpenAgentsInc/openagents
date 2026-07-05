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

### 2026-07-05 read-cutover evidence (#8335)

- **Retention decision (step 2 pre-step):** `artanis_health_snapshots` /
  `artanis_runtime_snapshots` are 125 rows each in production — porting
  as-is is cheap; no bounding needed before the sweep.
- **Backfill x2 + verify:** sweep 1 ported the full history; sweep 2
  (`--restart`) inserted 0 new rows across all twenty tables (dual-write
  had already fully caught up). `--verify --verify-newest 50`: **19/20
  tables exact** (rows, per-state tallies, newest-50 hashes). One table,
  `artanis_responder_ticks`, came back non-exact — see below. Full output
  posted on #8335.
- **Explain the one non-exact table:** `artanis_responder_ticks` matched
  on row COUNT (7940=7940) but had 20 rows with a stale `scan_state` (or
  `compose_state`) in Postgres and 1 newest-50 hash mismatch. Root cause:
  `mirrorArtanisRows` does a full-row D1-read-then-Postgres-upsert with no
  ordering guard, and this table is the one place two INDEPENDENT ticks
  (`ArtanisResponder.scan`, `ArtanisResponder.compose`) both write the
  SAME row (keyed by `scheduled_at`) — when their D1-write + Postgres-
  round-trip timings interleave, whichever mirror's Postgres upsert lands
  LAST wins for the WHOLE row, even if its D1 snapshot was read earlier
  and is missing the other tick's column update. `created_at`/`updated_at`
  can't break the tie because both ticks stamp the identical scheduled
  `nowIso`. Filed as
  [#8409](https://github.com/OpenAgentsInc/openagents/issues/8409) — real
  bug in the landed KS-8.6 mirror, not rubber-stamped past. It does NOT
  block the flip below (`responder_tick` is not one of the eight
  `ArtanisPersistenceRecordKind`s routed through `artanisRead`), but it
  DOES block ever safely reading this table from Postgres and blocks
  KS-8.19 D1 retirement for it.

### 2026-07-05 #8409 fix landed (code only — deploy + corrective sweep still pending)

- **Fix:** `mirrorArtanisRows`/`PostgresArtanisDomainStore.upsertRows`
  (`apps/openagents.com/workers/api/src/artanis-domain-store.ts`) now take
  an optional `updateColumns` scope. The INSERT side stays full-row
  (self-heal, unchanged), but the `ON CONFLICT ... DO UPDATE SET` side only
  overwrites the caller's OWN columns when a scope is passed — so a stale
  snapshot from one writer can never clobber another writer's concurrent
  column update on the same key. `recordArtanisResponderScanTick` /
  `recordArtanisResponderComposeTick` (`artanis-responder-ticks.ts`) now
  pass `SCAN_TICK_UPDATE_COLUMNS` / `COMPOSE_TICK_UPDATE_COLUMNS`
  respectively. The SAME race shape (two independent every-minute cron
  ticks writing disjoint columns of one singleton row) also existed for
  `artanis_responder_state` (scan owns `scan_cursor_iso` in
  `artanis-forum-responder.ts`; compose owns
  `responses_today`/`responses_day` in `artanis-reply-composer.ts`) —
  fixed the same way, though no production drift had been observed there
  yet. An audit of every remaining `mirrorArtanisRows` call site found no
  other genuine instance (every other table is either INSERT-only with a
  freshly-minted key per call, or has exactly one writer module for its
  `UPDATE` path).
- **Regression coverage:** `artanis-domain-repository.contract.test.ts`
  (real local Postgres + real D1/SQLite) reproduces the exact interleaving
  — a stale full-row D1 snapshot landing in Postgres AFTER a fresher
  concurrent writer's own converged mirror — for both
  `artanis_responder_ticks` and `artanis_responder_state`, and asserts
  both writers' columns survive. Verified the tests actually catch the
  regression by reverting the column-scoping fix locally and confirming
  both new tests fail with the exact reported symptom
  (`compose_state`/`responses_today` reverted to the stale pre-write
  value), then restoring the fix and confirming green again.
- **Fresh production `--verify` (2026-07-05, same day, hours after the
  original #8335 evidence, code fix NOT yet deployed):** 19/20 tables
  still exact; `artanis_responder_ticks` is still the one non-exact table,
  and the drift has GROWN since the original report — `d1=8026
  postgres=8025` rows, `scan_state` tallies `d1 {"pending":707,"ran":7319}`
  vs `postgres {"pending":728,"ran":7297}` (21/22-row skew, up from the
  original 20/7940) — strong live confirmation that the race is real,
  ongoing, and accumulates over time until the fix ships. This run used
  the existing sanctioned read-only `--verify` path
  (`KHALA_SYNC_APP_USER` role, the same role the live mirror itself uses)
  — no production data was written.
- **What is still outstanding (deliberately NOT done in this pass):**
  deploying this fix through the sanctioned `deploy:safe` gate (a larger,
  separate, explicitly-gated action out of scope for a narrow mirror-logic
  fix), and a one-time corrective full-row re-converge of the already-
  stale `artanis_responder_ticks` rows once the fix is live (safe to do at
  that point — D1 stays authoritative and this table is not
  read-routed — but pointless before the fix ships, since the still-live
  buggy code would just re-drift it). Whoever runs the next
  `deploy:safe` + backfill sweep for this domain should re-run
  `bun scripts/backfill-artanis.ts --verify --table artanis_responder_ticks`
  afterward and expect an exact match once both the deploy and the
  corrective sweep have landed.
- **What is actually flag-routed today:** only two functions route through
  `artanisRead` — `readArtanisPersistedRecord` (used by
  `ArtanisScheduledRunner.runTick`'s unconditional every-tick idempotency
  check on `loop_record`) and `readLatestArtanisPersistedRows` (used by
  the operator console `GET /api/operator/artanis/console` and the public
  `GET /api/public/artanis/report`). The other five cron ticks
  (`ArtanisResponder.scan/.compose`, `ArtanisAdmin.tick`,
  `ArtanisAdmin.closeoutVerifier`, `ArtanisFleet.tick`) only mirror writes
  today; their own decision-making reads are bare D1 SQL with no Postgres
  reader wired, so the flag is a no-op for them either way.
- **Compare-mode soak:** `KHALA_SYNC_ARTANIS_READS=compare` shipped to prod
  + staging in commit `07ada9d32b` (Worker version
  `473b6c53-8a65-40d2-b238-2e1d5c21c449`). Soaked live via `wrangler tail`
  filtered on `khala_sync_artanis` plus repeated `GET
  /api/public/artanis/report` calls (the operator-console path needs a
  real logged-in WorkOS session — a bearer-token probe correctly gets 401
  unauthenticated, so that path was not separately exercised this pass).
  See #8335 for the exact observed window and mismatch count.
- **Remaining D1-direct read paths** (analytics joins in
  `artanis-tick-streak.ts` / `artanis-distillation-dataset-receipt.ts`,
  dashboard/console aggregations, spend/grant aggregation reads in
  `artanis-spend.ts`, responder scan/composer joins in
  `artanis-responder-ticks.ts` / `artanis-responder-provenance.ts`, the
  labor receipt ordered list in `artanis-labor-receipt-store.ts`) are NOT
  yet flag-routed. Moving them is real, separate implementation work (new
  Postgres-routed queries + re-derived indexes + their own compare
  evidence per surface, and for `artanis_responder_ticks` specifically,
  #8409 must land first) — not a config flip, and not done in this pass.

### 2026-07-05 KS-8.6 follow-up — #8409 fix deployed, fresh clobber confirmed AFTER deploy (#8335)

**Do not flip `KHALA_SYNC_ARTANIS_READS=postgres` on this evidence. Stop-and-report
condition per the money/business-adjacent-data guardrail.**

- **Fetched/rebased:** confirmed `06ee7de4c7` (#8409 fix) is an ancestor of
  `origin/main`; fast-forwarded this worktree to `origin/main` (`7a1e0b8fc0`
  at kickoff). `apps/openagents.com/workers/api` test suite: 76 files / 736
  tests pass (incl. the #8409 regression coverage). `typecheck` clean.
  `check:architecture` zero-debt: clean.
- **Fresh `--verify` (before any redeploy this pass):** 19/20 tables still
  exact. `artanis_responder_ticks`: `d1=8125 postgres=8124` (23 stale rows,
  up from 20→21→23 across the three checks today), plus **one row entirely
  missing from Postgres** (`scheduled_at=2026-07-05T07:06:24.000Z`, fully
  resolved `ran`/`ran` in D1, absent from Postgres — a distinct fail-soft
  mirror-write-loss finding, separate from the column-clobber pattern).
- **Critical finding:** diffing the full 8000+-row table between D1 and
  Postgres and bucketing by `scheduled_at`, one of the 23 stale rows is
  **`2026-07-05T09:13:24.000Z`** — created (per D1 `created_at`) **27
  minutes AFTER** the `openagents-autopilot` Worker deploy at
  `2026-07-05T08:46:25Z` (`wrangler deployments list`), which is itself
  **33 minutes after** the #8409 fix commit landed on `main`
  (`06ee7de4c7`, `2026-07-05T08:13:27Z`) — i.e. chronologically, that
  deploy *should* have shipped the fix. D1 shows `scan_state=ran
  compose_state=ran` (fully resolved); Postgres shows `scan_state=pending
  compose_state=ran` (scan stuck) — the EXACT #8409 symptom, recurring
  after the fix was supposedly live.
- **Because deploys in this repo are ad hoc (`deploy:safe` run from
  whichever worktree an agent happens to be in, not CI-gated on every
  main push), a deploy's wall-clock timestamp does not guarantee it
  shipped a worktree at or after a given commit** — an agent could deploy
  from a worktree that branched before `06ee7de4c7`. To remove that
  variable, this pass fast-forwarded a clean worktree to `origin/main`
  (`7a1e0b8fc0`, confirmed `06ee7de4c7` many commits back), ran the full
  sanctioned `deploy:safe` gate (`check:deploy-from-main` OK at
  `7a1e0b8fc0`; `check:deploy` — the full typecheck/test/architecture/
  contract-drift/public-projection suite — green; staging deploy +
  `predeploy:parallel-dispatch-smoke` — 5/5 dispatch OK; prod migrations
  — 0 pending), and completed the final production
  `wrangler deploy --containers-rollout=none` manually (the very last
  step of the chained script failed on a missing
  `KHALA_SYNC_DATABASE_URL` env var in the shell that invoked it, NOT a
  code or gate problem — the two preceding gates it needed had already
  passed; re-ran that one gate with the var set, then ran the final
  `wrangler deploy` directly). Production Worker Version ID
  `17543300-c80f-450f-a84a-826be0b06358`, live and smoke-tested
  (`GET https://openagents.com/` HTTP 200,
  `GET /api/public/artanis/report` HTTP 200) at **2026-07-05T10:00:48Z**.
  This is now a GUARANTEED-fresh deploy of `main` HEAD, including
  `06ee7de4c7`, with no worktree-staleness ambiguity.
- **Post-guaranteed-deploy observation:** watched for **~24 minutes**
  (`2026-07-05T10:00:48Z` → `10:24:57Z`). Full-table diff (not a sample) of
  `artanis_responder_ticks` between D1 and Postgres: **23 stale rows total,
  UNCHANGED from before the guaranteed deploy — zero of them have
  `scheduled_at` after `2026-07-05T10:00:48Z`.** The newest mismatch overall
  remains the pre-deploy `09:13:24Z` row; the 1 row missing from Postgres
  (`07:06:24Z`) also remains unchanged (still absent, not self-healed). This
  is an encouraging signal that the guaranteed-fresh deploy stopped new
  occurrences, but treat it as suggestive rather than conclusive per the
  sample-size caveat below.
- **Honest caveat on sample size:** across all 23 stale rows
  (`2026-07-04T21:27` → `2026-07-05T09:13`, an 11.76h dual-write window)
  the historical clobber rate is ~2/hour average (median gap ~22 min, min
  gap 1 min, max gap 115 min) — so a short post-deploy window silently
  passing is only weak-to-moderate evidence the fix actually resolves the
  live mechanism; treat it as suggestive, not conclusive, without a window
  meaningfully longer than the median gap.
- **Compare-mode soak status (the ACTUALLY flag-routed record kinds —
  `approval_gate`, `forum_publication_intent`, `health_snapshot`,
  `loop_record`, `loop_tick`, `nexus_pylon_adapter_dispatch`,
  `runtime_snapshot`, `work_routing_proposal`; `artanis_responder_ticks`
  is NOT one of them):** watched via `wrangler tail` for **~35 minutes**
  (`2026-07-05T09:50:32Z` → `10:25:23Z`), confirmed the every-minute cron
  kept firing throughout (fresh `artanis_responder_ticks` rows each
  minute) and repeated `GET /api/public/artanis/report` calls all HTTP
  200. The `--search khala_sync_artanis` flag did not actually restrict
  the stream server-side (see the incidental finding below), so this was
  effectively a genuinely UNFILTERED production tail — **zero** mentions
  of the string `artanis` anywhere in over 1200 lines of real traffic,
  meaning zero `khala_sync_artanis_read_compare_mismatch` /
  `_dual_write_failed` / `_postgres_read_failed` diagnostics fired for the
  eight actually-routed record kinds during this window.
- **Decision:** did **NOT** flip `KHALA_SYNC_ARTANIS_READS` to `postgres`.
  Even though `artanis_responder_ticks` is not itself read-routed, a
  fresh, unresolved, in-production clobber in a sibling table mirrored by
  the SAME `mirrorArtanisRows`/`upsertRows` machinery that the flag-routed
  tables also depend on is a live, unexplained data-integrity signal in
  this domain's dual-write path — per the explicit guardrail for
  money/business-adjacent responder data, this is a stop-and-report
  condition, not a paper-over-and-proceed one.
- **Recommendation:** reopen #8409 (or file a fresh linked issue) with
  this evidence. Root cause is NOT yet conclusively re-identified — the
  cross-writer full-row-clobber mechanism #8409 fixed is real and its
  regression tests are green, but the production symptom persisted past a
  guaranteed-fresh deploy of that fix, so either (a) a distinct mechanism
  produces the identical symptom (candidates worth investigating: a
  silently-swallowed `khala_sync_artanis_dual_write_failed` on the scan
  tick's OWN mirror call. `scan_state`/`compose_state` both default to
  `'pending'` in the table schema, so if compose runs first (self-heal
  INSERT captures `scan_state='pending'`, the schema default, since scan
  hasn't run yet) and scan's LATER scoped mirror UPDATE silently fails
  (transient Hyperdrive/Postgres connect/timeout — the SQL client uses a
  bare 10s `connect_timeout` with NO retry in `mirrorArtanisRows`, which
  is deliberately fail-soft), Postgres is stuck at `scan_state='pending'`
  forever — nothing else ever touches that column again for that
  `scheduled_at`. This produces the EXACT observed symptom with no
  interleaving race at all, and #8409's column-scoping fix does nothing
  to prevent it. A weaker candidate: some D1-consistency edge case on the
  mirror's read-back — no `withSession`/bookmark is used in
  `artanis-domain-store.ts`, unlike `business-domain-store.ts` (though no
  `read_replication` config was found on the `OPENAGENTS_DB` D1 binding,
  which weakens this candidate)), or (b)
  the fix has a residual bug not covered by the current regression
  fixtures. Querying persisted Worker logs (`observability.logs.persist:
  true` is already on) for `khala_sync_artanis_dual_write_failed` around
  `2026-07-05T09:13:24Z` would directly test hypothesis (a) and is the
  concrete next step.
- **Broader note:** the fail-soft, no-retry `mirror*Rows` pattern is
  shared scaffolding across essentially every Khala Sync domain store
  (`business-`, `treasury-`, `forge-`, `training-`, `identity-auth-`,
  `sites-content-`, `gym-evals-`, `crm-email-`, `supervision-longtail-`,
  and others), not just Artanis. Single-writer-per-key tables self-heal
  on their NEXT write (the next full-row snapshot recaptures current D1
  truth), so a dropped write is invisible there; only a natural key with
  MULTIPLE independent writers (like `artanis_responder_ticks`/`_state`)
  turns one dropped write into a permanently-stuck stale column. Worth an
  audit of other multi-writer natural keys across domains if hypothesis
  (a) is confirmed — out of scope for this pass.
- **Incidental unrelated finding (out of scope, flagged for awareness only):**
  `wrangler tail --search khala_sync_artanis` did not actually restrict the
  stream server-side (it showed general production traffic throughout, not
  just artanis-tagged lines) — a useful accident, since it means the "zero
  artanis mentions" result above is from a genuinely unfiltered tail, not a
  possibly-broken filter hiding real events. That same unfiltered tail
  repeatedly surfaced two UNRELATED live errors this session: a Sites
  publish path `D1_ERROR: FOREIGN KEY constraint failed`
  (`siteId=site_project_otec`, `versionId=site_version_otec_20260605_revision_3`)
  and a recurring `TipsBufferBackingViolation:
  tips_buffer_backing_violated: agent balances 263 sat exceed buffer 15 sat`
  (`checkTipsBufferBackingInvariant`). Both are money/business-adjacent and
  real, but entirely outside this issue's Artanis/KS-8.6 scope — not
  investigated or fixed here; flagging so they don't go unnoticed.
- **Not done this pass (deliberately, given the stop condition):** the
  corrective full-row re-converge sweep for the already-stale rows (would
  be premature while the mechanism is unconfirmed-fixed — it could
  immediately re-drift), and the remaining D1-direct read-path migration
  work (analytics joins, dashboard/console aggregations, spend/grant
  aggregation, responder scan/composer joins, labor receipt ordered
  list) — all gated on this domain's dual-write mirror being trustworthy
  first.

### 2026-07-05 #8409 follow-up — root cause confirmed and fixed (retry, code only)

Reopened #8409 with a specific candidate hypothesis: a silently-swallowed,
fail-soft, NO-RETRY mirror write on one tick's OWN column update — a
DIFFERENT mechanism from the cross-writer clobber race the original
column-scoping fix addressed.

- **Confirmed directly in code**, not assumed: `mirrorArtanisRows`
  (`apps/openagents.com/workers/api/src/artanis-domain-store.ts`) attempted
  its D1 read-back + Postgres upsert exactly ONCE. Any failure — including
  a transient one — was caught, logged as
  `khala_sync_artanis_dual_write_failed`, and discarded; the write was
  never retried. The Postgres client factory
  (`khala-sync-push-routes.ts::defaultMakeKhalaSyncSqlClient`) uses a bare
  `connect_timeout: 10` with no client-level reconnect/retry either. For a
  single-writer table this is harmless (the NEXT write re-converges the
  full row from current D1 truth). For `artanis_responder_ticks` /
  `artanis_responder_state` — the only tables with two independent
  every-minute writers on disjoint columns of the SAME natural key — a
  dropped write is a PERMANENT stale column: nothing else ever touches
  that `scheduled_at` row's `scan_*` (or `compose_*`) columns again. This
  is a distinct, real bug from the #8409 clobber race, and it produces the
  IDENTICAL symptom (`scan_state` stuck at the schema default `'pending'`
  in Postgres while D1 shows `'ran'`) — exactly what the reopening
  evidence observed recurring after the column-scoping fix was
  guaranteed-deployed.
- **Fix:** `mirrorArtanisRows` now retries the whole D1-read-back +
  Postgres-upsert up to twice more with short backoff (`[100, 400]` ms —
  `MIRROR_WRITE_RETRY_DELAYS_MS`) before giving up, mirroring the existing
  bounded-retry precedent already used for `artanisRead`'s `postgres` mode
  (`READ_RETRY_DELAYS_MS = [50, 150]`). Each retry attempt logs a NEW,
  distinct diagnostic — `khala_sync_artanis_dual_write_retry` — so a
  recovered transient failure is observable without being confused with a
  permanent one; the final-exhaustion event is still
  `khala_sync_artanis_dual_write_failed` (unchanged event name, so existing
  alerting/dashboards keep working). Registry/argument validation errors
  (a caller passing an unregistered column — a programming error, not a
  transient failure) are never retried. The fail-soft invariant is
  unchanged: `mirrorArtanisRows` still NEVER throws, on any path.
- **Regression coverage:** `artanis-domain-store.test.ts` (unit level, fake
  Postgres) proves (a) a persistently-failing mirror write retries twice
  with the exact configured backoff before logging exactly one
  `_failed` diagnostic (preceded by two `_retry` diagnostics), and (b) a
  transiently-failing write (fails once, succeeds on the 2nd attempt)
  converges correctly with only a `_retry` diagnostic — no `_failed`, no
  lost data. `artanis-domain-repository.contract.test.ts` (real local
  Postgres + real D1/SQLite) adds a same-shape proof against the actual
  `artanis_responder_ticks` cross-engine path: a scan tick's mirror call
  fails once (transient), retries, and the row converges with
  `scan_state='ran'` — proving the exact production defect (a
  single-writer's own dropped mirror write) no longer permanently loses
  that writer's column. Verified test sensitivity directly: temporarily
  set `MIRROR_WRITE_RETRY_DELAYS_MS = []` (no retries) and confirmed all
  three new tests fail with the exact previously-reported shape (no retry
  attempted, write permanently lost), then restored the fix and confirmed
  green again.
- **Fresh production `--verify` baseline (2026-07-05, code fix NOT yet
  deployed):** `bun scripts/backfill-artanis.ts --verify --table
  artanis_responder_ticks` (read-only, `KHALA_SYNC_APP_USER` role, no data
  written) — `rows: d1=8179 postgres=8178` (1 row still entirely missing
  from Postgres, unchanged from the prior pass), `scan_state`/`compose_state`
  tallies `d1 {"pending":707,"ran":7472}` vs `postgres
  {"pending":730,"ran":7448}` (23-row skew — essentially unchanged from the
  23 found in the prior #8335 pass; the drift has plateaued rather than
  grown further in the intervening window, consistent with a low, steady
  transient-failure rate rather than an accelerating one). Newest-50 row
  hashes still all match.
- **What is still outstanding:** this fix is committed to `main` but the
  running production Worker has not been redeployed as part of this pass
  (a separate `deploy:safe` action). Once deployed, the honest closure bar
  is: (1) watch for `khala_sync_artanis_dual_write_retry` firing in
  persisted Worker logs (proof the retry path is live and recovering real
  transient failures) with `khala_sync_artanis_dual_write_failed` staying
  at or near zero for this table across a window spanning several of the
  historical median gaps (~22 min), THEN (2) run the corrective full-row
  re-converge sweep for the 24 already-stale/missing rows (safe once the
  live mirror stops re-drifting them — D1 stays authoritative and this
  table is not read-routed), THEN (3) a final `--verify` should show an
  exact match. Retry narrows the loss window for short blips; it does NOT
  eliminate loss during a Postgres/Hyperdrive outage longer than ~500ms —
  a periodic reconciliation sweep (re-running the existing backfill
  script's converge logic on a schedule) remains a reasonable further
  defense-in-depth follow-up for this specific multi-writer-key shape, not
  built in this pass.
- **Broader note (unchanged from the prior pass):** the same fail-soft,
  now-retried `mirrorArtanisRows` machinery underlies every `artanis_*`
  mirror call site; this fix benefits ALL of them (any transient blip
  anywhere in the domain now gets two extra chances), not just the
  responder-tick tables. The single-writer tables were never observably
  affected by the original defect (their next write self-heals), so no
  behavior change is expected for them beyond fewer redundant
  `dual_write_failed` diagnostics on transient blips.

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

**#8336 status (2026-07-05):** batch-job reads are moot — the whole
`inference_batch_jobs` feature (D1 + Postgres tables, code) was already
retired (28 tables remain). Real evidence gathered against production:
two backfill sweeps + a GREEN `--verify` (exact counts, hashes, and
tally=SUM(events) invariants on all 28 tables) plus a live
`wrangler tail` sample (tens of thousands of lines, zero
`dual_write_failed`) — see the #8336 issue comment for the verbatim
output. `KHALA_SYNC_ENTITLEMENTS_READS` was deliberately left at `d1`:
this session had no durable production log/metrics surface to establish
a genuine multi-hour representative-window soak, so neither the
`compare` nor `postgres` flip was attempted rather than flip on
incomplete evidence. Per the current owner-directed policy (see
MIGRATION_PLAN.md's KS-8.1/KS-8.2 status notes), the D1-table-drop half
of step 6 is deferred to the epic-wide KS-8.19 retirement sweep
(#8330), not done per-domain. Routing the non-gate reads to Postgres was
inventoried (see MIGRATION_PLAN.md §3.6 KS-8.9 decommission follow-up
status) but not implemented — the call sites need env/flag threading
through several existing Effect-based modules and stay a follow-up.

**#8336 status, part 2 (2026-07-05): bounded non-gate read allowlist,
following the KS-8.14 business-domain precedent (#8360).** A per-call-site
review found most of the inventoried "non-gate reads" were actually
enforcement/idempotency hazards (agent-search rate-limit/dedupe reads,
agent-rate-limit-recovery redemption-validity reads, read-your-own-write
grant read-backs) — those stay D1-only PERMANENTLY, same discipline as
`KHALA_SYNC_ENTITLEMENTS_READS` itself. The genuinely safe subset — three
public-projection reads that decide nothing — is now bounded-served for
real, through a brand-new, FULLY INDEPENDENT flag:

- `KHALA_SYNC_ENTITLEMENTS_NON_GATE_READS` (d1|compare|postgres, default
  d1) governs ONLY `countActiveOrangeChecks`, `readActiveOrangeCheckByActorRef`
  (`orange-check-entitlements.ts`), and `readPublicPrivacyReceipt`
  (`inference-privacy-receipt-routes.ts`). It is independent of
  `KHALA_SYNC_ENTITLEMENTS_READS`, which stays at its default `d1` — this
  pass does NOT flip the enforcement-gate flag.
- `postgres` mode REALLY serves these three from Postgres (single-attempt
  D1 fallback + diagnostic on error) — safe here, unlike the gate reads,
  because none of the three decides an allow/deny/consume outcome.
- Production evidence (2026-07-05): fresh backfill sweep + `--restart`
  catch-up + `--verify` — VERIFY OK, all 28 tables exact
  (`orange_check_entitlements` d1=2/postgres=2). Contract-suite coverage
  proves real D1-vs-Postgres answer parity for the orange-check reads and
  BOTH privacy-receipt kinds (entitlement + confidential-compute).
- Flag-flip sequence (mirrors the business-funnel precedent): deploy with
  the flag unset (default `d1`) → flip `compare` → brief soak (watch
  `khala_sync_entitlements_non_gate_read_compare_mismatch`) → flip
  `postgres` → brief soak (watch
  `khala_sync_entitlements_non_gate_postgres_read_fallback`) → live smokes
  on `/api/forum/launch-status` and an agent profile page. Recorded on
  epic #8282 with the exact commits/Worker versions.
- Rollback at any point: `KHALA_SYNC_ENTITLEMENTS_NON_GATE_READS=d1`. This
  can NEVER affect enforcement — the flag only touches display reads.

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
requests) LANDED in the remainder lane (#8338) and ride the same sequence
below — see "Remainder tables (KS-8.10 remainder, #8338)" at the end of
this section and MIGRATION_PLAN §3.7. The forum MONEY tables belong to
KS-8.8 and are not part of this procedure.

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

### Remainder tables (KS-8.10 remainder, #8338)

The eleven active remainder forum tables — `forum_private_message_threads`,
`forum_private_messages`, `forum_acl_grants`, `forum_score_snapshots`,
`forum_notification_reads`, and the work-request lifecycle family (6) —
ride this SAME sequence and the SAME flags (`KHALA_SYNC_FORUM_DUAL_WRITE` /
`KHALA_SYNC_FORUM_READS`). The historical trust pair
`forum_trust_edges` / `forum_actor_forum_trust` was dropped in #8379
(`apps/openagents.com` D1 migration `0300_drop_forum_trust_tables.sql` and
Khala Sync migration `0030_drop_forum_trust_remainder.sql`). Their mirror
(`apps/openagents.com/workers/api/src/forum/forum-remainder-store.ts`,
`wrapForumRemainderMirroring`) is composed around
`forumContentDatabaseForEnv`, so dual-write turning on for the content lane
turns it on for the remainder tables too — no separate flag. Postgres
schema is `0027_forum_remainder.sql` (apply with the same migration
runner). Backfill + verify is the sibling CLI
`packages/khala-sync-server/scripts/backfill-forum-remainder.ts` (same
`--verify` / `--restart` / rowid-cursor semantics; state file
`.forum-remainder-backfill-state.json`). Its `--verify` adds the
domain-specific gate beyond counts/tallies/hashes:

- **Work-request set-membership referential checks** — within-store orphan
  counts (every lifecycle child's `work_request_id` and acceptance/result
  `offer_id` resolves to a parent, no cross-store joins) plus cross-store
  equality of the distinct cross-domain reference sets (`escrow_id`,
  `reserve_receipt_ref`, `quote_ref`, `receipt_ref`) that point at KS-8.1
  assignments / KS-8.8 tips by id.

PRIVACY: private-message threads/messages are sensitive; the Postgres twin
stores exactly what D1 stores (bodies behind `content_ref`), and every
diagnostic and verify line carries row keys and sha256 hashes only — never
subjects, participants, or message content. Diagnostics reuse the content
lane's events (`khala_sync_forum_dual_write_failed` /
`khala_sync_forum_write_unclassified`). Actual Postgres read serving and the
D1 drop stay deferred/epic-gated exactly as for the content core.

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

### Sites REMAINDER tables (KS-8.12 follow-up, #8357)

The remainder 36 tables ride the SAME shared registry
(`sites-content-tables.ts`), the SAME mirroring seam
(`sitesContentDatabaseForEnv`), and the SAME flags as the core above —
they were simply added to the registry (Postgres twins in khala-sync
migration `0025_sites_remainder.sql`), so no new flag or wiring is
needed: the mirror auto-classifies + read-back-mirrors them wherever a
sites write call site is already wrapped, and
`scripts/backfill-sites-content.ts` now backfills + verifies the full
core+remainder set (`ALL_SITES_CONTENT_TABLES`). Scope:

- **Content satellites** — build validations, revision feedback,
  compatibility checks, provisioning plans, storage bindings, source
  exports, and the referral family (`site_referral_sources`,
  `referral_invites`, `site_referral_policy_events`).
- **`site_environment_values` (SECRETS, invariant 9)** — the twin
  carries metadata + the `secret_ref` INDIRECTION only. `plain_value` is
  EXCLUDED from the registry column list, so neither the dual-write
  mirror nor the backfill ever reads or ships it. (Because the twin omits
  `plain_value`, an env-values row is not byte-identical across stores;
  reads stay `d1` this lane so no `compare` runs against it.)
- **Commerce / money** — `site_commerce_*`, `site_mdk_*`,
  `site_payment_catalog_items`, `site_referral_payout_ledger_entries`.
  D1 stays the money authority; the twin is MIRROR-ONLY. These reference
  the KS-8.7/8.8 rails BY ID and MUST NOT fork them (plain text refs, no
  FKs). Verify adds commerce totals to the cent (`SUM(amount)` per asset)
  and set-membership referential checks (revenue-share → payment-event,
  payout-ledger → referral-source, invite → referral-source) run WITHIN
  each store — no cross-store joins.
- **Targeted sites + hostnames + legacy** — `targeted_site_*` (14),
  `tenant_custom_hostnames`, legacy `deployments`/`deployment_events`.
  `targeted_site_campaign_metric_events` is DELIBERATELY EXCLUDED — the
  Analytics-Engine-candidate campaign firehose stays on D1/AE pending a
  telemetry-sink decision.

Backfill/verify is the same procedure as the core (steps 2–3 above);
`bun scripts/backfill-sites-content.ts --verify` now also prints the
commerce totals and the referential set-membership section. Read cutover
(Scope E) stays DEFERRED for the whole sites domain, so read-serving
secondary indexes are re-derived at that cutover rather than ported now.
Same rollback: one flag flip back to `d1` / `off`.

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
  projects, prefilled workspaces, share projections, share-projection
  recipients, file↔message refs).
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

Delete tombstones (KS-8.13 follow-up, #8356): hard-delete paths append
`op:"delete"` changelog entries so removals replicate to scope
subscribers (interactive chat/thread/file surfaces soft-delete via
`deleted_at`, which rides as a normal upsert and needs no tombstone).
The one hard-delete family with a scope consumer is
`share_projection_recipients` (`replaceRecipients` deletes the whole
audience by `share_id`, then re-inserts): the Worker seam reads the rows
the delete will remove BEFORE it commits (their scope/key columns are
gone afterward), converges the Postgres twin, and appends one tombstone
per resolved scope. A recipient is now a scope-native entity
(`KhalaCodeShareProjectionRecipientEntity`,
`entity_type = share_projection_recipient`) projected into the SUBJECT's
own scope: `subject_kind='user'` → `scope.user.<id>`, `'team'` →
`scope.team.<id>`; `'email'` subjects have NO sync scope (the id is PII,
never a `scope.*.<id>`) and stay Postgres-mirror-only. `display_name` is
structurally absent from the contract. The pre-read is best-effort: a
failed capture still lets the D1 delete + Postgres converge proceed and
only withholds the tombstone (fail-soft, like the projection skip). All
OTHER remainder families (workroom templates, cloud sandbox/fine-tuning,
feedback/head-to-head/unsupported, download/outside-run/trace-plugin
receipts, `prefilled_workspace_*` child rows) stay Postgres-mirror-only
with NO scope fan-out by design — a future scope-native consumer for any
of them is a follow-up contract lane, and money-bearing receipt families
project public-safe state only if/when they ever fan out.

Diagnostics are row-key only: `khala_sync_khala_code_state_dual_write_failed`
is the drift metric (it also fires if a hard-delete tombstone append
fails; the twin delete still converged); `khala_sync_khala_code_state_write_unclassified`
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

## Gym/evals domain cutover (KS-8.15 remainder, #8355)

The active Wave D gym/evals remainder: 16 D1 tables → same-named Postgres
twins. Historical khala-sync migration `0026_gym_evals_domain.sql` created 21
twins; #8380 retired the write-dead `gym_agentcl_eval_*` family with Worker
migration `0301_drop_gym_agentcl_eval_tables.sql` and khala-sync migration
`0031_drop_gym_agentcl_eval_tables.sql`. The active registry now covers
`gym_*` (6), `mullet_*` (5), `blueprint_*` (3), `replay_clip_jobs`,
`mirrorcode_runs`.
Machinery mirrors the training core:
`apps/openagents.com/workers/api/src/gym-evals-domain-store.ts` (row-level
seam over the shared registry
`packages/khala-sync-server/src/gym-evals-domain-tables.ts`, fail-soft
read-back mirror + `make*ForEnv` store drop-ins) and
`packages/khala-sync-server/scripts/backfill-gym-evals.ts`
(cursor-resumable backfill + verify).

R2 PAYLOAD SPLIT (the issue's gate): `gym_harbor_full_trace_archives`
carries ONLY refs/metadata — the archive tarball body lives in R2
(`putArchive` → `bucket.put`), and D1/Postgres keep `artifact_r2_key` +
`artifact_sha256` + `artifact_bytes`. The twin never carries a body; no
table is skipped (every remainder D1 row is refs/metadata/public-safe
projection JSON).

DERIVED SNAPSHOTS — VERIFY BY COPY-EQUALITY, DON'T RECOMPUTE:
`gym_ladder_leaderboard_snapshots.ladder_json` and
`gym_run_progress_snapshots.progress_json` already hold the public-safe
projection the D1 write path built. The backfill copies those bytes
verbatim; `--verify` proves the "leaderboard recomputation equality"
acceptance as newest-N full-row hash equality — Postgres never recomputes
a leaderboard.

RETIRED (Wave 1 #8380): the five `gym_agentcl_eval_*` tables had no live
Worker writer and were removed from the active registry/backfill before the
Worker D1 and Postgres twin drop migrations landed. They are migration history
only and are no longer copied, verified, dual-written, or kept alive by tests.

Flag (Worker vars):

- `KHALA_SYNC_GYM_EVALS_DUAL_WRITE` — default **on** wherever
  `KHALA_SYNC_DB` exists; `off|0|false|disabled` disables the mirror.
- `KHALA_SYNC_GYM_EVALS_READS` — parsed (default `d1`) and reserved for
  this follow-up: reads stay on D1 authority this lane so public gym
  projections never regress mid-cutover; the derived-snapshot read flip to
  Postgres lands with the read-cutover follow-up.

Flag-flip order:

1. **Dual-write on** (default after #8355 lands + `0026` applied). The
   live gym stores (run-progress, mirrorcode, ladder, mutalisk delegation,
   harbor full-trace archive) mirror via their `make*ForEnv` drop-ins.
   Watch `khala_sync_gym_evals_dual_write_failed` — that event IS the
   drift metric. The `mullet_*` / `blueprint_*` / `replay_clip_jobs`
   writers are transactional/functional and route-threaded; their
   call-site mirror wiring lands here (their twins + backfill + contract
   coverage ship now).
2. **Backfill**: from `packages/khala-sync-server/`,
   `KHALA_SYNC_DATABASE_URL=<direct-url> bun scripts/backfill-gym-evals.ts`
   (rowid-cursor resumable via `.gym-evals-backfill-state.json`). Re-run
   `--restart` as the catch-up sweep.
3. **Verify**: `bun scripts/backfill-gym-evals.ts --verify` — exact row
   counts, newest-50 full-row hashes (the derived-snapshot equality), and
   lifecycle state tallies. Post the output on #8355. No cutover on a red
   verify.
4. **Retire remaining tables later**: broad D1 retirement and deleting flags is
   deferred to KS-8.19 (#8330). Until then rollback is one flag flip:
   `KHALA_SYNC_GYM_EVALS_DUAL_WRITE=off`. D1 authority is never behind.

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
  `khala_sync_forge_read_compare_failed`. `postgres` (LIVE in prod +
  staging since 2026-07-05, #8358) SERVES that `listRefs` ref
  advertisement from the Postgres twin via
  `makePostgresForgeGitCanonicalStore.listRefs`, and is FAIL-SOFT: any
  Postgres error (acquire/query/decode) falls back to the D1 authority for
  that one call and logs `khala_sync_forge_postgres_read_serve_failed`, so
  the advertisement can never break. WRITE authority stays on D1 in every
  mode — the ref-lock port is not yet wired as write authority.

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
   the mismatch log is silent. For a near-zero-traffic domain like Forge,
   a fresh full `--verify` (exact ref-set digest) is stronger evidence
   than a passive tail soak that observes no organic comparisons.
6. **Read cutover — DONE (#8358, 2026-07-05):** `KHALA_SYNC_FORGE_READS=postgres`
   serves the `listRefs` ref advertisement from Postgres, fail-soft to D1.
   Rollback is one flag flip back to `d1`/`compare`.
7. **Write cutover LATER**: wiring `makePostgresForgeGitCanonicalStore` as
   write authority (real `SELECT ... FOR UPDATE` ref locks), re-adding the
   deliberately-unported uniques, and moving all five forge stores'
   authority to Postgres coherently — a domain-wide flip, not a one-store
   swap — is the remaining #8358 work. The D1 drop stays with KS-8.19
   (#8330). Until the write cutover, D1 remains the write authority.

Rollback at ANY step: set `KHALA_SYNC_FORGE_READS=d1` (reads) and/or
`KHALA_SYNC_FORGE_DUAL_WRITE=off` (writes). D1 authority is never
behind.

### 2026-07-05 cutover evidence + ref-lock port status (#8358)

- **Migration check:** `bun scripts/migrate.ts --dry-run` against the
  direct Cloud SQL URL reported 34/34 migration files already applied
  (`0021_forge_domain.sql` among them) — nothing to apply.
- **Backfill x2 + verify:** production Forge traffic is genuinely tiny
  today — one live tenant (`tenant.openagents`), single-digit rows per
  table. Sweep 1 (`bun scripts/backfill-forge.ts --restart`) and sweep 2
  converged the same rows (no drift between sweeps — dual-write is
  keeping pace). `bun scripts/backfill-forge.ts --verify --verify-newest 50`
  came back **CLEAN**: exact row counts on all sixteen tables, every
  newest-hash check matched, the per-(tenant, repository) ref-set digest
  matched (1 repository), the per-(tenant, queue) merge-queue replay
  digest matched (0 queues, vacuously clean).
- **Ground-truth git cross-check (the actual §3.13 acceptance
  authority):** the live Forge intake surface only implements the
  receive-pack advertisement (`GET .../info/refs?service=git-receive-pack`)
  — there is no `git-upload-pack` route, so a plain `git ls-remote <url>`
  CLI invocation 404s. Minted a bounded 15-minute `git:receive-pack`
  verification token for the one live tenant/repository
  (`tenant.openagents` / `repo.openagents.issue6771.live.20260628190038-48007`,
  inserted directly with the same schema/hash/prefix convention
  `mintGitAccessToken` uses, `source_refs_json` tagged
  `ks8-16-cutover-ls-remote-crosscheck`), called the real advertisement
  endpoint with `curl` + a bearer header, and hand-parsed the pkt-line
  response: it advertised `refs/heads/main` at
  `a909337789007a12fa1dd48d5acf2cdfa44fe165` — an EXACT match against
  both stores' `forge_git_refs` row. The token was revoked immediately
  after (`state='revoked'`), and a follow-up backfill sweep converged the
  mint/revoke rows into Postgres, re-verified CLEAN.
- **Compare-mode soak:** `KHALA_SYNC_FORGE_READS=compare` shipped to
  production + staging via `deploy:safe` (Worker version
  `75c8132b-9994-4a59-a17a-751e185b011d`). Watched live via `wrangler tail`
  on production for ~7 continuous minutes (2026-07-05 08:47-08:54 UTC),
  spanning all Worker traffic, not just Forge — plus 6 real
  `GET .../info/refs?service=git-receive-pack` calls against the one live
  tenant/repository (via a second bounded verification token, minted and
  revoked the same way as the ground-truth check above), spaced across
  the window: all HTTP 200, and **zero**
  `khala_sync_forge_read_compare_mismatch` /
  `khala_sync_forge_read_compare_failed` / `khala_sync_forge_dual_write_failed`
  lines anywhere in the ~800-line capture. **This is a real but SHORT soak,
  not a representative one** — this domain has effectively no organic
  traffic, so 7 minutes with 6 self-generated reads is necessary-but-far
  short of sufficient evidence for a `postgres` flip. Re-ran the backfill
  + `--verify` after the soak (the soak token's mint/revoke rows) — clean.
  `KHALA_SYNC_FORGE_READS=compare` stayed live in prod/staging after the
  first pass so the soak kept accumulating passively; the second pass
  (below) flipped to `postgres`.

### 2026-07-05 read cutover — `KHALA_SYNC_FORGE_READS=postgres` (#8358, second pass)

- **Decision:** flipped reads to `postgres`. Prior to this pass, setting
  the flag to `postgres` was a code-level NO-OP (it logged a one-time
  `khala_sync_forge_postgres_reads_deferred` and still served D1). This
  pass implemented REAL Postgres read serving for the `listRefs` ref
  advertisement in `makeForgeGitCanonicalStoreForEnv`
  (`makePostgresForgeGitCanonicalStore.listRefs`), FAIL-SOFT: any Postgres
  error falls back to the D1 authority for that call and logs
  `khala_sync_forge_postgres_read_serve_failed`, so the advertisement can
  never break.
- **Evidence gathered this session (all read-only, no live token minting):**
  1. A FRESH full `bun scripts/backfill-forge.ts --verify --verify-newest 50`
     against the direct prod Cloud SQL URL — **CLEAN**: all 16 tables exact
     row counts + newest-50 row hashes match; the per-(tenant, repository)
     REF-SET DIGEST (the §3.13 ls-remote twin — the exact bytes the
     advertisement serves) matches for the 1 repository; merge-queue replay
     digests match. This directly proves the Postgres rows now served are
     byte-identical to the D1 authority.
  2. ~20 minutes of `wrangler tail` on production: ZERO forge advertisement
     requests, ZERO forge diagnostics. This domain has effectively no
     organic traffic, so a passive tail yields no comparisons — the fresh
     full `--verify` is the higher-signal evidence and was used instead of
     minting further live tokens for synthetic traffic.
  3. The prior live git-advertisement ground-truth cross-check (exact
     `refs/heads/main` object-id match, first pass above) still holds.
- **Unit coverage:** `forge-domain-repository.contract.test.ts` now proves
  postgres mode returns the Postgres value (not D1) for the served
  advertisement, and that a dead twin falls back to D1 and logs
  `khala_sync_forge_postgres_read_serve_failed`.
- **Write cutover — DELIBERATELY NOT DONE this pass:** wiring
  `makePostgresForgeGitCanonicalStore` as write authority is a
  domain-wide flip (five forge stores currently write D1-first + mirror to
  Postgres; flipping only the canonical git store would split authority
  incoherently) and requires re-adding the six deliberately-unported
  Postgres uniques so the write authority enforces the same integrity D1
  does. Left unwired rather than risk a tenant's git-ref integrity on a
  piecemeal flip — see below.
- **Ref-lock protocol port — IMPLEMENTED, NOT WIRED:**
  `apps/openagents.com/workers/api/src/forge-git-canonical-postgres-store.ts`
  (`makePostgresForgeGitCanonicalStore`) ports the D1
  held/applied/rejected lock-row dance onto real Postgres primitives: a
  `pg_advisory_xact_lock` per `(tenant_ref, repository_ref, ref_name)`
  (transaction-scoped, auto-released at COMMIT/ROLLBACK — needed because
  a brand-new ref's 'create' has no row yet for a plain row lock to
  hold) taken BEFORE the precondition check, plus a real
  `SELECT ... FOR UPDATE` on the ref row when one already exists (the
  literal §3.13 mechanism, re-validating the CAS precondition under
  lock). There is no lock-row bookkeeping at all in this path — nothing
  writes `forge_git_ref_locks`. `forge-git-canonical-postgres-store.test.ts`
  proves it against a real ephemeral Postgres, including the two races
  that actually matter: two simultaneous CREATEs of the same brand-new
  ref, and two simultaneous UPDATEs racing the same `old_object_id` —
  both cases resolve to exactly one winner, the loser gets the same
  typed `forge_git_unsafe_ref_update` the D1 lane raises, and the final
  ref state is never corrupted. **This store has no production call
  site yet** — it is deliberately landed in isolation so the locking
  design can be reviewed and proven before it is ever on the write path
  for a real git ref. Wiring it as write authority is the WRITE cutover
  step (below), still pending the coordinated domain-wide flip.
- **D1 drop:** confirmed out of scope for this lane — per the current
  KS-8.1/KS-8.2-established policy (also applied to KS-8.6/#8335 and
  KS-8.9/#8336 this same day), per-domain D1 drops are consolidated into
  the epic-closing KS-8.19 sweep (#8330). Not attempted here.
- **What's left before this issue can close:** the WRITE cutover — wiring
  `makePostgresForgeGitCanonicalStore` as write authority across all five
  forge stores coherently and re-adding the six deliberately-unported
  Postgres uniques (a domain-wide flip, NOT a one-store swap; corruption
  risk if done piecemeal, so deliberately left undone this pass) — and the
  D1 drop, which stays with #8330. The READ cutover is DONE (reads served
  from Postgres, fail-soft). Left OPEN on #8358 with this status.

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
  summary read (balance) routes with a separate `routeReads` opt-in. As
  of the #8337 follow-up (below), the SAME flag also unlocks real
  serving for a bounded allowlist of four other display-only surfaces
  (`BILLING_DOMAIN_POSTGRES_SERVED_READ_TABLES`,
  `billing-store.ts`) — those are wired unconditionally whenever the
  flag isn't `d1`, no `routeReads`-style opt-in needed.

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
(all six create paths + site-checkout challenges), annotated pay-in
plans through `runLedgerStatements` on the tip-ladder (forum + pylon
tips), tips-sweep + forwarding reconcile, USD-credit bridge, and MPP/
Lightning mints, **plus, as of the KS-8.7 follow-up (#8337, 2026-07-05):**
`first_batch_payment_policies` (operator-order-triage —
`OrderTriageRuntime.firstBatchPaymentPolicyMirror`, wired at
`OrderTriageService.layer`), `business-starter-credit.ts` `createGrant`'s
USD-credit pay-in leg, `cloud/cloud-metering.ts`
`settleCloudPrimitiveCharge` (fine-tuning + sandbox-compute charges via
their route-level deps), `inference/metering-hook.ts`
`makeLedgerMeteringHook` (the live inference charge path), and
`inference/inference-abuse-controls.ts` `clawbackInferenceCredits` (typed
and wired but has no production call site yet). `labor-escrow.ts` was
audited and found to only ever write treasury-domain
(`agent_balances`/`labor_escrows`) rows through its OWN always-on
annotated mirror (KS-8.8, #8319) — it never carries a `payInId`
annotation, so there was no billing-domain gap to close there; the
original RUNBOOK listing of it was a miscategorization.

**2026-07-05 production `--restart` + `--verify` evidence (#8337):** ran
against `khala_sync_prod` as `khala_app` (the same role the live mirror
uses). 20 of 21 tables came back exact — row counts, the FULL per-user
`billing_ledger_entries` balance map (SUM(amount_cents) to the cent),
grouped (currency, source)/(pay_in_type, state)/(direction, kind) msat and
cents sums, the `stripe_webhook_events` key-set digest, and newest-50 row
hashes all matched. `pay_ins` (301/301), `billing_ledger_entries`
(2264/2264), `billing_accounts` (44/44), `first_batch_payment_policies`
(3/3, the newly-wired writer) all converged and verified exact.
`billing_ledger_entries_next` confirmed ABSENT in both D1 and Postgres
(no live twin).

**One non-exact table, root-caused and explained:** `pay_in_legs` came
back `d1=323 postgres=321` (2 rows short), with grouped sums still exact
(`in:balance` and `in:lightning` msat sums matched d1=pg despite the
2-row gap, because SQLite's `SUM()` coerces the corrupted text values to
0 rather than erroring). Root cause: a genuine, real, pre-existing
production data bug in `inference/usd-credit-bridge.ts`'s
`usdCreditGrantStatements` — the audit-leg INSERT bound `party_ref` and
`amount_msat` in the WRONG param order (D1/SQLite's weak typing silently
accepted a text value in the `amount_msat` slot for years; Postgres's
strict `amount_msat bigint NOT NULL CHECK (amount_msat > 0)` column
rejected it on the first real converge attempt). This does **not** affect
any actual balance or credited amount — the balance credit itself is a
separate, correctly-parameterized `UPDATE agent_balances` statement in the
same atomic batch; the bug only corrupted that ONE audit leg row's own
`party_ref`/`amount_msat` columns. Exactly two historical rows in all of
production carry this corruption (one from the still-live
`usd-credit-bridge.ts` path, now fixed in this pass; one from the
already-removed MPP/x402 chat endpoint, #8387). The code bug is fixed
(`inference/usd-credit-bridge.ts`, with a regression test asserting the
audit leg's actual column values); the two already-corrupted D1 rows were
deliberately left untouched and unmirrored — mirroring the corrupted bytes
would violate the Postgres schema, and "correcting" them without owner
sign-off would rewrite historical financial audit-trail data. Tracked in
[#8412](https://github.com/OpenAgentsInc/openagents/issues/8412) for an
owner-gated historical-correction decision. Money reads never cut on a
red verify — see below, unaffected either way since the epic-gated read
flip has not happened yet.

**Not done in this pass (deliberately, per money discipline):** the
epic-gated `KHALA_SYNC_BILLING_READS=postgres` production decision on
#8282 (reads stay `d1`; the compare-mode soak hasn't even started for
this domain yet); moving the remaining D1-direct reads (recent-ledger-
entries projection, auto-top-up state reads, checkout receipt reads,
buyer-payment pipeline reads, pay-in receipt/tip-earnings reads) onto the
routed-read machinery — this is real, separate implementation work (new
Postgres-routed queries + re-derived indexes + their own compare evidence
per surface), matching the KS-8.6/Artanis precedent (#8335) of leaving
this to a later pass; stopping dual-write; snapshotting to R2; and
dropping any of the 22 D1 tables (per the #8330 KS-8.19 consolidation
policy, bulk domain drops wait for that closing sweep).

### 2026-07-05 follow-up #2: bounded Postgres read allowlist (#8337)

The "recent-ledger-entries projection, auto-top-up state reads, checkout
receipt reads, pay-in receipt reads" gap named above is now CLOSED for
four of the five named surfaces, following the exact
`BUSINESS_DOMAIN_POSTGRES_SERVED_READ_TABLES` bounded-allowlist pattern
KS-8.14 established for the business-funnel lane (#8360), adapted to this
domain's per-function (not generic-D1Database-proxy) architecture:

- **`billing-store.ts`** now names
  `BILLING_DOMAIN_POSTGRES_SERVED_READ_TABLES` — a `ReadonlySet<
  BillingDomainTable>` covering `billing_ledger_entries`,
  `billing_auto_top_up_policies`, `billing_auto_top_up_events`,
  `stripe_saved_payment_methods`, `stripe_checkout_sessions`, `pay_ins`.
  Since this lane has no generic "does this SQL touch table X" classifier,
  the Set is a documentation/audit registry that four HAND-WRITTEN,
  single-purpose read functions consult — never a blanket gate an
  unrelated future statement could ride.
- **Recent-entries display projection** (`billing.ts`'s
  `readRecentLedgerEntries`, now routed via the new `BillingRuntime.
  recentEntriesRead` hook) and **auto-top-up DISPLAY state**
  (`readBillingAutoTopUpState`, routed via the new `autoTopUpStateRead`
  hook) — both wired UNCONDITIONALLY by `billingRuntimeForEnv` whenever
  `KHALA_SYNC_BILLING_READS !== 'd1'` (no separate `routeReads`-style
  opt-in like the balance read needs, because only the display summary
  path ever calls either hook). The auto-top-up CHARGE decision
  (`chargeAutoTopUp`, stripe-billing.ts) is untouched — it still reads its
  own dedicated D1 query directly and takes no runtime hook at all.
- **Stripe checkout receipt read** (`stripe-checkout-receipts.ts`) and
  **inference/pay-in receipt read** (`inference-receipts.ts`) are
  standalone stores outside `BillingRuntime`; each gained a
  `makePostgres*Store` twin plus a `makeReadsRouted*Store` compare/
  postgres router (reusing the shared `BillingPostgresRawQuery` seam,
  `billingPostgresRawQueryForEnv`), and an env-wiring composer
  (`stripeCheckoutReceiptStoreForEnv`, `inferenceReceiptStoreForEnv`) now
  wraps the three `index.ts` call sites (public checkout-receipt route,
  public inference-receipt route, hosted-Gemini-promise-readiness route).
  The inference receipt's free-allowance branch reads a DIFFERENT domain's
  table (`inference_free_usage_events`, no live Postgres mirror in this
  lane) — the Postgres store explicitly refuses that ref shape
  (`InferenceReceiptPostgresNotServableError`) and the router transparently
  falls back to D1 for it, in every mode. (The public activity-timeline
  route's own `makeD1InferenceReceiptStore` call site was left unwired —
  its narrower `PublicActivityTimelineRouteInput` doesn't carry
  `KHALA_SYNC_DB` today; threading that through is a small, separate
  follow-up.)
- **Migration `0034_billing_bounded_read_indexes.sql`** re-derives the two
  missing read accelerators (`billing_auto_top_up_events` had NO user_id
  index at all; `pay_ins`'s `_public_receipt_ref` index was dropped by
  `0015` since nothing served it yet) — the other four surfaces already
  hit an existing PK/UNIQUE index.
- **Deliberately NOT allowlisted this pass** (documented as future,
  individually-reviewed-pass candidates in `billing-store.ts`'s own
  comment): the buyer-payment pipeline
  (`buyer_payment_challenges`/`receipts`/`entitlements`/`redemptions`/
  `reconciliation_events`) — every read in `buyer-payment-ledger.ts`'s
  `makeD1BuyerPaymentLedgerStore` is SHARED between the read-only
  checkout-return/payment-proof status routes and the
  challenge/webhook/redemption idempotency-dedupe decision paths, and
  cannot be split into a decision-free surface without store-interface
  surgery; and the forum tip-earnings leaderboard/creator-earnings
  projections (`forum/tip-earnings.ts`), which JOIN
  `pay_ins`/`pay_in_legs` against `forum_posts` (a different domain's
  mirror) in a single statement.

**Verification (real local Postgres, contract tests):**
`billing-repository.contract.test.ts` (28 tests, extended in this pass),
new `stripe-checkout-receipts.test.ts` (4 tests), new
`inference-receipts.test.ts` (6 tests) — all prove parity (D1 vs.
Postgres-served answers agree on real fixtures), REAL serving (a value
diverged directly on the Postgres twin is what `postgres` mode reads back
— proving genuine serving, not a D1-served shadow compare), fail-soft
fallback (including a real broken-connection Postgres store, not just an
injected throw), and compare-mode logging discipline (mismatch only on
genuine disagreement, never a false positive from field-key ordering —
the router's equality check is key-order-insensitive, `stableStringify`,
not raw `JSON.stringify`).

**No flag flip in this pass.** `KHALA_SYNC_BILLING_READS` stays `d1` in
production; deploying with `compare`/`postgres` for this bounded surface
and recording that decision on #8282 is separate, later work — same
epic-gated discipline as the balance read.

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
  `postgres` (#8360, the read-cutover follow-up) serves REAL Postgres reads
  ONLY for the bounded allowlist `BUSINESS_DOMAIN_POSTGRES_SERVED_READ_TABLES`
  in `business-domain-store.ts` (today: `business_funnel_events` alone);
  every other comparable-select in this domain — including the escalation
  pager and every referral-attribution existence-check — stays D1-served
  under `postgres` PERMANENTLY, logging
  `khala_sync_business_postgres_reads_deferred`.

Flags (Worker vars; structural — absent means default):

- `KHALA_SYNC_BUSINESS_DUAL_WRITE` — default **on** wherever
  `KHALA_SYNC_DB` exists; `off|0|false|disabled` disables the mirror.
- `KHALA_SYNC_BUSINESS_READS` — default `d1`; `compare` shadow-runs
  scoped-table SELECTs against Postgres, SERVES D1, and logs
  `khala_sync_business_read_compare_mismatch`; `postgres` serves the
  bounded allowlisted surface for real (fail-soft back to D1 on a Postgres
  read error, `khala_sync_business_postgres_read_serve_failed`) and defers
  every other comparable-select to `compare` behavior.

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
   consumption). LANDED in #8359.
7. **Bounded read cutover (LANDED, #8360):** `KHALA_SYNC_BUSINESS_READS=postgres`
   now serves REAL Postgres reads, but ONLY for the allowlist
   `BUSINESS_DOMAIN_POSTGRES_SERVED_READ_TABLES` in
   `business-domain-store.ts` (today: `business_funnel_events` alone — the
   public funnel dashboard's two full-table aggregate reads, re-derived
   index in khala-sync migration
   `0033_business_funnel_events_dashboard_read_index.sql`). Every OTHER
   comparable-select (the escalation pager, referral-attribution
   existence-checks, pipeline/order/referral list reads) stays D1-served
   under `postgres` PERMANENTLY — not a staging step — because those reads
   feed write-path decisions or cron evaluators where a lagging mirror read
   could silently corrupt business logic. A Postgres read error on the
   allowlisted surface fails soft back to D1
   (`khala_sync_business_postgres_read_serve_failed`) and never fails the
   request. Widening the allowlist to another table is a separate,
   individually reviewed follow-up. Production backfill/`--restart`/
   `--verify` are green (evidence on #8282/#8360); the actual production
   flag flip (`compare` → `postgres`) is still the EPIC-GATED step 8 below.
8. **D1 drop**: consolidated into the epic's KS-8.19 closing sweep (#8330)
   per the owner's decision to skip per-domain D1-drop tickets — NOT done
   in this domain's lane. A final `--restart` sweep + `--verify`
   immediately before any read cutover is MANDATORY, not optional (the
   unwired boundaries are backfill-converged until then).

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

## Identity/auth domain cutover (KS-8.18 #8329 + follow-up #8362)

The KS-8.18 domain migration — the LAST and most sensitive domain: the
SEVENTEEN canonical identity/auth tables (`users`, `auth_identities`,
`openauth_storage`, `openauth_agent_links`, `github_write_connections` /
`_connection_attempts` / `_auth_grants`, and the provider (BYOK) account
custody family: `provider_accounts`, `_connection_attempts`,
`_auth_grants`, `_events`, `_sanity_checks`, `_parallel_probe_receipts`,
`_leases`, `_failover_receipts`, `_token_custody`, `_token_custody_audit`)
(D1) → same-named Postgres twins (khala-sync migration
`0028_identity_auth_domain.sql`). Machinery:
`apps/openagents.com/workers/api/src/identity-auth-domain-store.ts` (the
`identityAuthMirrorFromEnv` fail-soft read-back mirror handle, its
`mirrorDeleteByKey` delete-mirror counterpart added in #8362, the
flagship `makeProviderAccountTokenCustodyStoreForEnv` drop-in, and the
four more drop-ins #8362 added: `makeOpenAuthStorageForEnv`,
`makeGitHubWriteRepositoryForEnv`, `makeProviderAccountRepositoryForEnv`)
and `packages/khala-sync-server/scripts/backfill-identity-auth.ts`
(backfill + verify).

**WHY LAST, and why extra caution.** Auth runs on EVERY request — this is
the hottest read family and the maximum blast radius. A bad cutover
breaks literally everything. It goes last, after the recipe has been
proven ~14 times. This lane lands MACHINERY ONLY: D1 stays the SOLE
authority; there is NO read cutover here.

SECRETS (SPEC invariant 9 — the invariant this domain motivated). The
twin holds EXACTLY what D1 holds (no widening), same at-rest encryption
posture. Raw tokens live on NEITHER engine — `provider_account_token_custody`
holds AES-GCM ciphertext keyed by KMS key id. Custody columns (that
ciphertext + its IVs + key ids; `openauth_storage.value_json`;
`provider_account_connection_attempts.user_code`;
`github_write_connection_attempts.state`) are twinned byte-for-byte but
NEVER appear in diagnostics or backfill/verify output — row KEYS
(ids/refs/owner_user_id) and sha256 hashes ONLY. If ANY log line or
verify row ever shows a ciphertext, session payload, device code, or
state nonce, treat it as an incident, not drift.

Diagnostics: the drift metric is `khala_sync_identity_dual_write_failed`
(keys only). Treat a nonzero steady rate as drift — fix, then re-run the
backfill sweep.

Flags (Worker vars):

- `KHALA_SYNC_IDENTITY_DUAL_WRITE` — default **on** wherever
  `KHALA_SYNC_DB` exists; `off|0|false|disabled` disables the mirror.
- `KHALA_SYNC_IDENTITY_READS` — default `d1`. There is NO routed identity
  read in this lane: `postgres` DEFERS (logs
  `khala_sync_identity_postgres_reads_deferred` once and still serves D1),
  so a premature flip can never serve an unproven AUTH read path.

WIRING STATUS (updated by follow-up #8362). #8329 wired the flagship
secret-bearing owner — the provider-account token-custody vault — end-to-
end. #8362 wired every remaining write call site: the five other typed
factories (`makeD1GitHubWriteRepository`, `makeD1ProviderAccountRepository`,
`makeD1Storage`, `makeD1AgentRegistrationStore`, `makeD1AgentOwnerClaimStore`
— each now has a `make*ForEnv(env)` drop-in in `identity-auth-domain-store.ts`
that read-back mirrors every write method) and the scattered inline
writers in `index.ts` (`upsertGitHubUser`/`upsertEmailUser`/`upsertUser`),
`onboarding/repository.ts` (all five `users` UPDATEs),
`auth/email-otp-hardening.ts` (the SECOND `openauth_storage` writer,
`reserveAuthEmailOtpSend`), `operator-provider-account-routes.ts`,
`provider-account-pool-routes.ts`, and `artanis-operator-dashboard-routes.ts`
(all now take an optional `IdentityAuthMirror` parameter threaded from the
nearest call site holding `env`). The shared machinery also gained a new
`mirrorDeleteByKey`/`deleteRows` capability for `openauth_storage.remove()`
— the only hard-delete write call site in this domain.

**Deliberately still unmirrored (documented inline at each site), and
this is intentional, not a gap to close later:** D1's own incidental
bulk/lazy-expiry side effects on HOT READ paths — the
`provider_account_leases` stale-expiry sweeps embedded in
`acquireProviderAccountLease`/`expireStaleProviderAccountLeases`/
`expireStalePoolLeases`, and `openauth_storage.get()`'s lazy TTL cleanup
inside `auth/openauth-storage.ts`. Mirroring those would add an
unbounded, per-request Postgres write to a read path — exactly the load
pattern this domain must avoid before any read cutover. Those rows
converge on the next `--restart` backfill sweep instead. A consequence
worth naming plainly: because `openauth_storage` rows are deleted from D1
lazily (only on a read that discovers expiry) while the mirror never
proactively deletes on that path, the Postgres twin will accumulate
expired-but-undeleted rows over time — `--verify`'s row-COUNT equality
for `openauth_storage` specifically will NOT converge to exact zero-drift
the way `users`/`auth_identities` do, purely because of this TTL-shaped
asymmetry, not because of missed wiring. Before any future read cutover
or D1 drop, this needs either an explicit active-TTL prune of Postgres's
own expired rows or an accepted "expected drift source" note analogous to
the tokens-served projection's drift-source list below.

A `--restart` sweep + `--verify` immediately before ANY read cutover
remains MANDATORY regardless (catches any writer this account of the
wiring missed, plus everything written before the Worker deploy that
carries this wiring).

Flag-flip order — never skip a step, each step soaks before the next:

1. **Dual-write on** (default after KS-8.18 lands + `0028` applied via the
   migration runner). Watch `khala_sync_identity_dual_write_failed`; a
   nonzero steady rate blocks progression.
2. **Backfill**: from `packages/khala-sync-server/`,
   `KHALA_SYNC_DATABASE_URL=<direct-url> bun scripts/backfill-identity-auth.ts`
   (wrangler-auth'd; rowid-cursor resumable via
   `.identity-auth-backfill-state.json`). Run it a SECOND time
   (`--restart`) as the catch-up sweep once dual-write has covered the
   whole window AND every writer has been wired (done in follow-up
   #8362 — see "WIRING STATUS" above; a further `--restart` sweep is
   still needed after the #8362 code deploys, to catch writes that
   landed on the pre-#8362 Worker).
3. **Verify**: `bun scripts/backfill-identity-auth.ts --verify` — exact
   row counts (identity SET EQUALITY over `users`/`auth_identities`),
   custody-safe per-state tallies, newest-50 row hashes. Post the output
   on the migration issue (it is secret-safe by construction). Exact or
   explain; no cutover on a red verify.
4. **Auth matrix replay** (the §3.15 acceptance): replay each credential
   class × allow/deny against SHADOW reads and confirm ZERO divergence,
   and run the explicit SESSION-REVOCATION check — revoke in staging and
   observe BOTH stores deny — before considering any read move.
5. **Read cutover — OWNER-GATED, HIGHEST-RISK, DONE LAST.** Serving auth
   reads from Postgres requires: the KV/cache layer in front (so Postgres
   does not inherit a per-request read storm), the session-invalidation
   proof above, custody audit-chain contiguity, re-adding the D1
   uniques/FKs on the twins, and moving write authority. Per
   MIGRATION_PLAN §5, D1 table drops for ALL domains (including this one)
   are consolidated into the closing KS-8.19 sweep (#8330), not any
   per-domain issue. This read-cutover step remains a SEPARATE,
   not-yet-scheduled follow-up on epic #8282 (writer wiring — #8362 — is
   done; the KV/cache layer, auth-matrix replay tooling, and the
   session-revocation staging drill are NOT built yet) — NEVER in the
   same change as the wiring lane, and NEVER without explicit owner
   sign-off.

Rollback at ANY step: set `KHALA_SYNC_IDENTITY_READS=d1` (reads) and/or
`KHALA_SYNC_IDENTITY_DUAL_WRITE=off` (writes). D1 authority is never
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
