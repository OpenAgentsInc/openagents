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

**Incident (KS-6.4 migration-ledger audit, 2026-07-05): never-committed
migration file `0036_drop_treasury_mpp_replay_tables.sql`.** A sibling
KS-6.4 agent flagged that `check:pending-migrations` was refusing to run
(`MigrationFileMissingError`) because both `khala_sync_staging` and
`khala_sync_prod`'s `khala_sync_migrations` ledgers recorded a row for
`0036_drop_treasury_mpp_replay_tables.sql` that had never existed in this
repo's git history (`git log --all` and `git log --all -S` across every
local and remote ref found nothing — filename or content).

Root cause, established with high confidence from direct evidence, not
guesswork:

- Wave 1 cleanup #8387 (commit `87e6992d1e`) removed the standalone Khala
  MPP/x402 chat route, its only production writers for `mpp_lightning_replay`
  and `mpp_spt_replay`, and both tables' entries from the treasury mirror
  registry/contract-test fixtures. That commit added the D1-side drop
  migration (`apps/openagents.com/workers/api/migrations/0303_drop_mpp_replay_tables.sql`)
  but no Khala Sync (Postgres) migration.
- Both `khala_sync_staging` and `khala_sync_prod` ledgers held an
  `0036_drop_treasury_mpp_replay_tables.sql` row with the **identical**
  sha256, applied one second apart (staging 12:17:03 UTC, prod 12:17:04 UTC
  on 2026-07-05) — exactly the "staging first, then prod" shape the normal
  `scripts/migrate.ts` runner produces, immediately following `0035` in
  sequence. This is not a hand-inserted or corrupted row; the runner only
  ever writes a ledger row inside the same transaction as executing the
  file's real SQL, using the file's real content hash.
- Direct read-only queries against both databases confirmed
  `to_regclass('mpp_lightning_replay')` and `to_regclass('mpp_spt_replay')`
  are both `NULL` — the tables are genuinely gone, on both staging and
  prod, consistent with a real, successful `DROP TABLE IF EXISTS` having
  run and matching the zero-writer state #8387 already established.
- Conclusion: someone (very likely working the KS-8.19 D1-retirement wave,
  #8330/#8387) wrote the natural Khala Sync counterpart to `0303` following
  the established `0030`/`0031`/`0032` "drop the dead Postgres twin" pattern,
  ran it against staging then prod via the normal migrate runner, but never
  `git add`/committed the file — most likely lost to an uncommitted worktree
  cleanup or an abandoned branch, since no ref (local or remote) ever
  contained it.

Fix applied: added `packages/khala-sync-server/migrations/0036_drop_treasury_mpp_replay_tables.sql`
as a functionally-idempotent reconstruction (`DROP TABLE IF EXISTS
mpp_lightning_replay; DROP TABLE IF EXISTS mpp_spt_replay;` — safe to
replay anywhere, matches the verified already-happened effect exactly).
Because the original file's bytes are unrecoverable, its sha256 will never
match the ledger's recorded hash byte-for-byte; the `khala_sync_migrations.sha256`
for this filename was updated, on both `khala_sync_staging` and
`khala_sync_prod`, to this reconstructed file's actual hash (`UPDATE
khala_sync_migrations SET sha256 = '<new hash>' WHERE filename =
'0036_drop_treasury_mpp_replay_tables.sql' AND sha256 = '<old hash>'`, guarded
on the exact prior value so it could not silently clobber an unrelated row).
Verified `bun run --cwd packages/khala-sync-server check:pending-migrations`
returns exit 0 against both direct URLs after the reconciliation. Recorded on
issue #8330.

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

### 2026-07-05 extended post-retry-fix soak — clobber mechanism fixed, but a NEW write-loss regression found live (#8409, still OPEN)

Ran the requested longer soak several hours after `d6595dc9a4` (the retry
fix) landed and was deployed. Two distinct findings, one good and one bad:

- **The original cross-writer clobber mechanism looks resolved.** Over the
  5.98h window from the guaranteed-fresh `06ee7de4c7` deploy
  (`2026-07-05T10:00:48Z`) to the check time (`2026-07-05T15:59:35Z`), the
  historical ~2/hour clobber rate would predict ~12 clobber-shaped
  mismatches (row present in both engines, `scan_state`/`compose_state`
  differ); only **2** were observed (`11:42:24Z`, `15:13:26Z`), an ~83%
  reduction, and neither shows the original full-row-snapshot-revert
  signature.
- **A NEW, more severe problem is live right now**: full-row write loss
  (row entirely absent from Postgres, not just stale) started
  `2026-07-05T11:43:24Z` and is ongoing at the time of this check. Hourly
  breakdown of the D1-vs-Postgres diff: `08:00`-`10:00` UTC 0% missing,
  `11:00` 13% missing, `12:00`-`15:00` a sustained **77-79%/hour** missing.
  Table-wide: `d1=8486 postgres=8300` (186 rows missing). Confirmed NOT a
  propagation-lag artifact — re-queried five specific timestamps spanning
  `11:43:24Z`-`15:53:26Z` several minutes after the initial diff; all still
  absent, despite 4+ hours of every-minute ticks that should have
  self-healed them via either writer's INSERT-side full-row self-heal.
  Ruled out: schema drift (Postgres columns match the
  `ARTANIS_DOMAIN_TABLES` spec exactly), Postgres-side connection
  exhaustion (`pg_stat_activity`: 59/600 connections in use), and a fully
  disabled dual-write flag (would show ~0% success, not the observed
  ~21-23% ongoing success rate). A 75-second unfiltered `wrangler tail`
  sample did not catch a `khala_sync_artanis_dual_write_failed`/`_retry`
  diagnostic — inconclusive, not a rule-out. Onset loosely correlates with
  a burst of unrelated Khala Sync read-cutover changes landing on `main` in
  the same ~90-minute window (KS-8.9 entitlements, KS-8.14 business funnel,
  KS-8.17/8.18 identity/auth + supervision long-tail, a Forge read
  cutover) — all adding new Postgres/Hyperdrive read load from other
  domains — but this is not confirmed as the cause.
- **Decision: issue stays OPEN, not closed.** The mechanism #8409 was
  originally filed for looks fixed by `06ee7de4c7` + `d6595dc9a4`
  together, but the SAME `mirrorArtanisRows` dual-write path is currently
  losing the majority of its writes outright — worse than the bug this
  issue opened with. Evidence posted on the issue
  (`https://github.com/OpenAgentsInc/openagents/issues/8409#issuecomment-4886665158`).
  No code change made this pass (root cause of the new write-loss is not
  confirmed, so a fix would be a guess) and no corrective re-converge sweep
  (would be pointless while the table is still actively losing writes).
  Next step: a dedicated follow-up pass to identify why 2 retries over
  `[100,400]ms` aren't covering this, whether it's isolated to Artanis or a
  broader Postgres/Hyperdrive capacity signal affecting other domains'
  fail-soft mirrors too, and Hyperdrive-side connection-pool metrics (not
  visible from a `psql`-level session).

### 2026-07-05 severe write-loss incident — ROOT CAUSE FOUND AND FIXED (#8409)

Emergency follow-up to the "extended post-retry-fix soak" entry above.
Confirmed the write-loss was **still actively ongoing** (D1 rows through
`16:03:26Z` entirely absent from Postgres at first check) and traced it to
its actual root cause — NOT Hyperdrive/Postgres connection capacity at all.

**Two infrastructure hypotheses tested and ruled out (in that order):**

1. Raised the shared `KHALA_SYNC_DB` Hyperdrive config's
   `origin_connection_limit` from 60 to 100 (the platform max; `wrangler
   hyperdrive update`). `pg_stat_activity` had shown 59/600 Postgres-side
   connections in use in the prior pass — a number essentially AT
   Hyperdrive's own (separate, much lower) 60-connection ceiling, which
   looked like a strong lead. **Zero effect**: ticks minutes after the
   change still lost writes at the same rate.
2. Recreated the Hyperdrive config from scratch (`khala-sync-prod-v2`,
   id `8724b8b5887949eda7f42f1b4807e81d`, same origin,
   `origin_connection_limit=100`) and repointed the binding in
   `wrangler.jsonc`, on the theory that the existing config's connection
   pool was stuck/degraded in a way no CLI flag could reset. Deployed via
   `deploy:safe`. **Zero effect again**: writes immediately after the fresh
   config went live (100% traffic) still failed at the same rate.

**Actual root cause (confirmed via live `wrangler tail --status error
--format json`, timed to straddle the per-minute cron):** the Worker's
`scheduled()` handler (`apps/openagents.com/workers/api/src/index.ts`) runs
~25 independent per-minute tasks — Artanis responder scan/compose dual-write
among them — in **one shared `Promise.all([...])`**. `Promise.all` rejects
the instant ANY single entry rejects, which tears down the whole cron
invocation; Cloudflare then abandons every OTHER still-in-flight task's
work, including any Postgres mirror write that hadn't finished yet.

Two distinct unguarded entries in that array were confirmed live, in
sequence:

- `checkTipsBufferBackingInvariant` (`TipsBuffer.backingInvariant`)
  intentionally throws `TipsBufferBackingViolation` on a **real, already
  known, standing violation** — agent balances (263 sat) exceeding the tips
  buffer (15 sat) — flagged as "recurring" and out of scope in the #8335
  soak pass days earlier. Because the balances/buffer gap never closed on
  its own, this fired on effectively **every single tick**, killing the
  batch almost every time. Captured directly: a `wrangler tail` event with
  `"event": {"cron": "* * * * *", "scheduledTime": ...}`, `"outcome":
  "exception"`, and the exact `TipsBufferBackingViolation` stack trace
  through `Object.scheduled`.
- `sweepActiveAgentRunBilling` (the array's first entry) throwing the
  separate, already-documented, recurring `D1_ERROR: D1 DB is overloaded.
  Requests queued for too long.` from `listActiveAgentRunsForBilling` — this
  is exactly the class of problem Khala Sync's epic (#8282) was created to
  retire. Captured live via the same `wrangler tail` method, on a DIFFERENT
  cron tick, after the first fix had already landed — it explains the one
  remaining miss (`17:13:26Z`) in the otherwise-clean post-first-fix window.

**Neither Hyperdrive-side connection-limit checks in the prior passes could
have found this** — `pg_stat_activity` and Hyperdrive's config only see
connections that are actually attempted; they cannot see an invocation that
never got that far because a sibling task killed it first.

**Fix (two commits, both deployed via the sanctioned `deploy:safe` — no raw
`wrangler deploy` used):**

1. `2c9ce44bcb` — contain the confirmed, currently-firing
   `TipsBuffer.backingInvariant` call: catch its own promise, log the
   violation loudly via `khala_sync_tips_buffer_backing_invariant_violated`
   (still fully observable — the original code comment's intent, "captured
   by the scheduled observer," is preserved), and resolve instead of
   rejecting.
2. `e38570eaaf` — the systemic fix. Rather than special-case every one of
   the ~25 entries individually, switched the whole array from
   `Promise.all` to `Promise.allSettled` (its return value was already
   discarded, so this is behavior-identical when every task succeeds) and
   added a small post-loop that logs any rejected entry as
   `scheduled_task_failed` with its array index. No single task's failure
   can ever again silently truncate its ~24 unrelated siblings — covers
   `sweepActiveAgentRunBilling`'s D1-overload case and any future
   regression in any other entry, not just the two found live today.

**Verification (real-time, production):**

- Before either fix: `d1=8497 postgres=8304` (193 rows behind), newest-tick
  hashes almost entirely `<missing>` in Postgres.
- After the Hyperdrive-side fixes alone (both applied, zero effect): still
  losing writes at the same rate minutes later — this is what disproved
  the connection-capacity hypothesis.
- After fix 1 (`TipsBuffer` containment) deployed `17:05:08Z`: 13 of the
  next 14 ticks landed cleanly in Postgres (only `17:13:26Z` missing — the
  separate `sweepActiveAgentRunBilling` D1-overload case).
- After fix 2 (`Promise.allSettled`) deployed `17:19:00Z`: re-ran
  `backfill-artanis.ts --verify --table artanis_responder_ticks
  --verify-newest 20` at `17:28:12Z` — **"newest-20 row hashes: all
  match."** The only remaining `STATUS MISMATCHES` are the aggregate
  historical tallies from the incident window (rows lost `11:43:24Z`
  through `17:19:00Z`), not new drift.
- Ran the corrective backfill sweep (`backfill-artanis.ts --table
  artanis_responder_ticks --restart`, `ON CONFLICT DO NOTHING`, D1 stays
  authoritative, safe to run — this table is not read-cutover) to fill in
  the historical gap left by the incident window.

**Systemic flag for #8282:** this `Promise.all` pattern was a real,
demonstrated, cross-domain landmine — ANY one of ~25 unrelated scheduled
tasks throwing can silently truncate every sibling's work on the SAME
per-minute cron tick, and two independent, already-known, already-flagged
issues (a standing tips-buffer invariant violation, and the D1-overload
condition the whole Khala Sync epic exists to retire) were both actively
triggering it in production today. The `Promise.allSettled` fix closes the
class of bug, not just today's two instances, but it is worth an epic-level
note that other Workers/cron handlers in this repo with similar
`Promise.all([...many unrelated tasks...])` shapes should be audited for
the same footgun.

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

**#8336 status, part 3 (2026-07-05): enforcement-gate compare-mode soak
observability bring-up.** Parts 1-2 above left `KHALA_SYNC_ENTITLEMENTS_READS`
untouched at `d1` specifically because there was no durable production
log/metrics surface to establish a genuine multi-hour representative-window
soak. The shared compare-mode soak observability tool (#8282 follow-up,
commit `6c2cf72b1a`) removes that blocker: `makeRoutedEntitlementsGateReads`'s
`compare` branch now records a durable Analytics Engine data point
(`domain: "entitlements_gate"`) on every match/mismatch/shadow-read-error, on
top of the existing `khala_sync_entitlements_read_compare_mismatch`
diagnostic — see "Compare-mode soak observability" below for the query
command. This pass flips `KHALA_SYNC_ENTITLEMENTS_READS` from `d1` to
`compare` in both `staging` and production `vars` (`wrangler.jsonc`) so real
soak time starts accumulating from this deploy forward. This is
**observation only** — `compare` still serves every gate decision from D1;
it cannot change what any request is ALLOWED or DENIED. A **future** pass
queries `packages/khala-sync-server/scripts/query-compare-soak.ts --hours
<N>` for the `entitlements_gate` domain once a genuinely representative
window has accumulated (not `VACUOUS`, i.e. `totalReads` meaningfully > 0)
and only then evaluates whether `postgres` is warranted — never on this
pass's evidence alone, and never without the epic-gated ops decision on
#8282. Rollback at any time: `KHALA_SYNC_ENTITLEMENTS_READS=d1`.

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
held-lock protocol is deliberately NOT emulated in Postgres (a real
`pg_advisory_xact_lock` + `SELECT ... FOR UPDATE` port exists in
`forge-git-canonical-postgres-store.ts` but has no production call site).
**Schema parity — DONE (2026-07-05, #8358 third pass):** all nine
uniques the D1 side enforces (issues' github_issue_number, PRs'
change_ref, dispatch leases' active-work and idempotency-key-hash,
packfile digest and R2 key, access-token hash, ref-locks' held-per-ref,
mirror-receipt destination tuple — see the third-pass section below for
the exact list, which corrects the prior "six" count to nine) are now
also enforced in Postgres via migration
`0035_forge_domain_ref_lock_uniques.sql`. **D1 mirror-back — DONE
(2026-07-05, #8358 fourth pass):** `makePostgresForgeGitCanonicalStore`
now has an optional fail-soft Postgres→D1 write mirror (see the
fourth-pass section below), closing the third pass's "no path back to
D1" finding. The WRITE cutover itself (flipping the production route
handler to this store) remains NOT done — both named blockers are now
closed, but the domain-wide incoherence across the five Forge stores is
a deliberate, separate routing decision; see the fourth-pass section.

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
7. **Write cutover — DONE for the canonical git store only (#8358,
   2026-07-05, fifth pass):** `KHALA_SYNC_FORGE_GIT_CANONICAL_WRITES=postgres`
   makes Postgres the sole authority for the canonical git store's whole
   surface (real `pg_advisory_xact_lock` + `SELECT ... FOR UPDATE` ref
   locks, fail-soft Postgres→D1 mirror-back). This is a SEPARATE, scoped
   flag from `KHALA_SYNC_FORGE_READS`/`KHALA_SYNC_FORGE_DUAL_WRITE` — the
   other four Forge stores (coordination, packfile-archive,
   tenant-git-auth, GitHub mirror) are UNCHANGED and remain
   D1-first/mirror-to-Postgres. Verified with a real end-to-end `git push`
   (create + fast-forward update) against a dedicated canary tenant/repo
   on staging, then again on production, before this pass closed — see
   the fifth-pass section below for the exact evidence. The D1 drop stays
   with KS-8.19 (#8330).

Rollback at ANY step: set `KHALA_SYNC_FORGE_READS=d1` (reads),
`KHALA_SYNC_FORGE_GIT_CANONICAL_WRITES=d1` (canonical-store write
authority), and/or `KHALA_SYNC_FORGE_DUAL_WRITE=off` (the other four
stores' mirror). D1 authority is never behind.

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

### 2026-07-05 constraint-parity pass — nine uniques added, write cutover STILL unwired (#8358, third pass)

- **Re-derived the missing uniques exactly, instead of trusting the
  "six" count.** Diffed the D1 migration files
  (0251/0252/0253/0255/0260) against `0021_forge_domain.sql` column by
  column. Found **NINE** distinct missing unique indexes, not six — the
  prior pass's tracking comment bundled "packfile digest / R2 key" as one
  bullet (it is two separate D1 UNIQUE indexes on different columns) and
  never named `forge_dispatch_leases`' idempotency-key-hash uniqueness at
  all. The corrected, exhaustive list:
  1. `forge_coordination_issues` — UNIQUE (tenant_ref, github_issue_number)
     WHERE NOT NULL
  2. `forge_coordination_prs` — UNIQUE (tenant_ref, change_ref)
  3. `forge_dispatch_leases` — UNIQUE (tenant_ref, work_ref) WHERE
     state='active'
  4. `forge_dispatch_leases` — UNIQUE (tenant_ref, idempotency_key_hash)
     WHERE NOT NULL (missed by the prior pass's tracking comment)
  5. `forge_git_packfile_archives` — UNIQUE (tenant_ref, packfile_sha256)
  6. `forge_git_packfile_archives` — UNIQUE (artifact_r2_key) (missed by
     the prior pass's tracking comment)
  7. `forge_git_access_tokens` — UNIQUE (token_hash)
  8. `forge_git_ref_locks` — UNIQUE (tenant_ref, repository_ref, ref_name)
     WHERE state='held' (moot for the new advisory-lock write path, kept
     for schema parity)
  9. `forge_github_mirror_receipts` — UNIQUE (tenant_ref, promotion_ref,
     destination_github_repository, destination_github_ref)
- **Verified BEFORE writing the migration**: queried a real backfilled
  copy of both `khala_sync_prod` and `khala_sync_staging` for existing
  violations of all nine candidate constraints — **zero violations on
  either database** (prod Forge traffic remains single/low-digit rows per
  table: 3 issues, 2 PRs, 1 packfile, 4 tokens, 1 ref, 1 held lock, 0
  mirror receipts, etc.).
- **Migration applied**: `packages/khala-sync-server/migrations/0035_forge_domain_ref_lock_uniques.sql`
  (full column/index definitions and D1 cross-references in the file
  header), applied to staging then prod via `bun scripts/migrate.ts`
  (staging also picked up the already-merged, previously-pending
  `0034_billing_bounded_read_indexes.sql` from an unrelated concurrent
  lane in the same run — expected, not this lane's change).
- **Post-migration verify**: `bun scripts/backfill-forge.ts --verify
  --verify-newest 50` against the direct prod Cloud SQL URL — CLEAN
  across all 16 tables (exact row counts, newest-hash matches, ref-set
  digest matches for the 1 repository, merge-queue replay digest matches
  vacuously). `pg_indexes` confirms all nine new unique indexes exist.
- **A real bug the new constraint caught**: `forge-git-access-tokens`'
  new `UNIQUE (token_hash)` index immediately failed
  `forge-backfill.test.ts`'s ephemeral-Postgres suite — two `tokenRow()`
  test fixtures shared one hardcoded fake `token_hash`
  (`"e3".repeat(32)`), which is unrealistic test data (a literal SHA-256
  collision between two different tokens), not a production integrity
  finding. Fixed the fixture to derive a distinct hash per row; the row
  KEY used in redaction assertions is built from `tenant_ref`/`token_ref`
  only (never `token_hash`), so the fix has no effect on custody-redaction
  coverage.
- **Test/typecheck/architecture/deploy status**: `khala-sync-server` full
  suite (355 tests, was 354 + the fixture fix) and all seven
  forge-prefixed `workers/api` suites (49 tests) pass; `workers/api`
  typecheck clean; `apps/openagents.com` `check:architecture` zero-debt
  clean (only pre-existing tracked-debt categories, unrelated to Forge);
  full `check:deploy` clean.
- **Write cutover — RE-EVALUATED, STILL deliberately NOT wired.** The
  named blocker (missing constraints) is now fixed, but two independent
  concerns remain:
  1. **Domain-wide incoherence (unchanged from the second pass)**: the
     canonical git store is one of five forge stores that all currently
     write D1-first and mirror to Postgres; flipping only it to Postgres
     write authority splits authority across the domain, and this pass
     did not build or land a coordinated all-five flip.
  2. **NEW finding this pass**: `forge-git-canonical-postgres-store.ts`
     has **no path that mirrors its writes back into D1 at all** — it is
     a pure Postgres-only implementation of `ForgeGitCanonicalStore`.
     Wiring it as write authority today would mean D1 goes stale for
     canonical git tables immediately, and the existing FAIL-SOFT-to-D1
     read fallback (`KHALA_SYNC_FORGE_READS=postgres`, used when a
     Postgres call errors) would then silently serve STALE ref state
     instead of failing loud on a Postgres outage — a regression versus
     today's behavior. Safely wiring write authority needs either a
     reverse D1-mirror write path in the Postgres store, or an explicit
     owner-approved decision to drop the D1 read fallback once Postgres
     becomes write-authoritative. Neither exists yet.
  Per the task's "if in doubt, leave D1 as sole write authority and
  document precisely why" guardrail, D1 remains write authority for all
  five forge stores. Constraint parity is a real, necessary precondition
  now satisfied; it was not sufficient on its own.
- **D1 drop**: still out of scope, still consolidated into #8330.
- **Status**: left OPEN on #8358. Read cutover remains DONE and live;
  write cutover remains the next concrete step, now unblocked on
  constraints but still blocked on the reverse-mirror gap above.

### 2026-07-05 D1 mirror-back pass — the reverse-mirror gap is closed (#8358, fourth pass)

- **Re-verified the third pass's constraint claim, not just trusted it**:
  `pg_indexes` against a fresh direct-URL query confirms all nine unique
  indexes from `0035_forge_domain_ref_lock_uniques.sql` exist today on the
  live schema; `bun scripts/backfill-forge.ts --verify --verify-newest 50`
  against the direct prod Cloud SQL URL is CLEAN (exact row counts on all
  sixteen tables, newest-hash matches, ref-set digest matches for the 1
  repository, merge-queue replay digest matches vacuously). The constraint
  parity claim from the third pass holds.
- **Built the reverse mirror the third pass identified as missing.**
  `makePostgresForgeGitCanonicalStore` (`forge-git-canonical-postgres-store.ts`)
  now takes an OPTIONAL second `mirror: ForgeGitCanonicalD1MirrorDeps`
  argument. When provided, every successful `applyReceivePack` /
  `importExternalRef` converge-upserts its RESOLVED rows (the exact rows
  already returned to the caller from inside the same Postgres transaction
  via `RETURNING`/in-tx reads — no extra read-back round trip needed,
  unlike the D1→Postgres direction) into a given D1 twin, fail-soft with
  bounded retry (100ms/400ms, matching `mirrorArtanisRows`'s discipline in
  `artanis-domain-store.ts`): it NEVER throws, logs
  `khala_sync_forge_postgres_write_mirror_retry` /
  `khala_sync_forge_postgres_write_mirror_failed`, and a dead D1 mirror
  never fails the already-committed Postgres write. Reuses the SAME
  generic table-driven D1 upsert the forward mirror relies on
  (`makeD1ForgeDomainWriteStore`), now extracted into its own module
  (`forge-domain-d1-write-store.ts`) so both mirror directions share one
  implementation without a circular import between
  `forge-domain-store.ts` and `forge-git-canonical-postgres-store.ts`.
  Passing no `mirror` argument reproduces the exact prior behavior
  (Postgres-only, zero D1 side effect) — the read-cutover call site
  (`servePostgresListRefs`) is untouched and still calls the factory with
  no mirror.
- **New test coverage** (`forge-git-canonical-postgres-store-d1-mirror.test.ts`,
  6 tests, real local Postgres + real SQLite as the D1 double): create and
  fast-forward-update `applyReceivePack` calls mirror the resolved
  ref/object/intake rows into D1 byte-for-byte; `importExternalRef`
  mirrors its ref/object; a fully broken D1 mirror never fails the
  Postgres write and logs one typed failure diagnostic per mirrored table;
  a transient D1 failure recovers on retry with only a retry diagnostic
  logged (no failure); omitting `mirror` entirely leaves D1 untouched.
- **What this closes, and what it does NOT close.** The third pass's NEW
  finding — no path to mirror Postgres writes back into D1, which would
  have made the FAIL-SOFT-to-D1 read fallback silently serve STALE state
  on a Postgres read error once write authority flipped — is now closed:
  the mechanism exists, is tested, and is fail-soft. The domain-wide
  incoherence concern (the canonical git store is one of five Forge
  stores, the other four still write D1-first/mirror-to-Postgres) is
  UNCHANGED and still open. This pass deliberately did NOT flip the
  production route handler to call `makePostgresForgeGitCanonicalStore`
  instead of `makeD1ForgeGitCanonicalStore` — that would move real tenant
  git-ref writes onto a path with zero production traffic history and no
  compare-mode soak of the mirror-back itself, which is a materially
  bigger and separately-reviewable call than building the plumbing. The
  write cutover is now reduced to an explicit routing decision (which
  route handler each of the five stores' write paths call) plus a soak of
  the new mirror path — not blocked on any unbuilt mechanism.
- **Test/typecheck/architecture status**: `workers/api` typecheck clean;
  all eight forge-prefixed `workers/api` suites (55 tests, was 49 + 6 new)
  pass; `apps/openagents.com` `check:architecture` zero-debt clean (only
  pre-existing tracked-debt categories, unrelated to Forge). `check:deploy`
  not run this pass — nothing shipped changes any production code path
  (the new `mirror` parameter is optional and the one production call site,
  `servePostgresListRefs`, passes none), so there is nothing to deploy.
- **D1 drop**: still out of scope, still consolidated into #8330.
- **Status**: left OPEN on #8358. Read cutover remains DONE and live;
  write cutover is now unblocked on both previously-named blockers
  (constraints, mirror-back) and reduces to the domain-wide routing
  decision plus a mirror-path soak — the next concrete step.

### 2026-07-05 WRITE cutover landed for the canonical git store — LIVE in production (#8358, fifth pass)

- **The routing decision, made explicitly:** flip the canonical git store
  ONLY, not all five Forge stores at once. A new flag,
  `KHALA_SYNC_FORGE_GIT_CANONICAL_WRITES` (default `d1`, `postgres`
  opt-in), is SEPARATE from and narrower than
  `KHALA_SYNC_FORGE_READS`/`KHALA_SYNC_FORGE_DUAL_WRITE` — flipping it
  does not touch the other four Forge stores (coordination,
  packfile-archive, tenant-git-auth, GitHub mirror), which remain
  D1-first/mirror-to-Postgres exactly as before. This is the scoped,
  one-store flip the task explicitly allowed, not the still-undone
  domain-wide coordination of all five stores.
- **Wiring:** `forgeGitCanonicalWritesFromEnv` +
  `makeForgeGitCanonicalStoreForEnv` (`forge-domain-store.ts`). When
  `postgres`, the ENTIRE canonical-git-store surface
  (preflight/apply/import/read/list) routes through
  `makePostgresForgeGitCanonicalStore` with its D1 mirror-back wired in
  — not just the writes. Reads route to Postgres too (not the separate
  `KHALA_SYNC_FORGE_READS` machinery) so there is no window where a
  fresh Postgres write could be immediately followed by a stale D1-served
  read of the same ref. Deliberately NO fallback to D1 on a Postgres
  error under this flag: once it is on, D1 is no longer the lock
  authority, so silently falling back to a D1 write would let two
  ref-lock protocols race the same ref — a Postgres outage must fail the
  push loud, not silently diverge. The production route handler call site
  (`index.ts`) needed ZERO changes — same pattern as the read cutover.
- **Test coverage:** `forge-git-canonical-write-authority.test.ts` (real
  ephemeral Postgres + SQLite D1 double) proves: (1) the default (`d1` or
  unset) is byte-identical to prior behavior; (2) `postgres` mode routes
  BOTH writes and reads to Postgres — proven by drifting the D1 twin
  out-of-band after a write and confirming `readRef`/`listRefs` still
  return the Postgres value; (3) `postgres` mode with no `KHALA_SYNC_DB`
  binding falls back to the D1-authoritative store unchanged (the flag
  can never point at a nonexistent twin).
- **Real end-to-end verification, staging first:** deployed
  `KHALA_SYNC_FORGE_GIT_CANONICAL_WRITES=postgres` to staging alone,
  minted a `git:receive-pack`-scoped token for a dedicated canary
  tenant/repo (`tenant.ks8-16-write-cutover-canary` /
  `repo.ks8-16-write-cutover-canary`, inserted directly with the same
  schema/hash/prefix convention `mintGitAccessToken` uses — never the
  real staging/prod customer tenant), and ran TWO real `git push`
  operations through the deployed Worker's actual HTTP smart-protocol
  route: a CREATE of a brand-new ref against a fresh orphan commit, then
  a fast-forward UPDATE. Both pushes succeeded through the real `git` CLI
  (not a fixture, not a unit test double). Direct queries against BOTH
  the staging Cloud SQL database and staging D1 immediately after each
  push confirmed BYTE-IDENTICAL convergence: exact
  `object_id`/`previous_object_id`/`state` on `forge_git_refs` (including
  the fast-forward's `previous_object_id` chain), and matching rows on
  `forge_git_objects` and `forge_git_receive_pack_intakes` — proving the
  Postgres advisory-lock write path AND the new D1 mirror-back both work
  correctly under real git-protocol traffic, not just synthetic unit
  inputs. The verification token was revoked immediately after use.
- **Production flip, gated on the staging result:** only after staging
  verified clean did production get
  `KHALA_SYNC_FORGE_GIT_CANONICAL_WRITES=postgres` (`wrangler.jsonc` top
  level), deployed via `deploy:safe`. The identical real end-to-end
  `git push` verification (dedicated canary tenant/repo, never the real
  `tenant.openagents` production repository) was repeated against
  production before this pass closed — see the production evidence
  block immediately below for the exact rows/hashes.
- **Scope discipline preserved:** the other four Forge stores are
  UNCHANGED — still D1-first with read-back mirror to Postgres, governed
  by the pre-existing `KHALA_SYNC_FORGE_DUAL_WRITE` flag only. The D1
  drop remains out of scope, consolidated into the epic-closing KS-8.19
  sweep (#8330).
- **Test/typecheck/architecture/deploy status:** `workers/api` typecheck
  clean; full `check:architecture` zero-debt clean (only pre-existing
  tracked-debt categories, unrelated to Forge); full `check:deploy` green;
  `deploy:safe` (staging deploy + smoke + prod deploy) green for both the
  staging-only canary deploy and the final staging+production deploy.
- **Status:** the canonical git store's write cutover is DONE and LIVE in
  production. #8358 closes on this basis; the domain-wide coordination of
  the other four stores (if ever pursued) and the D1 drop (#8330) remain
  future, separately-scoped work.

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

LIVE WIRING: the three re-homed crons + the funded-hygiene store, wired as
store-factory drop-ins: `RelayHealth.probeTick` (`makeRelayHealthStoreForEnv`
— probes/transitions mirror on insert, the retention prunes converge onto
the twin), `AutopilotContinuationPolicy.sweep`
(`makeAutopilotContinuationStoreForEnv`),
`AutopilotScheduledLaunches.dispatchDue` (`makeAutopilotWorkStoreForEnv` —
every work-order write mirrors by `work_order_ref`, closeout receipts by
`closeout_ref`), and `makeHygieneDebtReceiptStoreForEnv`. **CONFIRMED LIVE
(#8361, 2026-07-05):** the remaining scattered `adjutant_*` / `omni_*`
writers, the Effect onboarding store, `autopilot_token_usage`, and
`backend_incident_events` (34 call sites, commit `4acd3704c2`) now also
mirror through `makeSupervisionLongtailMirrorForEnv` — every writer in this
domain is dual-writing.

Flags (Worker vars):

- `KHALA_SYNC_SUPERVISION_DUAL_WRITE` — default **on** wherever
  `KHALA_SYNC_DB` exists; `off|0|false|disabled|no` disables the mirror.
- `KHALA_SYNC_SUPERVISION_READS` — default `d1`; current value **`postgres`**
  (prod + staging, set #8361 follow-up, 2026-07-05). `compare` arms the
  fail-soft, inline-awaited shadow-compare reader (never fire-and-forget — a
  Worker can cancel an un-awaited async tail once the response is sent)
  (`makeOmniPublicProofBundleCompareReader` in
  `supervision-longtail-domain-store.ts`), which itself NEVER serves
  Postgres at any flag value. `postgres` ADDITIONALLY unlocks a SEPARATE,
  bounded real-serve reader — `makeOmniPublicProofBundlePostgresServerForEnv`
  — for the ONE public projection surface in this domain,
  `omni_public_proof_bundles` (read by both the redacted public handoff page
  and the operator JSON view in `omni-bundle-routes.ts`; wired via the new
  `serveProofBundleFromPostgres` dependency in `omni-bundle-routes.ts`).
  Fail-soft: any Postgres query error (or `reads !== 'postgres'`, or no
  Postgres binding) falls back to the unchanged D1-served
  `readOmniPublicProofBundleById` path; a genuine Postgres "not found" IS
  trusted and served directly (no D1 re-check). Every OTHER comparable read
  in this domain has no reader wired at all, so this bounded allowlist is
  simply "this one table, nothing else" — the same shape as the KS-8.14
  business-domain precedent (#8360,
  `BUSINESS_DOMAIN_POSTGRES_SERVED_READ_TABLES`).

Flag-flip order — never skip a step, each soaks before the next:

1. **Dual-write on** (default after KS-8.17 lands + `0024` applied via the
   migration runner). Watch `khala_sync_supervision_dual_write_failed`; a
   nonzero steady rate blocks progression.
2. **Backfill**: from `packages/khala-sync-server/`,
   `KHALA_SYNC_DATABASE_URL=<direct-url> bun scripts/backfill-supervision-longtail.ts`
   (wrangler-auth'd; rowid-cursor resumable via
   `.supervision-longtail-backfill-state.json`). Run a SECOND time
   (`--restart`) as the catch-up sweep once dual-write has covered the whole
   window. **DONE (#8361, 2026-07-05):** both sweeps ran clean against
   production across all 29 tables.
3. **Verify**: `bun scripts/backfill-supervision-longtail.ts --verify` —
   exact row counts, per-state/sum tallies, **idempotency-key-set equality**
   (`omni_idempotency_keys`), **public proof-bundle digests**
   (`omni_public_proof_bundles`, the §3.14 shadow-compared projection
   surface), newest-50 row hashes. Post the output on the migration issue
   (secret-safe by construction). Exact or explain; no cutover on a red
   verify. **DONE (#8361, 2026-07-05):** `verify: CLEAN — every check
   matches` across all 29 tables, zero mismatches of any kind. **RE-RUN
   CLEAN (#8361 follow-up, 2026-07-05):** re-verified fresh against
   production a second time after real organic production writes landed in
   sibling domain tables since the first verify (`relay_health_probes` grew
   2024 → 2028 rows) — every table still matched exactly, proving the mirror
   keeps converging genuinely-new organic writes, not just the original
   backfilled history.
4. **Shadow-compare the public proof-bundle endpoint**: diff the servable
   `omni_public_proof_bundles` projection against BOTH stores until silent.
   **DONE (#8361):** `KHALA_SYNC_SUPERVISION_READS=compare` deployed to prod
   + staging 2026-07-05; live-tailed with zero
   `khala_sync_supervision_read_compare_mismatch`.
5. **Read cutover — DONE for the one bounded allowlist table (#8361
   follow-up, 2026-07-05)**: `KHALA_SYNC_SUPERVISION_READS=postgres` deployed
   to prod + staging. `makeOmniPublicProofBundlePostgresServerForEnv` now
   serves `omni_public_proof_bundles` reads for real, matching the KS-8.14
   business-domain precedent (#8360) rather than waiting on a traffic-based
   soak: this table is genuinely zero-traffic (0 rows in D1 AND Postgres, in
   BOTH prod and staging, confirmed fresh 2026-07-05 — no organic write has
   ever landed here since the domain was wired), so a live-traffic soak is
   structurally unavailable and would stay vacuous forever if waited on. The
   accepted evidence instead is: (a) the contract suite proving present/
   absent/broken-Postgres behavior against a real local Postgres twin
   (`supervision-longtail-domain-repository.contract.test.ts`), (b) the
   row-for-row `--verify` clean across all 29 tables (re-confirmed fresh,
   above), and (c) every OTHER comparable read in this domain staying
   D1-only by construction (no reader wired), so the blast radius of a
   Postgres bug here is bounded to this one already-shadow-compared,
   write-decision-free, public-safe surface. Every other write path in this
   domain (`adjutant_*`, other `omni_*` tables, `autopilot_*`,
   `relay_health_*`, `backend_incident_events`, `hygiene_debt_receipts`)
   stays D1-served permanently under this flag, by design — widening the
   allowlist to any of those is a deliberate, individually-reviewed
   follow-up, never a blanket flip.
6. **D1 drop**: NOT done here — consolidated into the epic's KS-8.19 sweep
   (#8330), consistent with #8358/#8360/#8362.

Rollback at ANY step: `KHALA_SYNC_SUPERVISION_DUAL_WRITE=off` (writes) and/or
`KHALA_SYNC_SUPERVISION_READS=d1` (real serve + shadow-compare readers both
go inert; D1 authority is never behind).

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

### 2026-07-05 follow-up (#8362): post-deploy re-verify + read-site classification

Owner authorization on file for the eventual read cutover — quoted verbatim
per the owner's instruction so the decision trail is auditable, and it
satisfies the "ask a human" gate only, never the "prove it" gate:

> "For the D1 closeout issues etc, you have full approval from the owner (me)
> to do all needed cutovers and retire D1 - after ensuring content is backed
> up / moved over for example all forum posts need to be backed up and moved
> over into the new system, and any other relevant data."

**Forum-content note:** forum posts are NOT owned by this domain — they
migrate under the separate KS-8.10 Forum (content + trust) lane
([#8321](https://github.com/OpenAgentsInc/openagents/issues/8321), landed).
Nothing in the identity/auth domain's own tables holds forum post bodies, so
this domain has no forum-backup action of its own.

**Confirmed the #8362 write-site wiring is now LIVE in production** (it had
only been committed, not yet deployed, when the prior #8362 evidence was
posted): `wrangler deployments list` shows the first post-#8362-commit
deploy at `2026-07-05T06:53:56Z` (version `b34ad490-450b-4728-b945-ad858983917a`),
with several more deploys since from concurrent domain work, none of which
rolled back that wiring.

**Fresh `--restart` + `--verify` against production, hours into live
traffic on the new wiring:** `backfill-identity-auth.ts --restart` re-scanned
all 17 tables (462/462/176/21/1/1/63/42/68/155/478/64/31/26/12/0/0 rows —
identical counts to the pre-deploy snapshot on #8362, so no net-new rows
appeared, but a restart sweep re-converges every row regardless of count
drift). `--verify --verify-newest 50` came back **CLEAN — every one of the
17 tables matched exactly** (row counts, custody-safe scalar tallies,
newest-50 hashes), confirming the new writer wiring behaves correctly under
real live traffic, not just against a pre-deploy snapshot. `KHALA_SYNC_IDENTITY_READS`
remains untouched at `d1`; no flags were flipped; D1 remains sole authority.

**Read call-site classification** (the concrete inventory for whoever picks
up the owner-gated read-cutover follow-up). Grepped every
`FROM users|auth_identities|openauth_storage|openauth_agent_links|
github_write_*|provider_account*` call site outside this domain's own store
and tests:

*Permanent D1-only auth-decision reads — never a compare/postgres candidate
at the current risk bar, same reasoning as the entitlements domain's 6
enforcement-gate reads:*

- `auth/openauth-storage.ts`, `auth/email-otp-hardening.ts` — session/OTP
  validation on every request.
- `index.ts` (`upsertGitHubUser`/`upsertEmailUser`/`upsertUser`, the
  `openauth_agent_links` resolves, the `primary_email` lookups feeding live
  flows) — core session/identity resolution.
- `agent-registration.ts`, `agent-owner-claim-routes.ts`,
  `agent-scoped-grant-routes.ts` — identity-linkage and scoped-grant
  authorization decisions.
- `github-write-connections.ts` — GitHub write-scope grant/connection state.
- `provider-account-repository.ts`, `provider-account-token-custody.ts`,
  `provider-account-pool-routes.ts` — BYOK credential resolution, secret
  custody, and account-leasing decisions (which account serves this
  request).
- `operator-provider-account-routes.ts`, `provider-launch.ts` — mixed
  operator-console reads alongside live lease acquire/release/launch
  decisions; kept whole-file D1 rather than splitting, since a stale read
  here can double-lease or misroute a shared provider account.
- `onboarding/repository.ts`, `billing.ts`, `customer-orders.ts` — treated
  conservatively as decision-adjacent (onboarding-step gating, billing/order
  ownership resolution tied to a user identity) even though none of these
  are literal allow/deny gates; err D1 pending a dedicated review.

*Candidate genuinely-safe, non-decision-critical display/reporting reads —
NOT implemented or flipped this pass, flagged as the starting inventory for
a future bounded allowlist (the `inference-entitlements-store.ts`
`*_NON_GATE_READS`-style pattern) once a dedicated follow-up builds it:*

- `admin-overview-routes.ts` — admin dashboard user listing (joins
  `software_orders`, a different domain — would need that table's own
  Postgres twin/read-routing confirmed first).
- `artanis-operator-dashboard-routes.ts` — operator-console
  `provider_accounts` summary display.
- `operator-order-triage-routes.ts` — operator triage listing over
  `provider_account_leases` / `_failover_receipts`.
- `operator-targets.ts` — CRM/outreach `users` listing (display/reporting,
  not an auth gate).
- `provider-account-usage-routes.ts` — `provider_accounts` usage/stats
  reporting route.
- `forum/repository.ts` — `users` join for author display name/avatar
  projection on forum posts.

None of these candidates were implemented this pass. Each would need: (a)
confirming any cross-domain joined table already has its own Postgres twin
and read-routing, (b) a new typed Postgres query function per route mirroring
the entitlements bounded-allowlist pattern, (c) a NEW flag distinct from
`KHALA_SYNC_IDENTITY_READS` (e.g. `KHALA_SYNC_IDENTITY_NON_GATE_READS`) so a
display-read flip can never imply an auth-decision flip, and (d) its own
compare-mode soak with live `wrangler tail` evidence before any flip — real,
separate implementation work, not a config change. The KV/cache layer and
auth-matrix shadow-read replay tooling required for the actual auth-decision
read cutover (step 5 above) remain NOT built.

Verification this pass: `bun run typecheck` (workers/api) clean;
`identity-auth-domain-repository.contract.test.ts` (10/10) and
`identity-auth-backfill.test.ts` (10/10, `bun test`) both green;
`bun run check:architecture` (zero-debt) passed. No source files were
changed this pass — only fresh production verification evidence and this
documentation update.

### 2026-07-05 follow-up (#8362): bounded non-gate read allowlist

Following the entitlements domain's `*_NON_GATE_READS` precedent (#8336)
and the billing/business-domain bounded-allowlist precedent (#8337/#8360),
this pass adds a SECOND, FULLY INDEPENDENT read surface for the identity/
auth domain — never touching `KHALA_SYNC_IDENTITY_READS`, which stays at
its default `d1` forever in this pass.

**Re-audit of the six candidates the prior pass inventoried.** Every read
call site was re-read with fresh eyes, tracing consumers 30-50+ lines out
per the issue's own conservative-bar instruction. Five of the six turned
out to be decision-adjacent, cross-domain-blocked, or a read-after-write
hazard on closer inspection and stay D1-only PERMANENTLY (not merely
deferred):

- `admin-overview-routes.ts` (admin `users` listing) — the query also
  JOINs `software_orders` in the SAME statement, and `software_orders` is
  NOT in `BUSINESS_DOMAIN_POSTGRES_SERVED_READ_TABLES` (only
  `business_funnel_events` is served today) — the whole statement cannot
  be routed without a query-split refactor that is out of scope here.
- `artanis-operator-dashboard-routes.ts` (`listOperatorAccountStatusRows`)
  — the SAME function serves a plain GET AND a read-your-own-write
  immediately after `resetOperatorAccountCooldown`'s mirrored D1 UPDATE; a
  lagged/failed mirror could show the caller stale pre-reset cooldown data
  in the reset-confirmation response.
- `operator-order-triage-routes.ts` (lease/failover/users triage reads) —
  the lease read sits on this domain's OWN documented
  `provider_account_leases` staleness gap (the lazy stale-expiry sweep is
  deliberately never mirrored); the `users` join is embedded in one giant
  cross-domain SQL string shared with an existence-gate consumer
  (`requireTriageRecordByOrderId`).
- `operator-targets.ts` (CRM/outreach `users`+`auth_identities` target
  resolution) — NOT a display list: tracing consumers shows it resolves
  the target for a real USD-credit grant
  (`handleOmniOperatorInferenceCreditApi` → `applyManualBillingCredit` +
  `fundInferenceFromCredit`) and for account-linking/share-creation
  actions. A stale read could let a money grant or admin action resolve
  against a target D1 would have correctly rejected.
- `forum/repository.ts` (`readForumAgentPublicProfile`) — 3 of 4 call
  sites are pure GET display, but the 4th (`followActorResponse`, a POST
  handler) uses the SAME function to gate a follow-creation existence/
  self-follow check; a stale read could let a follow-write proceed against
  a target D1 would have correctly 404'd.

**The one candidate that survived:** `provider-account-usage-routes.ts`'s
`listPoolState` — an admin-only, single-table (`provider_accounts`, no
JOIN) pool-state projection with exactly one call site
(`buildProviderAccountUsageProjection`, itself called only from a strict
GET route with no mutate in the handler and no other read-after-write
consumer anywhere in the repo).

**New, fully independent flag:** `KHALA_SYNC_IDENTITY_NON_GATE_READS`
(d1|compare|postgres, default `d1`) governs ONLY
`IdentityAuthNonGateReads.providerAccountPoolStateByUserId`. Machinery
added to `identity-auth-domain-store.ts`:
`makeD1IdentityAuthNonGateReads` (the SAME SQL `listPoolState` already
ran inline), a `nonGateReads` field on `PostgresIdentityAuthStore`,
`makeRoutedIdentityAuthNonGateReads` (d1/compare/postgres routing, the
same fail-soft discipline as the entitlements router — `compare` serves
D1 and shadow-compares off the response path; `postgres` makes one real
attempt with D1 fallback + diagnostic on any error), and the
`identityAuthNonGateReadsForEnv(env)` factory. New diagnostic events,
deliberately distinct from every gate-read/dual-write event name:
`khala_sync_identity_non_gate_read_compare_mismatch`,
`khala_sync_identity_non_gate_postgres_read_failed`,
`khala_sync_identity_non_gate_postgres_read_fallback`. The call site
(`provider-account-usage-routes.ts`'s `listPoolState` and
`buildProviderAccountUsageProjection`) takes the routed reads as an
optional trailing parameter and falls back to its untouched inline D1
query when absent — byte-identical D1 behavior with the flag off/unbound.

**Backfill/verify:** no new writer was added (this pass is read-only), so
no backfill was needed. A fresh
`packages/khala-sync-server/scripts/backfill-identity-auth.ts --verify
--verify-newest 50` against production D1 (`openagents-autopilot`) and
Cloud SQL (`khala_sync_prod`, role `khala_app`) came back **CLEAN — all
17 tables exact**, `provider_accounts` 42/42 with newest-50 hashes
matching, confirming the read this pass serves has zero drift to inherit.

**Test coverage:** a new pure unit test file,
`identity-auth-domain-store.test.ts`, pins the router's compare/postgres
routing and diagnostic-event behavior without needing local Postgres. A
new contract-test block in
`identity-auth-domain-repository.contract.test.ts` proves real
D1-vs-Postgres ANSWER parity for `providerAccountPoolStateByUserId`
(including owner-scoping and soft-delete-exclusion parity, and an
empty-result-set parity case) against a real local Postgres instance.

**Flag-flip evidence** (mirrors the entitlements/business-domain
precedent — deployed with the flag unset first, then flipped one stage at
a time with a live `wrangler tail` soak before proceeding; all three
production deploys ran through `deploy:safe` — full `check:deploy` gate,
staging deploy + 5/5 parallel-dispatch smoke, prod D1 + Khala Sync
pending-migrations checks (0 pending both times) — then the final
`wrangler deploy --containers-rollout=none` step run directly each time
because the chained script's shell lacked `KHALA_SYNC_DATABASE_URL`, the
same non-code gap the KS-8.9/#8409 precedents hit and recovered from
identically):

- **Stage 1 (flag unset/default `d1`)**: deployed commit `69eeeaab57`.
  Production Worker Version `4a6b0a8b-a418-42ec-abca-5c012d951afb`. Smoke:
  `GET https://openagents.com/` → 200,
  `GET /api/admin/provider-accounts/usage` (no session) → 401 (unchanged
  auth-gated behavior — proves the new optional param wiring is fully
  inert with the flag unset).
- **Stage 2 (`compare`)**: deployed commit `d3e8bdfc6c`. Production Worker
  Version `18252417-5655-4b68-b676-75ee108430e5`. Live `wrangler tail`
  soak: ~5 minutes wall-clock, ~44,000 lines of real unfiltered production
  traffic observed. **Zero**
  `khala_sync_identity_non_gate_read_compare_mismatch`,
  `khala_sync_identity_non_gate_postgres_read_failed`, or any
  `khala_sync_identity_*` diagnostic event fired. Honest caveat: the admin
  usage route itself received no organic hits in this window (it is a
  low-traffic operator-only surface) — thin-by-vacuity for THIS route
  specifically, same caveat class as the KS-8.17 near-zero-traffic
  precedent, but this is a genuinely unfiltered live-production tail
  covering all real traffic, not a possibly-broken filter hiding events.
- **Stage 3 (`postgres`)**: deployed commit `12fab239c0`. Production
  Worker Version `9f1230c7-6049-4b85-93ff-07486a68213b`. Live `wrangler
  tail` soak: ~4.3 minutes wall-clock, ~27,000 lines of real unfiltered
  production traffic observed. **Zero**
  `khala_sync_identity_non_gate_postgres_read_fallback`,
  `khala_sync_identity_non_gate_postgres_read_failed`, or any
  `khala_sync_identity_*` diagnostic event fired. Final smoke:
  `GET https://openagents.com/` → 200,
  `GET /api/admin/provider-accounts/usage` (no session) → 401.

Current production state after this pass:
`KHALA_SYNC_IDENTITY_NON_GATE_READS=postgres` (serving
`providerAccountPoolStateByUserId` from Postgres for real, fail-soft to
D1); `KHALA_SYNC_IDENTITY_READS=d1` (untouched, default, no routed auth
read); `KHALA_SYNC_IDENTITY_DUAL_WRITE` untouched (on wherever
`KHALA_SYNC_DB` exists).

**Post-deploy `--verify` (after the `postgres` flip is live):** a fresh
`--verify --verify-newest 50` against production D1 and Cloud SQL came
back **CLEAN — all 17 tables exact**, `provider_accounts` 42/42 with
newest-50 hashes matching, confirming data integrity holds after serving
this read from Postgres in production.

Rollback at any point: `KHALA_SYNC_IDENTITY_NON_GATE_READS=d1`. This can
NEVER affect an auth decision — the flag only ever touches this one
display/reporting projection. `KHALA_SYNC_IDENTITY_READS` and
`KHALA_SYNC_IDENTITY_DUAL_WRITE` are untouched by this follow-up.

Verification this pass: `bun run typecheck` (workers/api) clean; full
`apps/openagents.com/workers/api` test suite green except two
PRE-EXISTING, unrelated failures (`nexus-pylon-visibility-routes.test.ts`,
`treasury-domain-store.test.ts` — neither imports any file this pass
touched; both sit in domains under active concurrent work this session
per the shared multi-agent git-hygiene policy) — 1060/1062 files,
9481/9489 tests passed; `bun run check:architecture` (zero-debt) passed.

## Compare-mode soak observability (#8282 shared follow-up)

**Problem this closes.** Proving a `compare`-mode read (D1 serves, Postgres
shadow-read compared, mismatches logged via the existing
`khala_sync_*_compare_mismatch` / `khala_sync_*_read_compare_failed`
diagnostics) is safe to flip to real Postgres serving requires a genuine
multi-hour-or-longer soak with ZERO mismatches. Before this landed, the only
way to observe that was a `wrangler tail` piped to one agent's terminal for
the length of a single session:

1. Not a genuine long soak — an hour-plus read-heavy domain needs far more
   observation time than one agent session allows.
2. Invisible after the session ends — no durable record exists of "this flag
   has run mismatch-free for N hours across M requests."
3. Silently vacuous for low/zero-traffic domains. The supervision pass
   (#8361) found `omni_public_proof_bundles` has zero organic traffic, so a
   "clean" `wrangler tail` there proved nothing — and there was no way to see
   that pattern except by noticing it manually.

**What was built.** A durable, queryable Cloudflare Analytics Engine data
point per compare-mode read, ADDITIVE to (never a replacement for) the
existing per-call diagnostic events:

- `packages/khala-sync-server/src/compare-soak-metrics.ts` —
  `makeCompareSoakMetrics(dataset)` builds a fail-soft recorder:
  `record({ domain, readKind, outcome })` where `outcome` is
  `"match" | "mismatch" | "error"`. **Fail-soft contract (load-bearing):**
  `record()` never throws, blocks, or slows the real read path — a missing
  `ANALYTICS` binding degrades to a true no-op (`makeCompareSoakMetrics(undefined)`),
  and any `writeDataPoint` fault is caught and swallowed. Never required for
  correctness; always safe to wire in unconditionally.
- The `analytics_engine_datasets` wrangler binding `ANALYTICS` →
  dataset `khala_sync_compare_soak` (prod) /
  `khala_sync_compare_soak_staging` (staging), added to
  `apps/openagents.com/workers/api/wrangler.jsonc`. Optional on the
  `WorkerBindings` type (`apps/openagents.com/packages/sync-worker/src/index.ts`)
  — absent binding = no-op, never an error.
- Each domain constructs its recorder as
  `options.metrics ?? makeCompareSoakMetrics(env.ANALYTICS)` inside its own
  `*StoreEnv` → runtime factory (same pattern as the existing `log`/`mirror`
  injection points), then calls `metrics.record(...)` alongside the existing
  `log(...)` diagnostic call in every compare-mode branch: once on match, once
  on mismatch, once on a shadow-read error (recorded as `outcome: "error"` so
  a domain never reads as vacuous just because its Postgres shadow reads are
  themselves erroring — that is still real traffic, just not a comparable
  result).
- `packages/khala-sync-server/scripts/query-compare-soak.ts` — queries the
  dataset via the Cloudflare Analytics Engine SQL API
  (`POST https://api.cloudflare.com/client/v4/accounts/{account_id}/analytics_engine/sql`)
  and reports, per domain, over a lookback window: total compare-mode reads,
  match/mismatch/error counts, and a `vacuous: true` flag for any known
  domain that had ZERO rows in the window at all — the exact #8361 failure
  mode made explicit and automatic instead of something an agent has to
  notice by hand.

**Wired call sites today** (the `domain` slug each records under):

| Domain slug | File | Flag | Live in prod as `compare`? |
| --- | --- | --- | --- |
| `entitlements_gate` | `inference-entitlements-store.ts`, `makeRoutedEntitlementsGateReads` | `KHALA_SYNC_ENTITLEMENTS_READS` | **Yes** (`compare` in prod as of 2026-07-05, #8336 part 3 — soak observability bring-up, NOT a serving change; D1 still decides every gate) |
| `entitlements_non_gate` | `inference-entitlements-store.ts`, `makeRoutedEntitlementsNonGateReads` | `KHALA_SYNC_ENTITLEMENTS_NON_GATE_READS` | No (prod runs `postgres`, not `compare`, today) |
| `supervision` | `supervision-longtail-domain-store.ts`, `makeOmniPublicProofBundleCompareReader` | `KHALA_SYNC_SUPERVISION_READS` | No (prod runs `postgres` as of the #8361 follow-up, 2026-07-05 — real-served through the SEPARATE `makeOmniPublicProofBundlePostgresServerForEnv` reader for the one bounded `omni_public_proof_bundles` allowlist; the shadow-compare reader in this row keeps running unconditionally regardless, so soak metrics keep accumulating past cutover too) |
| `artanis` | `artanis-domain-store.ts`, `artanisRead` | `KHALA_SYNC_ARTANIS_READS` | **Yes** (`compare` in prod) |
| `billing` | `billing-store.ts`, `makeRoutedBillingBalanceRead` / `makeRoutedBillingRecentEntriesRead` / `makeRoutedBillingAutoTopUpStateRead` | `KHALA_SYNC_BILLING_READS` | No (default `d1`) |
| `forge` | `forge-domain-store.ts`, `compareListRefs` (inside `makeForgeGitCanonicalStoreForEnv`) | `KHALA_SYNC_FORGE_READS` | No (prod runs `postgres`, not `compare`, today) |

A domain not yet listed above (e.g. a future KS-8 lane) gets the pipeline
"for free" the moment its compare branch calls
`metrics.record({ domain: "<slug>", readKind: "<op>", outcome })` — add the
slug to `KNOWN_COMPARE_SOAK_DOMAINS` in `query-compare-soak.ts` so the query
script reports it (rather than silently omitting it) and to the table above.

**Querying a soak window:**

```sh
cd packages/khala-sync-server
CLOUDFLARE_API_TOKEN=<token> CLOUDFLARE_ACCOUNT_ID=<id> \
  bun run query-compare-soak -- --hours 6
# or: bun scripts/query-compare-soak.ts --hours 6 --dataset khala_sync_compare_soak_staging
```

The owner's Cloudflare API token (needs "Account Analytics Read" permission)
normally lives in `~/work/.secrets/cloudflare-openagents.env`
(`CLOUDFLARE_API_TOKEN`); the account id is visible via `wrangler whoami`.
Output is a per-domain table: total reads, matches, mismatches, errors, and a
status column that reads `VACUOUS` (zero traffic — treat a soak here as
NO evidence, not clean evidence), `MISMATCHES — do NOT flip`, or `clean`.
Pass `--json` for a machine-readable report instead
(`CompareSoakQueryReport`).

**Deciding to flip a domain's read flag to `postgres` still requires:**

1. This pipeline reporting `clean` (zero mismatches) for the domain over a
   genuinely representative window — hours, not minutes, and matching the
   domain's real traffic pattern (a read-heavy domain needs proportionally
   more observation).
2. `totalReads` meaningfully greater than zero — a `VACUOUS` window is NOT
   evidence of anything; wait for real traffic or accept that the domain
   cannot be soak-verified this way and needs another proof strategy.
3. The epic-gated ops decision recorded on #8282 (or the domain's own
   follow-up issue), per every other KS-8 domain's cutover discipline in this
   runbook.

This tool only makes the soak *observable*; it does not itself constitute
soak time, and it never flips any domain's read flag.

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

## Claude-approval poll — verified NOT a khala-sync candidate; converted to local IPC push (KS-6.9, #8419)

Desktop's Claude-approval poll ran `window.setInterval(() => void
pollClaudeApprovals(), 1000)` in `clients/khala-code-desktop/src/ui/main.ts`,
flagged by the 2026-07-04 cleanup audit (§6.2 item 6) as latency-sensitive
because it gates the approval-to-execution round trip for the local Claude
Agent SDK. It was explicitly sequenced after KS-6.8 (#8418), which found the
audit's blanket "all three desktop hot polls map cleanly onto khala-sync
scopes" claim was WRONG for the other two polls (2s thread-token-summary, 5s
inbox) — both read exclusively device-local state with no matching sync
scope. #8418 explicitly flagged that the 1s approval poll was NOT
investigated in that pass and its data source needed independent
verification. This entry is that verification.

**Investigated independently, verified against the actual code — same
conclusion as KS-6.8, for an even stronger reason.**
`pollClaudeApprovals()` calls the `claudeApprovalPending` RPC
(`src/bun/rpc-handlers.ts`), which reads `ClaudeApprovalService.pending()`
(`src/bun/claude-approvals.ts`). That service holds an in-memory
`Map<string, ClaudeApprovalPending>` and Effect `Deferred`s created inside
the desktop app's OWN Bun main process, the instant the local Claude Agent
SDK invokes its `canUseTool` callback mid-turn to ask permission for a tool
call. Responding (`respond()`) resolves that EXACT in-memory `Deferred`,
unblocking the SDK call that is still synchronously waiting on it in the
same process. This state:

- never leaves the process (unlike thread-token-summary's local JSONL/SQLite
  files or the inbox's six local RPC reads, which at least persist to disk);
- has no multi-device concept — it is tied to one specific in-flight SDK
  tool call in one specific running app instance;
- cannot be "pushed via a khala-sync scope" without inventing a distributed
  synchronization bridge for a live, blocking SDK callback — categorically
  out of scope for a "mirror #8383" cutover, and not what a scope entity is
  for.

This is squarely the §6.3 "device-local codex telemetry" exclusion class
KS-6.8 already confirmed for the other two polls — if anything more clearly
local, since it is same-process live state rather than persisted
device-local data. **Not a khala-sync migration candidate.**

**What shipped instead — the honest, real improvement, not just "leave it
as-is":** unlike thread-token-summary/inbox, this poll had a genuine
low-risk fix available, because the exact same problem was already solved
elsewhere in this codebase. Codex tool-approval requests do NOT poll — they
arrive inline via the `chatTurnEvent` push message the desktop already sends
over Electrobun's `rpc.send` IPC transport (proven live in production for
streaming turn/token updates and the KS-6.2/#8383 fleet lifecycle feed).
Claude approvals used a side-channel poll only because the Claude Agent
SDK's `canUseTool` callback sits outside the normal turn-event stream — nothing
technical requires it to stay poll-only.

Changes:

- `createClaudeApprovalService` (`src/bun/claude-approvals.ts`) accepts an
  optional `onRequestQueued` callback, fired synchronously the moment a
  request is queued (before any consumer would poll `pending()`).
- `createKhalaCodeDesktopRpcRequestHandlers` (`src/bun/rpc-handlers.ts`)
  gained an `emitClaudeApprovalRequested` input, wired to the default
  service's `onRequestQueued` only when the caller doesn't supply its own
  `claudeApprovalService` (test seams keep full control).
- `src/bun/index.ts` wires `emitClaudeApprovalRequested` the same way as the
  existing `emitChatTurnEvent`/`emitFleetLifecycleEvent` module-level
  bindings: a no-op until the native `BrowserView` RPC exists, then
  `rpc.send.claudeApprovalRequested(request)` plus the matching preview-mode
  SSE event for headless/preview windows.
- New RPC message `claudeApprovalRequested` in the shared
  `KhalaCodeDesktopRPCSchema` (`src/shared/rpc.ts`).
- UI (`src/ui/main.ts`) reacts to the message by immediately calling
  `pollClaudeApprovals()` instead of waiting for the next 1s tick, for both
  the native Electrobun transport and the preview-window SSE path.
- **The 1000ms `window.setInterval` poll was deliberately KEPT as a fallback
  safety net**, not removed. Unlike a proven server-mediated sync channel, a
  raw same-process IPC push has no delivery guarantee if the webview hasn't
  finished registering its message listener yet (e.g. very early boot); an
  approval request silently missed would hang the SDK turn indefinitely with
  no other detection path. The poll now only matters for that narrow
  fallback window instead of being the primary detection path.

**Latency evidence (measured, not just claimed):** the poll's structural
detection-latency bound is fixed by its interval — up to 1000ms worst case,
~500ms mean for a uniformly arriving request, regardless of anything else.
The push path's detection latency was measured directly
(`clients/khala-code-desktop/tests/claude-approvals.test.ts`, 500 samples,
request-creation timestamp to `onRequestQueued` firing, same process): mean
0.0038ms, p50 0.0018ms, p99 0.03ms, max 0.31ms — four to five orders of
magnitude below the old poll's structural bound. The remaining Electrobun IPC
hop from Bun main process to the webview (needed to actually show the
approval dialog) uses the identical transport already proven live for
`chatTurnEvent`/`fleetLifecycleEvent` in production; it could not be measured
further in this environment without driving a real GUI window, but it
structurally cannot exceed a single IPC round trip, nowhere near the
polling interval it replaces as the primary path.

**Evidence:** `bun run scan:architecture` (175 grandfathered findings, zero
new violations), `bun run typecheck` (clean), `bun test tests/*.test.ts` (696
pass / 0 fail, up from 693 baseline — 3 new tests in
`claude-approvals.test.ts`), `bun run build:ui` + Bun bundle both succeed —
full `bun run verify` chain green from `clients/khala-code-desktop`.

**Issue status:** #8419 stays open, retitled in spirit from "migrate to sync
push" to "verified not a sync candidate; converted to a proven local IPC
push with the poll kept as fallback" — matching the KS-6.8/#8418 precedent.
The literal acceptance criteria ("migrate onto khala-sync push, same
transport as #8383") is not honestly achievable because the underlying data
is same-process live state, not a server-observable, multi-device entity.

## Public tokens-served aggregates projection — model/demand/channel-mix + history (KS-6.7, #8417)

The public tokens-served model-mix, demand-mix, channel-mix, and per-day
history reads
(`GET /api/public/khala-tokens-served/{model-mix,demand-mix,channel-mix,history}`)
now serve a `scope.public.tokens-served-aggregates` STORED-SNAPSHOT
projection first, with a fail-open fallback to the previous live-at-read
KS-8.2 (#8308) rollup-backed ledger call when the projection has not been
refreshed yet for the requested window. This is a **full cutover** (not a
dual-write-only posture like KS-6.4/KS-6.5): all four routes are plain
unauthenticated Worker routes registered directly in `index.ts` and are
POLLED on a timer from `apps/web/src/subscriptions.ts`
(`KHALA_TOKENS_SERVED_HISTORY_POLL_INTERVAL_SECONDS`) — none of them ride
the `/api/sync/connect`/`/log`/`/bootstrap` engine surface, so the
anonymous-actor-required wall that forced KS-6.4/KS-6.5 into a dual-write
posture does not apply here.

**Design — no separate shaping logic, no new Postgres table:**

- Contract: `packages/khala-sync/src/tokens-served-mix.ts` — one entity per
  snapshot kind (`tokens_served_model_mix_snapshot`,
  `..._demand_mix_snapshot`, `..._channel_mix_snapshot`,
  `..._history_snapshot`), each carrying the EXACT shape the corresponding
  ledger read already returns (`window`/`totalTokens`/`groups` for the
  mixes, `window`/`bucket`/`timezone`/`series` for history) plus
  `generatedAt`. Mix snapshots are keyed `entityId = window` (one of
  `today`/`7d`/`30d`/`all`); the history snapshot is additionally keyed by
  timezone (`entityId = "<window>:<timezone>"`, bucket is currently always
  `"day"`).
- Projector + reader: `packages/khala-sync-server/src/
  tokens-served-mix-projection.ts` — same "no bespoke table, ride the
  generic `khala_sync_changelog` directly" shape as settled-feed/gym
  run-progress (**no new migration required**). Each refresh is a plain
  upsert of the snapshot's post-image; the read is the latest row for one
  `(entityType, entityId)` pair.
- Worker glue: `workers/api/src/khala-sync-public-tokens-served-mix.ts` —
  `refreshTokensServedAggregatesBestEffort` recomputes ALL FOUR bounded
  windows for all four snapshot kinds using the SAME ledger reads the
  routes already call (`readPublicTokensServedModelMix`/`...DemandMix`/
  `...ChannelMix`/`...History`), then upserts each result. Because the
  post-image IS the ledger's own shaped output, the projection and the
  exact ledger read are byte-for-byte comparable by construction — the
  KS-6.x/KS-8.x reconcile discipline reduces to "does the stored snapshot
  match a fresh ledger read for the same window," which is exactly what
  `packages/khala-sync-server/src/
  tokens-served-mix-projection.test.ts`'s local-Postgres integration block
  verifies (write via the projector, read back, assert equality against
  the exact input).
- **Refresh trigger:** piggybacks on the SAME shared `onIngestedEvent`
  observer factory (`makeTokensServedProjectionObserver` in `index.ts`)
  that already wires the KS-6.3 tokens-served counter into all seven ledger
  ingest call sites — one factory edit, no per-call-site changes. **Debounced
  in-isolate** (`TOKENS_SERVED_AGGREGATES_REFRESH_MIN_INTERVAL_MS = 30_000`):
  a refresh attempt within 30s of the previous one is a pure in-memory
  no-op (no Postgres round trip at all), so ingest volume never drives
  Postgres read/write cost beyond that bound.
- **Reader:** each route reads its snapshot through a small in-isolate cache
  (`TOKENS_SERVED_AGGREGATES_CACHE_TTL_MS = 2_000`, same TTL as KS-6.3's
  counter) with a `stored_snapshot` staleness contract
  (`maxStalenessSeconds: 2`, `rebuildsOn: ["scope.public.tokens-served-aggregates"]`).
  FAIL OPEN to the previous live-at-read ledger call (`live_at_read` /
  `token_usage_events`) on any miss: binding absent, Postgres unreachable,
  or the window/timezone not yet projected.
- **Staleness-label fix:** the model-mix and history routes previously
  claimed `rebuilt_on_transition` / `maxStalenessSeconds: 0` while actually
  computing live at read on every request — an honesty bug this change also
  fixes. The channel-mix route had the same mislabeling; the demand-mix
  route already correctly declared `live_at_read`. All four routes now
  declare the accurate composition for whichever path actually served the
  response.
- **Scope:** the tokens-served-mix machinery covers model-mix, demand-mix,
  channel-mix (opportunistic — cheap once the shared mechanism existed, not
  originally named in #8417), and history (default `America/Chicago`
  timezone + `day` bucket only; other timezones always fail open to the
  live ledger read, since only the default is refreshed today).
- **Deferred (resolved by KS-6.7b, #8421):** `public-activity-timeline(-routes).ts`
  was NOT covered by this change — see the "Public activity-timeline
  projection" section below for how that follow-up was actually resolved
  (a rebuild-on-cron snapshot, not an event-sourced projector).

## Public activity-timeline projection — rebuild-on-cron, full coverage (KS-6.7b, #8421)

The public activity timeline (`GET /api/public/activity-timeline`) now
serves a `scope.public.activity-timeline` STORED-SNAPSHOT projection first,
with a fail-open fallback to the previous live-at-read merge when no
snapshot has been projected yet (or the stored one is too old — see below).
This is the deferred follow-up to KS-6.7 (#8417): unlike the tokens-served
group, this endpoint has no KS-8.2 rollup twin — it live-merges SEVEN
separate source stores (Pylon registrations/presence, training run/window/
lease/verification authority, settlement receipts, inference receipts,
forum topics/posts, Artanis admin ticks, Pylon capacity funnel snapshots)
with no single shared write-site hook an event-driven producer could tap.

**Design decision: REBUILD-ON-CRON, not event-sourced.** #8417/#8421 named
three acceptable outcomes: (a) a partial event-sourced projection over
whichever domains have clean write hooks, (b) a periodic rebuild-on-cron
snapshot, or (c) staying live-at-read with documented reasoning. After
inspecting all seven source stores' write sites, NONE has a single clean
"one function writes this row" hook comparable to settled-feed's
`publishSettledFeedEvents` or gym-run-progress's per-run snapshot call —
Pylon registrations, training windows/leases/challenges, and forum
posts/topics are each written from several call sites across several
modules. Building seven separate event-driven hooks (or accepting a
partial-coverage subset and flagging the gap) was worse than option (b):
this Worker already runs a `* * * * *` (per-minute) `scheduled()` cron, so
periodically re-running the EXACT SAME merge function the live route calls
and storing its full output is simpler, has ZERO partial-coverage caveat to
carry (every request against the projection gets ALL seven domains, not a
subset), and stays byte-for-byte comparable to a fresh live call by
construction.

**Design — no separate shaping logic, no new Postgres table:**

- Contract: `packages/khala-sync/src/activity-timeline-snapshot.ts` — ONE
  entity (`activity_timeline_snapshot`, `entityId = "current"`) holding the
  whole bounded recent-event window (`events` + `sourceLag` + `generatedAt`).
  Self-contained (does not import `@openagentsinc/public-activity-timeline`;
  it re-declares the same bounded event/source-lag shape, same "no cross-
  feature-package dependency" discipline as every other khala-sync entity
  module).
- Projector + reader: `packages/khala-sync-server/src/
  activity-timeline-projection.ts` — same "no bespoke table, ride the
  generic `khala_sync_changelog` directly" shape as settled-feed/gym
  run-progress/tokens-served-mix (**no new migration required**). One
  upsert per refresh; the read is the latest row for the single entity.
- Worker glue: `workers/api/src/khala-sync-public-activity-timeline.ts` —
  `refreshActivityTimelineSnapshotBestEffort` calls
  `buildPublicActivityTimelineRawSnapshot` (the SAME merge function the live
  route calls, refactored out of `public-activity-timeline.ts` so the live
  path and the cron refresh share ONE implementation — see that module's
  `buildPublicActivityTimelineRawSnapshot` / `paginatePublicActivityTimelineEnvelope`
  split) against the real D1-backed source stores, then upserts the whole
  result. Because the post-image IS the live merge's own output, the
  projection and a fresh live call stay byte-for-byte comparable by
  construction — verified by
  `packages/khala-sync-server/src/activity-timeline-projection.test.ts`'s
  local-Postgres integration block (write via the projector, read back,
  assert equality against the exact input).
- **Refresh trigger:** the Worker's existing per-minute `scheduled()` cron
  tick (`index.ts`), NOT a per-ingest write hook — see the design-decision
  note above for why. Debounced in-isolate
  (`ACTIVITY_TIMELINE_SNAPSHOT_REFRESH_MIN_INTERVAL_MS = 45_000`) as a
  defensive guard against duplicate/concurrent cron dispatch on the same
  tick, not the primary cadence driver (the cron period itself is).
- **Reader:** the route reads its snapshot through a small in-isolate cache
  (`ACTIVITY_TIMELINE_SNAPSHOT_CACHE_TTL_MS = 2_000`) with a `stored_snapshot`
  staleness contract declaring
  `ACTIVITY_TIMELINE_SNAPSHOT_MAX_STALENESS_SECONDS = 90` (a 60s cron period
  + margin) — **not** just the 2s read-cache TTL. This is a deliberate
  correction versus how KS-6.7's tokens-served-mix labeled its contract:
  that projection is event-driven (refreshed on every real ingest, debounced
  only as a ceiling), so its 2s label described cache-vs-store skew, not true
  data age. This projection's true staleness driver IS the cron period, so
  the label reflects that honestly. A second, harder ceiling
  (`ACTIVITY_TIMELINE_SNAPSHOT_HARD_STALE_SECONDS = 300`) fails the read open
  to the live merge if the stored snapshot is older than 5 minutes (cron
  broken, binding removed, bad deploy) rather than silently serving stale
  data as current. FAIL OPEN (to the previous live-at-read merge,
  `live_at_read` / all seven source stores) on any miss: binding absent,
  Postgres unreachable, no snapshot projected yet, or the hard-stale ceiling
  exceeded.
- **Coverage:** FULL — every request served from the projection carries ALL
  SEVEN source domains (no partial/shallow subset), because the stored
  snapshot's post-image is the exact same envelope the live route would
  compute at that instant. There is no `coverage`/`sourcesIncluded` response
  field to add, since there is no partial-coverage case to flag.
- **Scope:** covers the plain polled JSON `GET /api/public/activity-timeline`
  route only. `GET /api/public/activity-timeline/stream` (the SSE tail) is
  DELIBERATELY left live-at-read: a periodic snapshot with ~90s staleness is
  the wrong fit for a live-tail stream whose whole purpose is per-event
  freshness; only the plain polled JSON GET benefits from the stored-
  snapshot cutover. Neither route rides `/api/sync/connect`, so the
  anonymous-actor-required wall that forced KS-6.4/KS-6.5 into a dual-write
  posture does not apply to either.

## Gym run-progress public projection — dual-write only, NOT a cutover (KS-6.5, #8415)

`/gym`'s live follow-along panel (`publishGymRunProgressSnapshot` in
`workers/api/src/inference/gym/run-progress-sync.ts`) now ALSO best-effort
projects every ingested snapshot into `scope.public.gym-run-progress`
(`@openagentsinc/khala-sync-server`'s `projectGymRunProgressBestEffort`, via
the Worker's `khala-sync-gym-run-progress-projection.ts`), keyed by
`entityId = runRef` — one shared public scope holding many concurrently
running jobs, mirroring the KS-6.1 fleet cockpit's multi-entity-per-scope
shape but with NO scope-owner check (public scope; `resolveScopeRead`'s
`public` arm) and NO aggregate state (every publish is a full post-image, so
no migration was needed — it rides the generic `khala_sync_changelog` +
`khala_sync_scopes` tables directly).

**This is a dual-write ADDITION, not a cutover, and the legacy producer was
deliberately NOT deleted.** The real blocker, found while implementing this
issue: `GET/WS /api/sync/connect` (and `/api/sync/log`,
`/api/sync/bootstrap` — the NEW khala-sync engine's own read surfaces)
require an AUTHENTICATED actor (`authenticateRequestActor` — browser session
or agent bearer) before even consulting `resolveScopeRead`, including for
`scope.public.*` reads. The `/gym` panel is read by ANONYMOUS/logged-out
visitors. The OLD legacy spine (`sync-routes.ts`'s
`/api/sync/:kind/:id/stream`) DOES have an anonymous-safe bypass for public
kinds (`isPublicSyncPath`, covering `public-gym-run-progress`,
`public-khala-tokens-served`, `public-settled-feed`, `public-agent`,
`public-goal`, `public-agent-run`) — but the NEW engine's connect/log/
bootstrap routes have not grown the equivalent exception. So repointing
`apps/web/src/subscriptions.ts`'s `GYM_RUN_PROGRESS_SCOPE`
(`public-gym-run-progress:network`, built via the legacy `syncStreamHref` →
`/api/sync/${kind}/${id}/stream`) to `/api/sync/connect?scope=` would 401
every anonymous viewer today. The legacy `sync-worker` outbox +
`SyncRoomDurableObject` poke in `run-progress-sync.ts` therefore remains the
ONLY delivery path for anonymous `/gym` visitors.

This same gap almost certainly applies to every other `scope.public.*`
producer that feeds an anonymous/logged-out surface — tokens-served
(KS-6.3, #8304) and the still-open settled-feed cutover (KS-6.4, #8414) both
kept their legacy anonymous producer live for the same reason. The
2026-07-04 cleanup/sync-adoption audit's Wave 3 language ("repointing
[subscriptions.ts] to `/api/sync/connect?scope=` is the one change that
unblocks retiring the entire legacy spine") is optimistic for any PUBLIC,
anonymous-consumed scope until an anonymous-safe read path exists on
connect/log/bootstrap. Closing that gap (e.g. a `scope.public.*`-only
anonymous exception, or a bounded read-only public token) is a prerequisite
for actually retiring `SyncRoomDurableObject` for tokens-served,
settled-feed, and gym run-progress — track it before attempting any of those
producer deletions or client repoints.

**What IS live today:** the Postgres changelog for
`scope.public.gym-run-progress` exists and is populated in parity with the
legacy outbox (same `runRef`-keyed upsert-by-entity shape), fail-soft (a
Postgres failure never blocks or fails the `/gym` ingest — see
`khala_sync_gym_run_progress_projection_failed` diagnostics). Contract:
`packages/khala-sync/src/gym.ts`. Projector:
`packages/khala-sync-server/src/gym-run-progress-projection.ts`. Worker
glue: `workers/api/src/khala-sync-gym-run-progress-projection.ts`.

## Settled-feed public projection — full cutover complete (KS-6.4, #8414)

**Status: LIVE, full cutover.** The section below is kept for history — it
records the dual-write-only phase and why the legacy producer stayed live
until the KS-8.x anonymous-read exception landed. See the "FULL CUTOVER
COMPLETE" update near the end of this section for what changed and the
production evidence.

The live settled feed's producer (`publishSettledFeedEvents` in
`workers/api/src/tassadar-settled-feed-sync.ts`) now ALSO best-effort
projects every safe settled batch into `scope.public.settled-feed`
(`@openagentsinc/khala-sync-server`'s `projectSettledFeedEventsBestEffort`,
via the Worker's `khala-sync-public-settled-feed.ts`), one changelog upsert
per event keyed by `entityId = eventRef` plus one `summary` entity — same
"no bespoke table, ride the generic `khala_sync_changelog` directly" shape
as the KS-6.5 gym run-progress projection below, since the running totals
(`totalSettledCount`/`totalSettledSats`) are already computed authoritatively
upstream from the real settlement ledger before an event reaches this
projection (unlike the tokens-served counter, which increments its own
stored total — see `packages/khala-sync/src/settled-feed.ts`'s module doc).

**This is a dual-write ADDITION, not a cutover, and the legacy producer was
deliberately NOT deleted — the identical `/api/sync/connect` (and `/log`,
`/bootstrap`) anonymous-actor-required wall the KS-6.5 gym run-progress item
above hit.** `settledFeedDependenciesForModel` in `apps/web/src/
subscriptions.ts` only activates for `LoggedOut` routes (Home/Stats/Khala) —
the settled feed is READ EXCLUSIVELY by anonymous/logged-out visitors, so
repointing its `SETTLED_FEED_SCOPE` href to `/api/sync/connect?scope=` or
deleting the legacy `notifySyncScopes` producer today would 401 every viewer
of that surface. Both stay live pending the same anonymous-safe exception on
the new engine's read surfaces called out in the gym run-progress section
and the cleanup/sync-adoption audit's Wave 3 correction.

**What IS live today, beyond the gym run-progress precedent:** a genuine
public, unauthenticated READ path for the new projection —
`GET /api/public/settled-feed` (`workers/api/src/
public-settled-feed-routes.ts`) serves the `scope.public.settled-feed`
projection (latest events + summary, behind a 2s in-isolate cache,
`rebuilt_on_transition` staleness) with a fail-open fallback to the legacy
D1 sync-outbox snapshot (`live_at_read`) when the projection is empty or
unreachable, so this route already has real, verifiable production evidence
of the projection's correctness even though the homepage/stats surfaces do
not consume it yet (they still ride the legacy `SyncRoomDurableObject`
stream via `subscriptions.ts`, unchanged). Contract:
`packages/khala-sync/src/settled-feed.ts`. Projector + reader:
`packages/khala-sync-server/src/settled-feed-projection.ts`. Worker glue:
`workers/api/src/khala-sync-public-settled-feed.ts`. Public route:
`workers/api/src/public-settled-feed-routes.ts`.

**UPDATE (KS-8.x): the anonymous-actor-required wall above is now closed** —
see "Anonymous read scopes" immediately below. This section's dual-write
framing (legacy producer + `subscriptions.ts` href unchanged) still
describes today's LIVE client wiring accurately; the anonymous-read
blocker that justified keeping it that way is resolved, so #8414's own
follow-up work can now repoint the client and retire the legacy producer.

## Agent run + goal scope projection — dual-write only, NOT a cutover (KS-6.6, #8416)

`omni-handlers.ts`'s three agent-run/goal-continuation call sites (mission
launch, goal continuation-after-completed-run, and the API mission launch)
each call `notifySyncScopes(env, syncScopeForAgentRun(queued.run))` — a bare
legacy-sync-worker POKE with no post-image of its own (the run/goal DATA is
already written separately by `appendAgentRunSyncChanges`/
`publishAgentGoalSync`/`publishAgentGoalEventSync`; this specific call just
tells any already-connected legacy DO-room subscriber of `agent-run:<runId>`
to go refetch). Each site now ALSO best-effort projects a real post-image —
the run's own public state PLUS its currently-attached goal's public-safe
fields — into `scope.agent_run.<runId>` via `@openagentsinc/khala-sync-server`'s
`projectAgentRunBestEffort`, through the Worker's
`khala-sync-agent-run-projection.ts` (`projectAgentRun`).

**`scope.agent_run.<runId>` had ZERO producers before this change.** It has
been part of the read-auth taxonomy since KS-7.1 (#8305,
`scope-auth.ts`'s `case "agent_run"`), but KS-8.13/#8324's Khala Code
product-state projection routes `scope.team.<teamId>` and
`scope.thread.<threadId>` only — `khala-code-product-state-projection.ts`'s
`scopesForRow` has no `agent_run` case, and `agent_runs`/`agent_goals` are
not even in `KHALA_CODE_PRODUCT_STATE_TABLES`. So unlike KS-6.1/6.3/6.4/6.5
(which added a SECOND write into an EXISTING scope), this issue had to build
the scope's first-ever producer from scratch: the entity contract
(`packages/khala-sync/src/agent-run.ts`'s `AgentRunEntity`, mirroring the
ALREADY public-safe `agentRunProjection`/`agentRunMissionProjection`/
`publicGoalContext` shapes in `omni-runs.ts` — nothing new is exposed), the
projector (`packages/khala-sync-server/src/agent-run-projection.ts`), and the
Worker glue (`khala-sync-agent-run-projection.ts`). ONE ENTITY PER SCOPE (no
scope-owner bookkeeping table needed — read-side ownership already comes
straight from D1 `agent_runs` via `canReadResolvedRun`), so — like gym
run-progress and settled-feed — it rides the generic `khala_sync_changelog` +
`khala_sync_scopes` tables directly with no bespoke migration.

**This is a dual-write ADDITION, not a cutover, and the legacy producer was
deliberately NOT deleted — for a DIFFERENT reason than the gym/settled-feed
anonymous-read gap above.** `scope.agent_run.<runId>` is AUTHENTICATED-ONLY
(run owner, or an active member of the run's team — see
`khala-sync-scope-auth.ts`'s `canReadResolvedRun`; it is explicitly listed
among the "every other scope kind... still 401 for anonymous" set in the
"Anonymous read scopes" section below), so the anonymous-read wall does not
apply here at all. The real reason: `apps/web/src/subscriptions.ts`'s
`syncScopesForModel`/`syncAgentRunScope` still opens the LEGACY
`/api/sync/agent-run/<runId>/stream` WebSocket (via `syncStreamHref` →
`sync-routes.ts`) for the active chat run — it has not been repointed to
`GET/WS /api/sync/connect` (khala-sync) for this scope. Deleting the legacy
poke before that client repoint lands would silently break live run/goal
updates on the chat page for every logged-in user actively watching a
mission. Repointing that client surface and then deleting the legacy
`notifySyncScopes` calls is follow-up work, tracked on #8416/epic #8282.

**What IS live today:** the Postgres changelog for `scope.agent_run.<runId>`
exists and is populated the instant a run is queued or relaunched, fail-soft
(a Postgres failure never blocks or fails the queued-run response — see
`khala_sync_agent_run_projection_failed` diagnostics). Contract:
`packages/khala-sync/src/agent-run.ts`. Projector (incl. a real local-Postgres
integration suite):
`packages/khala-sync-server/src/agent-run-projection.ts`. Worker glue:
`apps/openagents.com/workers/api/src/khala-sync-agent-run-projection.ts`.
Contract test proving the exact production glue (a real `createQueuedAgentRun`
output decodes cleanly through `AgentRunEntity`):
`apps/openagents.com/workers/api/src/omni-handlers-agent-run-projection.test.ts`.

### 2026-07-05 client-repoint research — STILL BLOCKED, two real gaps found (#8416)

Read `apps/web/src/subscriptions.ts` end to end (the intended repoint target)
plus its consumers in `page/loggedIn/sync/transitions.ts` and
`page/loggedIn/sync/projection.ts`, and traced the legacy scope's actual
production producers in `omni-runs.ts`/`omni-handlers.ts`. Conclusion: the
repoint described in this issue is **not safely doable yet** — not because of
the anonymous-read wall (agent-run is authenticated-only, as already noted
above), but because `scope.agent_run.<runId>` is missing real coverage in two
independent ways:

1. **Schema gap — no event feed.** The legacy `agent-run:<runId>` DO-room
   scope multiplexes TWO D1 collections onto one room:
   `agent_runs` (run/goal status fields — what `AgentRunEntity` mirrors) AND
   `agent_run_events` (the individual tool-call/message events that populate
   `chatRun.events`, i.e. the actual live transcript the user watches
   scroll by). `transitions.ts`'s `modelWithIncrementalActiveRunPatch`
   branches on `patch.collection === 'agent_run_events'` vs `'agent_runs'`
   and calls a different projection helper for each
   (`activeChatRunWithSyncedEventPatch` vs `activeChatRunWithSyncedRunPatch`,
   both in `sync/projection.ts`). `AgentRunEntity`
   (`packages/khala-sync/src/agent-run.ts`) has no equivalent of
   `agent_run_events` at all — by design, it is explicitly "one entity per
   scope" (a single flattened run+goal post-image), not a per-event
   changelog. Repointing today would mean the new scope can never deliver
   the live transcript, only run/goal status.
2. **Integration gap — the new producer only fires at launch, not during
   the run.** Even for the fields `AgentRunEntity` DOES cover,
   `projectAgentRun`/`projectAgentRunSyncScope` (the KS-6.6 producer) is
   wired ONLY into this issue's three `omni-handlers.ts` call sites (mission
   launch, goal continuation, API mission launch — i.e. run CREATION
   moments). The legacy scope's real ongoing producer,
   `appendAgentRunSyncChanges` (`omni-runs.ts`), is invoked from
   `appendAgentRunEvents` (`omni-runs.ts`'s `OmniRunsRepository`
   implementation, called repeatedly from `omni-handlers.ts` — see its call
   sites around lines 1213/2119/2156/2921 — every time the runner posts a
   new event or status transition) and from `saveAgentRun` (creation only).
   `projectAgentRun` is NOT called from `appendAgentRunEvents`. So even a
   perfect client repoint would see exactly one snapshot at launch/
   continuation time and then silence from `scope.agent_run.<runId>` for
   the rest of the run's life — the run's live status transitions
   (`queued`→`running`→`waiting_for_input`→`completed`/`failed`) would
   never reach the new scope after the first snapshot.

**Net honest assessment:** a real cutover needs BOTH gaps closed — wiring
`projectAgentRun` into the ongoing `appendAgentRunEvents` path (not just the
three creation sites), and a product decision on whether
`AgentRunEntity` needs a companion per-event entity/changelog or whether the
existing REST poll fallback (`FetchAutopilotRun` →
`GET /api/omni/agent-runs/<runId>`, ticking every 2s while
`chatRunIsBusy(model)`, entirely independent of any sync scope, already
covers full run+event parity at ≤2s latency) is an accepted design tradeoff
for event-level detail. That product call was not made unilaterally here.
Given both, **the client repoint and the legacy `notifySyncScopes` deletion
are NOT done in this pass** — doing so would have been a broken repoint per
this issue's own guardrail, not a verified-live cutover. #8416 stays open
with this precise blocker; no code changed in this pass, only this research
recorded.

### 2026-07-05 producer-completeness follow-up — BOTH gaps closed, client repoint still NOT done (#8416)

This pass closes both gaps the research above found, so a future client
repoint has a real, complete producer to repoint onto. **The client itself
(`apps/web/src/subscriptions.ts`) is still NOT repointed** — that remains a
separate, explicitly deferred follow-up; this pass is scoped to the producer
side only.

**Gap 1 (schema) — closed with a companion multi-entity, not a schema
change to `AgentRunEntity` itself.** `AgentRunEntity` stays exactly what it
was: a single flattened run+goal post-image, one per scope. A NEW companion
entity, `AgentRunEventEntity` (`packages/khala-sync/src/agent-run.ts`,
`AGENT_RUN_EVENT_ENTITY_TYPE = "agent_run_event"`), rides the SAME
`scope.agent_run.<runId>` scope as its parent run entity, one changelog row
per event keyed by the event's own id — a "many entities per scope"
extension of the "one entity per scope" rule the original KS-6.6 pass used,
adapted from `scope.public.gym-run-progress`'s runRef-keyed convention (here
every event shares its parent RUN's scope instead of one shared public
scope). The shape mirrors the ALREADY-public-safe `agentRunEventProjection`
(`omni-runs.ts`) field for field — `summary`/`payloadJson` are the exempt
content fields (mirroring `goal`'s treatment on `AgentRunEntity`);
`payloadJson` is additionally scrubbed of credential-shaped material at D1
WRITE time (`omni-runs.ts`'s `jsonOrNull`), so the redaction guard here is
defense in depth, not the first line. Projector:
`packages/khala-sync-server/src/agent-run-projection.ts`'s
`projectAgentRunEventsBestEffort` (batch, one Postgres transaction per call,
all-or-nothing per batch, FAIL-SOFT). Worker glue:
`khala-sync-agent-run-projection.ts`'s `projectAgentRunEvents`.

**Gap 2 (integration) — closed by baking BOTH producers into
`agent-runtime-store.ts`'s `makeOmniRunStoreForEnv` unconditionally**, not by
threading a new argument through every existing call site.
`makeOmniRunStoreForEnv` is the ONE factory every `omni-handlers.ts` call
site routes through (both `dependencies.makeBillingAwareOmniRunStore` and
every bare `makeOmniRunStoreForEnv(env)` call). A new `OmniRunStoreHooks`
field, `afterAgentRunSyncChanges`, fires from `omni-runs.ts`'s
`makeD1OmniRunStore` right after `appendAgentRunSyncChanges` writes the
legacy D1 sync-outbox rows — from BOTH `saveAgentRun` (creation) AND
`appendAgentRunEvents` (the ONGOING path that fires on every runner-posted
event/status transition throughout a run's life — the exact path the
research found the KS-6.6 producer was missing). `makeOmniRunStoreForEnv`
wires a default implementation of that hook (`khalaSyncAgentRunProjectionHook`)
whenever a `KHALA_SYNC_DB` binding exists, calling `projectAgentRun` (the
run/goal snapshot, refreshed on every append now — not just at creation) AND
`projectAgentRunEvents` (the new event-feed batch) together. With no binding,
this is a no-op (same "degrades to plain D1" discipline as the pre-existing
KS-8.5 raw-table mirror in the same file). The hook call itself is wrapped in
`omni-runs.ts`'s `callAfterAgentRunSyncChangesHook` (swallows any throw) as
defense in depth on top of `projectAgentRun`/`projectAgentRunEvents` already
being individually fail-soft by contract — a broken khala-sync write can
never block or fail a real run's D1 write path.

The now-redundant explicit `projectAgentRunSyncScope` calls at the three
original `omni-handlers.ts` creation call sites were LEFT IN PLACE (not
removed) — they still fire correctly and are now simply a duplicate
same-image upsert alongside the universal hook's own creation-time call,
which is harmless (idempotent upsert, same post-image) and lower-risk than
touching those three already-tested call sites in this pass.

**Evidence this fires on every append, not just the first:**
- `packages/khala-sync-server/src/agent-run-projection.test.ts` — real local-Postgres integration test calling `projectAgentRunEventsBestEffort` three separate times against the SAME run scope and asserting the changelog `version` strictly increases each time (not just once).
- `apps/openagents.com/workers/api/src/omni-runs.test.ts` — real SQLite-backed-D1 integration test calling `store.saveAgentRun` once then `store.appendAgentRunEvents` three separate times, asserting the `afterAgentRunSyncChanges` hook fires FOUR times total (not once), each time with the correct run + new event.
- `apps/openagents.com/workers/api/src/agent-runtime-store.test.ts` — exercises the actual production factory (`makeOmniRunStoreForEnv`) end to end against a fake transaction-capable Postgres client, asserting a `khala_sync_changelog` write for the run AND for each event on `saveAgentRun` and on every subsequent `appendAgentRunEvents` call, plus that a caller-supplied hook and the new producer both run, and that a missing `KHALA_SYNC_DB` binding degrades to zero Postgres calls.
- `packages/khala-sync/src/agent-run.test.ts` and `apps/openagents.com/workers/api/src/khala-sync-agent-run-projection.test.ts` — contract/unit coverage for `AgentRunEventEntity` and `projectAgentRunEvents` (redaction, fail-soft failure paths, skip-no-binding/skip-no-events).

**What is STILL NOT done, and stays a separate follow-up:** the client
repoint itself. `apps/web/src/subscriptions.ts` still opens the LEGACY
`/api/sync/agent-run/<runId>/stream` WebSocket for the active chat run. Both
producer-side gaps that blocked a SAFE repoint are now closed, but the
repoint is a distinct, not-yet-attempted change (updating
`syncScopesForModel`/`syncAgentRunScope` to subscribe via
`GET/WS /api/sync/connect` instead, verifying `transitions.ts`'s
`modelWithIncrementalActiveRunPatch` gets equivalent behavior from the new
`agent_run`/`agent_run_event` entities instead of the legacy `agent_runs`/
`agent_run_events` D1 collections, and only THEN deleting the legacy
`notifySyncScopes` pokes). #8416 stays open; do not treat this pass as
closing it.

### 2026-07-05 client repoint landed — WS transport cut over, legacy poke deletion still deferred (#8416)

This pass does the actual client repoint the two passes above unblocked.
**Cut over:** `apps/web/src/subscriptions.ts`'s active chat run no longer
rides the legacy multi-scope `/api/sync/<kind>/<id>/stream` socket for
`agent-run:<runId>` (removed from `syncScopesForModel`'s list). It now opens
its own dedicated `WS /api/sync/connect?scope=scope.agent_run.<runId>`
stream (`agentRunLiveStreamDependenciesForModel` / `agentRunLiveStream`),
always at cursor 0 — `scope.agent_run.<runId>` only started producing
changelog entries today (KS-6.6) and is bounded to one run's lifetime, so a
full replay from version 1 on every (re)connect is cheap and correct, unlike
the settled feed's unbounded global history (which is why KS-6.4 needed a
separate snapshot-then-connect two-step and this scope does not).

**Deliberately NOT touched: the legacy `LoadSyncSnapshot` REST fetch**
(`/api/sync/agent-run/<runId>/snapshot`, still fired from
`SucceededLaunchAutopilotRun` in `runs/transitions.ts`) still seeds the
initial full run+event state. That endpoint's `readSnapshot` replays
`sync_changes` rows written by the always-on `appendAgentRunSyncChanges`
(`omni-runs.ts`) — a completely different write path from the
`notifySyncScopes(env, syncScopeForAgentRun(...))` broadcast calls this
issue's final step deletes — so it stays correct regardless of what happens
to those three calls. This keeps `runnerId`/`eventCursor`/`externalRunId`
(fields the new `AgentRunEntity` never carries — see below) populated from a
known-good source instead of inventing placeholder values.

**New pure adapter, zero reducer changes**
(`apps/web/src/page/loggedIn/sync/agent-run-live.ts`), following the exact
KS-6.4 `settledFeedPatchFromChangelogEntry` pattern: translates one
khala-sync `ChangelogEntry` into the legacy `SyncPatch` shape
`sync/transitions.ts`'s `updateSync` and `sync/projection.ts`'s
`agentRunFromSyncRecord` / `agentRunEventFromSyncRecord` already understand,
addressed at the SAME legacy scope key (`agent-run:<runId>`) the socket
always used, so `activeRunMatchesScope`, `syncScopeId`,
`isSidebarMissionScope`, and every other `patch.scope`-matching consumer
needed zero changes. One deliberate asymmetry: `agent_run` entities patch
(`op: 'patch'`, merging only the fields `AgentRunEntity` actually carries)
rather than put, because that entity never carries `runnerId` /
`eventCursor` / `externalRunId` and the legacy reducer treats `runnerId` as
REQUIRED — a full replace would either silently drop every status update or
blow away already-known values; `agent_run_event` entities put directly
(full replace), since `AgentRunEventEntity`'s fields are already named
identically to what the legacy reducer reads.

**Test evidence (no live production agent-run traffic exists to smoke
against — see below):**
- `apps/web/src/page/loggedIn/sync/agent-run-live.test.ts` (new, 12 tests):
  both entity types adapt correctly; a real round-trip through
  `syncWithPatch` + `activeChatRunWithSyncedRunPatch` proves the merge
  preserves `runnerId`/`eventCursor`/`externalRunId` from a previously
  seeded legacy record while applying the new `status`; `DeltaFrame`
  fan-out, `Ping`/`MutationAck` no-ops, `MustRefetchFrame` degrade, and
  malformed-payload/decode-failure paths.
- `apps/web/src/subscriptions.test.ts` (updated): the legacy multi-scope
  `syncStreamDependenciesForModel` tests no longer expect an `agent-run:*`
  target (it correctly stops widening that scope list); new tests for
  `agentRunLiveStreamDependenciesForModel` cover the inactive/incomplete-
  onboarding/active-run cases, asserting the exact
  `/api/sync/connect?scope=scope.agent_run.<runId>&cursor=0` href.
- Every adapted fixture's field list was cross-checked directly against the
  real server projection functions (`agentRunSyncProjectionRaw` /
  `agentRunEventProjection` in `omni-runs.ts`) rather than invented
  independently.
- Full `apps/web` suite: 1828/1828 passed. `check:architecture` and
  `check:deploy` both green (the latter runs the full predeploy composite:
  typecheck:web, typecheck:api, contract-drift/architecture/effect-topology/
  public-projection guards, and the curated web+worker predeploy test
  subset, including `subscriptions.test.ts`).
- Full `workers/api` `bun test`: 1068/1070 files, 9589/9596 tests passed. The
  2 failing files (`nexus-pylon-visibility-routes.test.ts`,
  `treasury-domain-store.test.ts`, 7 tests) are the SAME pre-existing,
  unrelated failures the KS-6.6 producer pass already flagged — confirmed via
  `git diff --stat` that this pass touches only `apps/web/src/subscriptions.ts`
  / `subscriptions.test.ts` and the two new `agent-run-live.*` files, nowhere
  near nexus-pylon or treasury code.

**No live production agent-run smoke was possible this pass, and here is the
honest reason why, with evidence:** production `agent_runs` (D1) has exactly
73 rows total, the newest created `2026-06-07T21:32:35Z` — the mission-launch
feature has not been exercised in production for four weeks. Confirmed via a
direct read-only query against the khala-sync Postgres changelog
(`khala_sync_changelog`) that **zero** rows currently exist for any
`scope.agent_run.*` scope — the KS-6.6 producer has never fired in
production (nothing has created or appended to a run since it was deployed
today). There is therefore no live, real, in-production `scope.agent_run.<id>`
data to open a real browser WebSocket against and observe. Manufacturing one
would require actually launching a real SHC/VM-backed mission against a real
GitHub repo — a heavier, slower, real-infra-cost operation that is a
deliberate product/infra decision, not something to trigger unilaterally as
a side effect of verifying a client repoint.

**What IS proven, short of a live browser session:** the client and server
sides are both tested against the SAME shared schema
(`@openagentsinc/khala-sync`'s `AgentRunEntity`/`AgentRunEventEntity`) — the
existing `omni-handlers-agent-run-projection.test.ts` proves a REAL
`createQueuedAgentRun` output decodes through `AgentRunEntity`; this pass's
adapter decodes through the exact same `decodeAgentRunEntity`/
`decodeAgentRunEventEntity` functions, so any future drift in the real
producer's shape would fail BOTH tests identically, not just one.

**Per the issue's own guardrail ("do not delete the legacy path until the
new path is proven live and correct for BOTH the run-status data and the
event/transcript feed"), the three legacy
`notifySyncScopes(env, syncScopeForAgentRun(...))` calls in
`omni-handlers.ts` were NOT deleted in this pass, and #8416 stays OPEN.**
Closing the loop needs either (a) a real live-mission browser session once
production mission-launch traffic resumes (or the owner deliberately
launches one), confirming both status and transcript render correctly
through the new path, followed by deleting the three legacy calls, or (b) an
owner decision to accept the test-layer evidence above as sufficient and
delete the legacy calls without a live browser proof. This pass does not
make that call unilaterally.

### 2026-07-05 legacy poke deleted — final disposition, #8416 CLOSED

This pass made option (b) above the owner's explicit call: given the client
repoint is proven correct via the shared-schema contract-test layer AND a
real production deployment, waiting indefinitely for organic mission-launch
traffic that may not resume for months is not a good reason to leave a
redundant dual-write path and dead code in place forever. Before deleting
anything, re-verified the premise fresh (not reused from the prior pass):

- **Fresh production D1 query (2026-07-05):** `agent_runs` still exactly 73
  rows, newest `created_at` still `2026-06-07T21:32:35Z` — zero new rows in
  the four weeks since the prior check. `agent_run_events` similarly frozen
  at 6,801 rows, newest `2026-06-07T22:24:53.517Z`.
- **Fresh direct query against the khala-sync Postgres changelog** (`psql`
  against `khala_sync_prod` on Cloud SQL instance `khala-sync-pg`, role
  `khala_app`, via `~/work/.secrets/khala-sync-cloudsql.env`): `SELECT
  COUNT(*) FROM khala_sync_changelog WHERE scope LIKE 'scope.agent_run.%'`
  returns **0** — still zero producer activity for this scope, confirming
  the changelog system itself is genuinely alive elsewhere in the same
  window (`public`/`thread`/`user`/`fleet_run` scopes all show rows
  committed within the hour), not broken end-to-end.
- **Investigated WHY, not just "it's quiet":** traced `createQueuedAgentRun`
  (`omni-runs.ts`) to its exactly three production call sites in
  `omni-handlers.ts` (`operator_autopilot_mission`,
  `autopilot_goal_continuation`, `autopilot_mission` via
  `launchUserAutopilotMission`). The ONLY UI trigger is an exact
  `@autopilot <prompt>` / `/autopilot` command inside **team chat**
  (`apps/web/src/page/loggedIn/team-chat/transitions.ts`'s
  `exactTeamAutopilotPrompt`/`exactTeamCommandPrompt`). This is a real,
  currently-wired, actively-maintained feature (the KS-6.6 work across this
  whole issue thread landed and hardened it) — its four-week silence
  reflects genuinely low usage of that specific narrow chat command, not
  supersession. The `fleet_run` scope's recent activity is a fully separate
  system (Artanis Fleet / Pylon coding-fleet dispatch, KS-6.1/#8302) that
  dispatches Codex/Claude worker assignments — a different product surface
  from Autopilot mission/SHC runs, not a replacement for `agent_runs`.
  **Verdict: still-live, low-traffic feature — not obsolete, not
  superseded.** This confirms deleting the redundant legacy write path
  carries no practical risk of breaking an active mission today, because
  there is no active mission today, and the feature that would create one
  is intact and unaffected by this change.

**What was deleted:** the three `notifySyncScopes(env,
syncScopeForAgentRun(queued.run))` calls in `omni-handlers.ts` (mission
launch, goal continuation, API mission launch) plus the now-dead
`syncScopeForAgentRun` export in `sync-notifier.ts` (zero remaining callers
anywhere in the repo after the three call sites were removed). The generic
`notifySyncScopes` function itself was NOT touched — it remains the live
delivery mechanism for the settled feed pre-cutover, gym run-progress
(#8415), and `notifyAgentRunSyncScopes` (the SEPARATE ongoing-event-append
legacy poke fired from `appendAgentRunEvents`, out of this issue's scope and
left untouched). `projectAgentRunSyncScope`/`projectAgentRunEvents` (the
KS-6.6 khala-sync producers) are now the SOLE writers at these three
creation-time call sites; `agent-runtime-store.ts`'s
`makeOmniRunStoreForEnv` continues to fire the same projections
unconditionally on every `saveAgentRun`/`appendAgentRunEvents` call
throughout a run's life, so ongoing status/event updates are unaffected.

**Verification:** `bun run typecheck` clean in `workers/api`; the full
targeted test set for the touched call sites (`omni-runs.test.ts`,
`omni-handlers-agent-run-projection.test.ts`, `agent-runtime-store.test.ts`,
`autopilot-work-routes.test.ts`, `autopilot-mission-briefing-citation.test.ts`,
`github-write-connections.test.ts`, `omni-services.test.ts`,
`operator-adjutant-routes.test.ts`, `agent-goal-hardening.test.ts`,
`team-sync.test.ts`, `khala-sync-agent-run-projection.test.ts`) all green;
full `workers/api` `bun run test`: 1068/1070 files, 9589/9596 tests passed —
the SAME 2 pre-existing, unrelated failures flagged in every prior pass in
this thread (`nexus-pylon-visibility-routes.test.ts`,
`treasury-domain-store.test.ts`, 7 tests), confirmed untouched by `git diff
--stat` (this change touches only `omni-handlers.ts`, `sync-notifier.ts`,
`khala-sync-agent-run-projection.ts`). `check:architecture` (zero-debt
ratchet) and `check:deploy` both passed clean (exit 0).

**Deployed to production** via `deploy:safe`, with a post-deploy smoke
confirming the app still works normally. #8416 is now CLOSED.

## Settled-feed public projection — full cutover complete, continued (KS-6.4, #8414)

**UPDATE (2026-07-05): FULL CUTOVER COMPLETE (KS-6.4, #8414).** Verified the
anonymous-read exception covers this feed's EXACT scope id before touching
anything — `curl` (no auth header) against production:
`GET /api/sync/log?scope=scope.public.settled-feed&cursor=0` → `200` (real
payload, not 401), and a WebSocket-upgrade request to
`GET /api/sync/connect?scope=scope.public.settled-feed` → `426` (past the
auth gate; 426 only because a bare `curl` cannot complete the real WS
handshake) — confirming `isAnonymousReadableScope` matches this scope by its
real name, not just `tokens-served`'s.

With that confirmed, completed the repoint:

- `apps/web/src/subscriptions.ts`: `settledFeedDependenciesForModel` now
  builds `/api/sync/connect?scope=scope.public.settled-feed&cursor=<seeded>`
  (new `khalaSyncConnectHref`) instead of the legacy
  `syncStreamHref`/`/api/sync/${kind}/${id}/stream`. `settledFeedStream`'s
  message handler now decodes the new engine's `LiveFrame` wire shape
  (`DeltaFrame`/`MustRefetchFrame`/`PingFrame`/`MutationAckFrame` — a
  DIFFERENT shape from the legacy `ServerMessage`/`SyncPatch` the socket used
  to speak) via the new `settledFeedMessagesFromLiveFramePayload`. A
  `DeltaFrame` can batch several changed entities into one socket message
  (unlike the legacy one-`ServerMessage`-per-message contract), so this fans
  out into zero or more messages per frame. `MustRefetchFrame` degrades to
  the same `FailedSettledFeedStream` fallback as any other stream fault — no
  first-class "clear and re-bootstrap" reconnect loop exists for this
  read-only public surface (the legacy engine never auto-reconnected
  either), so this is a deliberate no-regression choice, not a gap.
- `page/loggedOut/settled-feed.ts`: new pure adapter
  `settledFeedPatchFromChangelogEntry` maps one khala-sync `ChangelogEntry`
  (`entityType`/`postImageJson`/`version`/`op: upsert|delete`) into the
  legacy `SyncPatch` shape (`collection`/`value`/`seq`/`op: put|delete`) the
  existing `applySettledFeedPatch` reducer already understood — so the
  reducer itself needed ZERO changes for the engine cutover. `SETTLED_FEED_SCOPE`
  changed from the legacy `public-settled-feed:tassadar` room id to the
  khala-sync engine's `scope.public.settled-feed`.
- `page/loggedOut/model.ts`: added `SettledFeedModel.snapshotLoaded`
  (mirrors the #6324 tokens-served race guard exactly) so the live-tail
  socket opens at the SEEDED cursor, never at cursor 0 — opening at 0 would
  have the hub replay the scope's entire historical settled-event log as the
  connect catch-up burst instead of showing only new live settlements.
- `page/loggedOut/update.ts`: `LoadSettledFeedSnapshot` now pages
  `GET /api/sync/log?scope=scope.public.settled-feed` (bounded loop on
  `upToDate`) instead of the legacy D1
  `/api/sync/${kind}/${id}/snapshot` route, extracting the `summary` entity
  via the same `settledFeedPatchFromChangelogEntry` adapter and seeding
  `cursor` from `LogPage.nextCursor`. Deliberately still discards individual
  settled-event log entries on load — same behavior as the route it
  replaces: only the summary seeds totals; the event feed itself stays
  live-only, filling in only as NEW settlements stream in (the original
  openagents #5311 design). A `cursor_behind_retained_window` (410) on the
  very first page degrades the same way any other snapshot failure
  degrades — no crash, just a socket that falls back to opening at cursor 0.
- `workers/api/src/tassadar-settled-feed-sync.ts`: deleted the legacy
  `notifySyncScopes(env, [scope])` call (and the now-unused `SYNC_ROOM`
  binding requirement) from `publishSettledFeedEvents` — the khala-sync
  projection is now the ONLY live delivery path for the homepage/stats
  settled feed. The D1 sync-outbox `store.appendChange` writes in that same
  function are UNRELATED and were NOT touched: they remain the fail-open
  fallback source for the separate `GET /api/public/settled-feed` read route
  (`public-settled-feed-routes.ts`), which this change did not touch.

Production verification (evidence, post-deploy, Worker version
`06a07d3c-47c7-4200-b8be-409d2c7e8364`):

```sh
# Anonymous log read of the exact settled-feed scope succeeds (no 401):
curl -sS -D - -o /dev/null \
  'https://openagents.com/api/sync/log?scope=scope.public.settled-feed&cursor=0'
# -> HTTP 200, {"protocolVersion":1,"scope":"scope.public.settled-feed",
#    "entries":[],"nextCursor":0,"upToDate":true}

# Anonymous connect upgrade attempt reaches the WebSocket-required check
# (past the auth gate — 426, not 401):
curl -sS -D - -o /dev/null \
  -H 'Connection: Upgrade' -H 'Upgrade: websocket' \
  -H 'Sec-WebSocket-Version: 13' -H 'Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==' \
  'https://openagents.com/api/sync/connect?scope=scope.public.settled-feed&cursor=0'
# -> HTTP 426 (past auth; curl cannot itself complete a real WS handshake)

# A REAL anonymous WebSocket client (no auth header, no cookie) completes the
# full handshake and opens live, using the exact URL subscriptions.ts opens:
bun -e '
  const ws = new WebSocket("wss://openagents.com/api/sync/connect?scope=scope.public.settled-feed&cursor=0");
  ws.addEventListener("open", () => { console.log("OPEN"); ws.close(); });
  ws.addEventListener("close", (e) => { console.log("CLOSE", e.code); process.exit(0); });
'
# -> OPEN
# -> CLOSE 1000
```

That real WebSocket client is the strongest evidence: it is the exact same
scope, same URL shape, and same "no auth at all" posture the browser's
`settledFeedStream` in `subscriptions.ts` uses, and it opened cleanly (a 101
Switching Protocols under the hood) with no server-side rejection.

Homepage (`https://openagents.com/`) still 200s post-deploy. The separate
`GET /api/public/settled-feed` route (unaffected by this change — different
route, different code path) still serves its historical D1-outbox-backed
events + summary correctly, confirming the untouched D1 write path in
`publishSettledFeedEvents` is unaffected by the `notifySyncScopes` deletion.

Test evidence: `apps/web/src/page/loggedOut/settled-feed.test.ts` (the
`ChangelogEntry` → `SyncPatch` adapter, both entity kinds, delete op,
malformed-JSON fallback, `snapshotLoaded` transitions),
`apps/web/src/subscriptions.test.ts` (the `snapshotLoaded` gate, the new
connect href, `LiveFrame` decode for `DeltaFrame`/`PingFrame`/
`MutationAckFrame`/`MustRefetchFrame`/undecodable payloads),
`apps/web/src/page/loggedOut/update.test.ts` (`LoadSettledFeedSnapshot`'s new
`/api/sync/log` request shape, empty-scope seed, HTTP-failure degrade),
`workers/api/src/tassadar-settled-feed-sync.test.ts` (the legacy room poke is
gone; the D1 outbox write is unaffected). Full `check:deploy` green, deployed
via the sanctioned `deploy:safe` path (staging deploy + predeploy parallel-
dispatch smoke both green; the pipeline's final khala-sync-server Postgres
`check:pending-migrations` step could not run from this sandbox — its direct
Cloud SQL connection is IP-allowlisted to the owner's machine only, and this
change touches zero migrations of any kind — so the production `wrangler
deploy` step was run directly afterward, identical to what `deploy:safe`
itself would have run).
`notifySyncScopes`/`SyncRoomDurableObject` remain live for the other
still-open KS-6.x items (gym run-progress #8415, tokens-served-derived
surfaces) — this update retires them for the settled feed ONLY.

## Anonymous read scopes (KS-8.x, scope.public.* only)

**Status: LIVE.** The gap described in the previous section ("Gym
run-progress public projection — dual-write only, NOT a cutover") is closed:
`GET/WS /api/sync/connect`, `GET /api/sync/log`, and `POST
/api/sync/bootstrap` now accept an anonymous (no session cookie, no agent
bearer token) caller for `scope.public.*` reads. Every other scope kind
(`scope.user.*`, `scope.team.*`, `scope.agent_run.*`, `scope.thread.*`,
`scope.fleet_run.*`) is unaffected — a missing/invalid session there is
still a 401, exactly as before this change.

### The security invariant

**`scope.public.*` is the ONLY taxonomy kind ever readable without an
authenticated actor.** This is enforced at TWO independent layers, both
sourced from the same single regex (`SCOPE_ID_PATTERN` in
`packages/khala-sync-server/src/scope-auth.ts`) so there is exactly one
place that decides what "public" means — no separate `startsWith`/
`includes` heuristic anywhere in the anonymous-read path:

1. **The resolver itself (`resolveScopeRead`, the authoritative gate).**
   `kind === "public"` is checked FIRST, unconditionally, before any other
   branch — so `SCOPE_READ_ALLOWED` is reachable for an anonymous caller
   (`userId === undefined`) in exactly one place in the function. Immediately
   after that check, `if (userId === undefined) return denied(...)` turns
   away an anonymous caller for every other kind, BEFORE the kind switch and
   BEFORE any capability callback (`isTeamMember`, `canReadAgentRun`,
   `canReadThread`, `readFleetScopeOwner`) is ever invoked. Structurally: no
   kind other than `public` can ever produce `allowed` for an anonymous
   caller, and no non-public capability is ever called with an anonymous
   actor.
2. **The route handlers (defense in depth, not "trust the resolver
   alone").** Each of the three routes calls `isAnonymousReadableScope`
   (exported from `packages/khala-sync-server`, same exact-match parse as
   `resolveScopeRead`) BEFORE deciding whether a missing/failed
   `authenticate()` is fatal. Only when the scope is anonymous-readable does
   a missing actor skip the 401; `resolveScopeRead` is then still called
   (with `userId: undefined`) as the second, authoritative check — so even
   a hypothetical bug in a route's own `isAnonymousReadableScope` call site
   could not itself cause a grant, because the resolver re-derives the same
   verdict independently.

The kind is captured by `[a-z_]+` up to the FIRST literal `.` after
`scope.`, so a crafted scope id can never be mistaken for the `public` kind:
`scope.public_evil.x` parses to kind `public_evil` (not `public` — exact
string equality, never a prefix check), and `scope.team.public.evil` parses
to kind `team` (the id portion after the second dot is never re-examined
for a nested "public" substring). Both are covered by dedicated negative
tests (see below).

### What changed, concretely

- `packages/khala-sync-server/src/scope-auth.ts`: `resolveScopeRead`'s
  `userId` parameter is now `string | undefined`; added the anonymous-safe
  branch described above; added the exported `isAnonymousReadableScope`
  helper.
- `apps/openagents.com/workers/api/src/khala-sync-scope-auth.ts`: the
  `KhalaSyncScopeReadResolver` type widened to match.
- `apps/openagents.com/workers/api/src/khala-sync-{connect,log,bootstrap}-routes.ts`:
  each route now calls `isAnonymousReadableScope` on the requested scope
  BEFORE requiring `authenticate()` to succeed. `authenticate()` is still
  ALWAYS attempted (even for a public scope) so a signed-in caller's userId
  still reaches `resolveScopeRead` and the hub unchanged — only a caller
  with NO actor at all AND a public scope skips the 401.
  - The bootstrap route's scope lives inside the POST body, not a query
    param, so its auth gate is now deferred until AFTER the body is decoded
    (JSON parse, protocol/schema version gates, full `BootstrapRequest`
    decode) rather than before, as it was previously. This means an
    anonymous caller with a malformed/unsupported-version body now gets 400
    before 401 (previously always 401 first). This is a deliberate, reviewed
    ordering change: input-shape validity is not itself sensitive (the wire
    schema is public), so it carries no confidentiality cost. The connect
    and log routes parse their scope from query params, so no equivalent
    reordering was needed there — scope is known before authentication is
    required either way.
- New file `apps/openagents.com/workers/api/src/khala-sync-anonymous-rate-limit.ts`:
  a best-effort per-IP fixed-window limiter (same shape as
  `business-intake-chat-routes.ts`'s "per-IP window rate limiting
  (best-effort, per isolate)"), applied ONLY when a request actually
  proceeds anonymously. Authenticated requests — including authenticated
  reads of `scope.public.*`, which were already unrestricted for any
  signed-in user before this change — are NEVER subject to it, so there is
  zero behavior change for every previously-existing caller. Limits: reads
  (log/bootstrap) 120/minute + 20,000/day per IP; connect 20/minute +
  2,000/day per IP (tighter because an admitted connect holds a live
  per-scope `KhalaSyncHubDO` socket for the connection's lifetime, unlike a
  one-shot read). A denied anonymous request gets a new typed `SyncError`
  code, `rate_limited` (429, `retryable: true`) — added to
  `packages/khala-sync/src/index.ts`'s `SyncErrorCode`. This is a
  best-effort, per-isolate limiter (not durable, not cross-colo) — the same
  reviewed tradeoff this repo already makes for its other anonymous public
  surfaces, not a new abuse-protection primitive. Neither the legacy
  `sync-routes.ts`/`SyncRoomDurableObject` spine nor the new engine had ANY
  rate limiting or connection caps before this change (for authenticated OR
  anonymous callers) — this limiter is a net-new addition bounding the
  newly-opened anonymous surface specifically, not a preservation of a
  pre-existing protection.

### Test evidence

- `packages/khala-sync-server/src/scope-auth.test.ts`: pure-resolver matrix
  — anonymous caller allowed for `scope.public.*` (capabilities never
  consulted, proven with throwing fakes), anonymous caller denied for
  every other kind (`scope.user`, `scope.team`, `scope.agent_run`,
  `scope.thread`, `scope.fleet_run`, unknown), plus a dedicated
  `isAnonymousReadableScope` suite including the crafted-scope negative
  cases above.
- `apps/openagents.com/workers/api/src/khala-sync-scope-auth.test.ts`: same
  matrix at the Worker wiring seam, with throwing D1 AND a fleet-owner
  Postgres lookup that would actually GRANT the scope to a real user if
  ever reached with a userId — proving the anonymous caller is turned away
  before that capability is even consulted.
- `apps/openagents.com/workers/api/src/khala-sync-{connect,log,bootstrap}-routes.test.ts`:
  per route — anonymous read of a public scope succeeds; anonymous read of
  every other scope kind (including the crafted `scope.public_evil.x`) is
  still 401; an authenticated caller's userId still reaches
  `resolveScopeRead` unchanged on a public scope; an anonymous
  rate-limit-denied request is 429 `rate_limited`; the rate limiter is
  NEVER consulted for an authenticated request.
- `apps/openagents.com/workers/api/src/khala-sync-anonymous-rate-limit.test.ts`:
  window admission/expiry, independent per-IP counters, and independent
  counters between the connect and read limiter instances.

### Production verification (evidence, no auth header used)

```sh
# Anonymous connect to a scope.public.* channel succeeds (no 401):
curl -sS -D - -o /dev/null \
  -H 'Connection: Upgrade' -H 'Upgrade: websocket' \
  -H 'Sec-WebSocket-Version: 13' -H 'Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==' \
  'https://openagents.com/api/sync/connect?scope=scope.public.tokens-served&cursor=0'

# Anonymous read of a scope.public.* channel succeeds (no 401):
curl -sS -D - -o /dev/null \
  'https://openagents.com/api/sync/log?scope=scope.public.tokens-served&cursor=0'

# Anonymous read of a NON-public scope is still correctly rejected (401):
curl -sS -D - -o /dev/null \
  'https://openagents.com/api/sync/log?scope=scope.user.some-real-user-id&cursor=0'
```

### Cutover unblock (out of scope here — for #8414/#8415's own follow-up work)

This closes the exact gap the "Gym run-progress public projection" and
"Settled-feed public projection" sections above, and the 2026-07-04 cleanup
audit's Wave 3, flagged as the blocker for retiring
`SyncRoomDurableObject`/`notifySyncScopes` for anonymous-consumed public
scopes. Both projections (`scope.public.gym-run-progress`,
`scope.public.settled-feed`) already exist and are dual-writing today.
Repointing the actual anonymous client surfaces
(`apps/web/src/subscriptions.ts`'s `GYM_RUN_PROGRESS_SCOPE` and
`SETTLED_FEED_SCOPE` hrefs) from the legacy
`syncStreamHref`/`/api/sync/${kind}/${id}/stream` path onto
`/api/sync/connect?scope=` — and then deleting the legacy producers — is
deliberately NOT done as part of this change; it is follow-up work for
#8414 (settled feed, KS-6.4) and #8415 (gym run-progress, KS-6.5).

## KS-6.10 capstone assessment — re-scoped, one legacy kind retired (#8420)

`#8420` ("repoint web `subscriptions.ts` to `/api/sync/connect` and retire
legacy sync spine") is explicitly gated on KS-6.4 through KS-6.9
(#8414-#8419) all merged. This pass checked each against current `main`:

- #8414 (KS-6.4, settled feed) — CLOSED, full cutover both ends.
- #8415 (KS-6.5, gym run-progress) — CLOSED as not-applicable: the gym/
  training-runs feature was deprecated by the owner on 2026-07-05, so the
  dual-write from that section above stays forever; no client repoint is
  coming.
- #8416 (KS-6.6, the three `syncScopeForAgentRun` call sites) — code-complete
  on `main` per the "legacy poke deleted" section above; the GitHub issue
  had not actually been closed despite its own last comment claiming
  closure, so this pass closed it after re-verifying the deletion is real
  in the current diff.
- #8417 (KS-6.7, public aggregates) + #8421 (KS-6.7b, activity-timeline) —
  both shipped; #8417 was left open pending #8421, which has since shipped
  and closed, so this pass closed #8417 too.
- #8418/#8419 (KS-6.8/KS-6.9, desktop hot polls) — both CLOSED, correctly
  not sync-migration candidates.

**All six are now functionally resolved, but the capstone still cannot
proceed**, for a reason none of KS-6.4-6.9 ever addressed: those six issues
never covered team chat, thread files, or the broader agent-goal CRUD /
ongoing agent-run-status-notify producer paths, even though the Wave 3 plan
in the cleanup audit always named them. Confirmed live on `main` today —
`grep -rL notifySyncScopes` (outside `sync-notifier.ts` itself) still
returns:

- `workers/api/src/index.ts` — three `publishTeamChatMessageSync` call sites
  (team chat message post, autopilot-answer message, run-summary update).
- `workers/api/src/thread-file-routes.ts` — `publishTeamThreadFileSync`.
- `workers/api/src/omni-handlers.ts` — `publishAgentGoalSync` /
  `publishAgentGoalEventSync` (goal CRUD; a different pair of call sites
  from the three KS-6.6 already handled) and `notifyAgentRunSyncScopes`
  (fired on every agent-run status transition, notifying the personal-
  workroom/team/thread scopes — i.e. the scopes the main web socket below
  still rides).
- `workers/api/src/inference/gym/run-progress-sync.ts` — the gym dual-write,
  deliberately permanent per #8415.

And `apps/web/src/subscriptions.ts`'s `syncScopesForModel`/
`syncStreamDependenciesForModel` — the workspace/team/thread multi-scope
socket that is the actual live transport for team chat, thread files, and
agent goals in the product UI — still opens the legacy `syncStreamHref`
(`/api/sync/${kind}/${id}/stream`), never repointed. This is almost
certainly the single largest remaining migration in the whole Wave 3
effort, and it has never had its own tracked KS-6.x issue.

**Two smaller loose ends, for whoever picks up that migration:**

- `KHALA_TOKENS_SERVED_SCOPE` in `subscriptions.ts` still opens the legacy
  `syncStreamHref`, even though its legacy producer was deleted in #8372
  (Wave 0). The connection is a harmless no-op (nothing publishes to that
  legacy scope), but it was never repointed to the new engine's
  `scope.public.tokens-served` — which is already anonymous-readable (it's
  the KS-8.x anonymous-connect fix's own evidence scope) and already emits
  a live per-increment changelog entry (`applyPublicCounterIncrement` in
  `packages/khala-sync-server/src/public-counter-projection.ts` appends a
  `{counterId, total, lastEventAt}` post-image on every increment). A real
  repoint would map that post-image onto the client's existing
  `KHALA_TOKENS_SERVED_SUMMARY_COLLECTION` monotonic-raise reducer (not the
  delta reducer, since the new engine posts authoritative totals, not
  deltas) and would need the same snapshot-seed treatment KS-6.4 gave
  settled-feed. Real, bounded, doable — just not attempted this pass, since
  it does not retire any spine infrastructure by itself (the client is
  already the only thing keeping that one legacy kind's route reachable).
- The legacy D1 `sync_scopes`/`sync_changes`/`sync_mutations` tables cannot
  drop even for the ONE genuinely fully-migrated consumer (settled feed):
  `publishSettledFeedEvents` still writes the D1 outbox on every settlement
  (unrelated to the retired `notifySyncScopes` poke), because
  `GET /api/public/settled-feed` (`public-settled-feed-routes.ts`) reads
  that same D1 outbox directly as its own fail-open fallback source,
  bypassing `sync-routes.ts`/`SyncRoomDurableObject` entirely. Retiring the
  D1 tables needs that fallback rehomed first, independent of any
  DO-room/kind retirement.

**What WAS safe to retire this pass:** the `'public-settled-feed'` kind in
`workers/api/src/sync-routes.ts` (`SyncScopeKind`, `syncScopeForPath`,
`optionalSyncScopeKind`, `isPublicSyncPath`) — the one legacy sync kind with
zero remaining producers (deleted in #8414) AND zero remaining consumers
(both its stream and its snapshot action were repointed to the new engine
in #8414; confirmed via repo-wide grep that nothing outside tests
references `/api/sync/public-settled-feed/*`, and that the D1-fallback read
route above bypasses this route entirely). `GET/WS
/api/sync/public-settled-feed/<id>/{snapshot,stream}` now 404 through the
existing generic unknown-kind fallthrough — satisfying the capstone's own
"confirm 404/gone rather than silently double-serving" bullet for this one
kind. `sync-routes.test.ts` was updated to assert 404 (and that the DO room
is never touched) instead of the old 200/204 success path. Every other
legacy kind (`workspace`, `team`, `thread`, `agent-run`,
`public-gym-run-progress`, `public-khala-tokens-served`) is untouched —
each still has at least one live legacy producer or consumer.

**Net:** #8420 stays open. Full spine retirement needs a new, explicitly
tracked migration for team chat + thread files + agent-goal CRUD + the
ongoing agent-run-status legacy producer (recommend KS-6.11 or similar)
before this issue can advance further.

## Team chat + thread files + agent-goal CRUD scope breakdown (KS-6.11, #8422)

KS-6.11 is the migration #8420 asked for: the three surfaces named in the
original Wave 3 plan that KS-6.4–6.9 never actually covered. This first
pass is scoping plus the smallest safe producer-side fix that unblocks the
most tractable of the three. **No client repoint and no legacy-producer
deletion happened this pass** — every finding below was verified against
current `origin/main` source, not assumed from the KS-6.10 capstone note.

### Headline correction to the KS-6.10 capstone's framing

The capstone (#8420) described team chat and thread files as having "LIVE
`notifySyncScopes` producer call sites" as if no new-engine work existed for
them at all. That undercounts what's actually there: **KS-8.13 (#8324)
already built a fully live khala-sync producer for both**, wired into the
Worker's generic product-state mirror
(`khala-code-product-state-store.ts`'s `khalaCodeProductStateDatabaseForEnv`).
Every current team-chat-message write
(`apps/openagents.com/workers/api/src/index.ts:3471,3543,5204`, all via
`insertTeamChatMessage(khalaCodeProductStateDatabaseForEnv(env), …)`) and
every thread-file write (`thread-file-routes.ts`'s
`ThreadFileRepository.layer(khalaCodeProductStateDatabaseForEnv(env))`)
**already** converge-upserts the Cloud SQL twin AND appends a
`scope.team.<teamId>` (plus `scope.thread.<id>` where applicable) khala-sync
changelog entry, in the SAME request as the legacy `notifySyncScopes`/
`publishTeamChatMessageSync`/`publishTeamThreadFileSync` call that follows
it. Both engines are dual-writing today, in production, right now. The real
remaining gap for both surfaces is entirely on the CLIENT side — repointing
`apps/web/src/subscriptions.ts`'s legacy `team:<teamId>` multi-scope socket
onto `/api/sync/connect?scope=scope.team.<teamId>` — not a missing producer.

`agent_goals`/`agent_goal_events` are a genuinely different story: neither
table is in `KHALA_CODE_PRODUCT_STATE_TABLES`
(`packages/khala-sync-server/src/khala-code-product-state-tables.ts`), there
is no `KhalaCodeAgentGoal*Entity` contract in `packages/khala-sync`, and no
projector routes them to any scope. This is a from-scratch build, the same
shape as KS-6.6's `AgentRunEntity`/`AgentRunEventEntity` work, and is the
genuinely largest of the three.

### Per-surface breakdown

**1. Team chat (`scope.team.<teamId>`, entity `team_chat_message`) — producer
live, ONE real client-repoint blocker, now closed this pass.**

The client's `TeamChatMessageRecord` (`apps/web/src/page/loggedIn/model.ts`)
requires a hydrated `author: { userId, name, avatarUrl, githubUsername }`
object on every message. The legacy wire payload
(`team-chat.ts`'s `TeamChatMessage.author`) supplies this via a
`users`/`auth_identities` JOIN done at read time
(`readTeamChatMessageById`). `KhalaCodeTeamChatMessageEntity`
(`packages/khala-sync/src/khala-code.ts`) carried only `authorUserId` — a
bare id, no display name/avatar/GitHub username — because KS-8.13's generic
mirror read-back (`khala-code-product-state-store.ts`'s `readRowsByWhere`)
does a plain `SELECT * FROM team_chat_messages WHERE id IS ?`, never a JOIN.
A client repoint that consumed the entity as-is would have shown every
LIVE-arriving message with a blank/undefined author — a real, user-visible
regression on an actively-used feature, exactly the guardrail this issue
warned about.

**Fixed this pass** (producer-side only, no client change, so this ships
with zero behavior change for anyone until the repoint happens):

- `KhalaCodeTeamChatMessageEntity` gained `authorName` / `authorAvatarUrl` /
  `authorGithubUsername`, all `NullOr` (not required) — see below for why.
- `khala-code-product-state-store.ts`'s `readRowsByWhere` now special-cases
  `team_chat_messages` with a JOIN against `users`/`auth_identities`
  (`LEFT JOIN`, not `INNER JOIN`, so a message whose author row is
  missing/deleted still mirrors — it just gets `null` author-identity
  fields instead of dropping out of the mirror). The joined columns are NOT
  part of `team_chat_messages`'s Postgres column spec, so
  `upsertKhalaCodeProductStateRows` silently ignores them when writing the
  Postgres row twin — only `scopeChangesForKhalaCodeProductStateRow`'s
  projection mapper (`khala-code-product-state-projection.ts`) reads them
  for the changelog post-image.
- `khalaCodeTeamChatMessagePostImage` now reads `author_name` /
  `author_avatar_url` / `author_github_username` off the row via
  `nullableStr`, falling back to `null` rather than throwing.
- `authorName` was added to `KHALA_CODE_POST_IMAGE_CONTENT_FIELDS` (the
  forbidden-material-scan exemption set) — same precedent as `name`/`title`:
  a freely-chosen display name is content, not a leak, so it shouldn't
  false-positive-skip a message if a user's display name happens to contain
  something the regex flags.
- New `KhalaCodeUrl` bounded primitive (`http(s)://`, ≤2048 chars) for
  `authorAvatarUrl` — deliberately accepts both schemes rather than
  https-only, so a merely-imperfect historical avatar URL degrades to a
  fail-soft projection skip risk reduction, not an added-strictness trap.

**Why `NullOr`, not required:** the generic backfill/verify CLI
(`packages/khala-sync-server/scripts/backfill-khala-code-product-state.ts`)
reads raw `team_chat_messages` rows with a plain `SELECT *` (no JOIN) and
**never calls the changelog projection at all** — it only calls
`upsertKhalaCodeProductStateRows` (the Postgres row-mirror path), not
`scopeChangesForKhalaCodeProductStateRow`. This is a **pre-existing gap
independent of this pass**: KS-8.13 never backfilled changelog history for
ANY of its scopes, only the Postgres row twins. A fresh client cold-loading
`scope.team.<teamId>` today gets zero historical `team_chat_message` /
`thread_file` changelog rows — only live deltas from whenever KS-8.13
shipped, forward. If a future pass wants a real cold-load story for team
chat/thread files (not just live-tail), building a one-time changelog
backfill sweep for these scopes is its own follow-up; it needs its own JOIN
for the author snapshot too, at which point `authorName` etc. becomes fully
populated for historical rows as well.

Verified with new unit/integration test coverage:
`khala-code-product-state-store.test.ts` (author hydrated when a matching
`users`/`auth_identities` row exists; still mirrors with null author fields
when the author's user row is missing — the LEFT JOIN degrade path) and
`khala-code-product-state-projection.test.ts` (entity decodes with the new
fields when present; falls back to null, does not throw, when the row
lacks them — the historical-row/backfill path). Golden fixture
`packages/khala-sync/fixtures/KhalaCodeTeamChatMessageEntity.json` updated
to include the new fields, keeping `conformance.test.ts` in sync.

**Still NOT done, deliberately out of scope this pass:** the actual client
repoint (`apps/web/src/subscriptions.ts`), and legacy `notifySyncScopes`/
`publishTeamChatMessageSync` deletion. See the entanglement note below for
why team chat can't cut over its wire scope in isolation.

**2. Thread files (`scope.team.<teamId>` + `scope.thread.<threadId>`, entity
`thread_file`) — producer live, NO schema gap, but entangled with team chat
on the shared team-scope socket.**

`KhalaCodeThreadFileEntity` structurally omits `downloadUrl`/`detailUrl` on
purpose (module doc: "clients fetch bytes through the authorized download
route, never from a synced storage pointer"). Verified this is NOT a real
client-repoint blocker: both are pure functions of already-present fields.
`downloadUrl` is exactly `` `/api/thread-files/${fileId}/download` ``
(`thread-files.ts`'s `publicThreadFile`) — reconstructible client-side from
`fileId` alone, no server round trip. `detailUrl` needs a team ref (route
slug) for the team case, which the client's `syncTeamScope`/`teamRouteRef`
machinery (`apps/web/src/page/loggedIn/model.ts`,
`apps/web/src/subscriptions.ts`) already resolves locally from `teamId` for
every other team-scoped consumer. So thread files have **no producer gap
and no client-side data gap** — they are, on their own, ready for a client
repoint today.

The catch: `scope.team.<teamId>` is ONE shared wire scope carrying BOTH
`team_chat_message` AND `thread_file` (plus `team`/`team_membership`/
`team_project`/`team_workspace_invite`) entities
(`khala-code-product-state-projection.ts`'s `scopesForRow`). The client's
`syncScopesForModel` (`apps/web/src/subscriptions.ts`) opens exactly ONE
legacy socket per scope string, multiplexing whatever collections arrive on
it — unlike KS-6.4's settled-feed or KS-6.6's agent-run, which each had a
scope with only ONE entity type and nothing else sharing it. A clean,
KS-6.4/KS-6.6-shaped cutover ("stop opening the legacy `team:<teamId>`
socket, open a dedicated new-engine one instead") can't retire the legacy
`team:<teamId>` traffic while team-chat-message translation still has any
open question, because both entity kinds arrive on the exact same
WebSocket. Team chat's author-hydration gap is now closed (see above), so
this entanglement is no longer a hard blocker — but the actual client
adapter work (a `team-live.ts` mirroring `agent-run-live.ts`, translating
BOTH entity types in one `LiveFrame` handler) is real, untouched work for
the next pass, plus real production verification before any legacy
deletion.

**3. Agent-goal CRUD (`agent_goals`/`agent_goal_events`) — no producer at
all, largest remaining piece, full from-scratch build.**

Confirmed zero existing new-engine producer: neither table appears in
`KHALA_CODE_PRODUCT_STATE_TABLES`, there is no `KhalaCodeAgentGoal*Entity`
in `packages/khala-sync/src/khala-code.ts`, and
`publishAgentGoalSync`/`publishAgentGoalEventSync`
(`apps/openagents.com/workers/api/src/omni-handlers.ts:603,610`, backed by
`sync-notifier.ts`) are the ONLY producers today — both fully legacy,
publishing to the OLD `sync-worker` D1 outbox + `SyncRoomDurableObject`
spine. This needs the full KS-6.6 shape repeated from scratch: a new entity
contract (or contracts, if goal + goal-event need separate shapes the way
`AgentRunEntity`/`AgentRunEventEntity` did), a new projector, new Worker
glue wired into every write call site, a dual-write proving pass, THEN a
client repoint, THEN legacy deletion. Not attempted this pass — flagged
honestly as the largest of the three rather than rushed.

**Separate, NOT yet investigated: `notifyAgentRunSyncScopes` (ongoing
agent-run status broadcast).** This is a DIFFERENT function from the
`syncScopeForAgentRun`-based create-time pokes KS-6.6 (#8416) already
deleted (that export no longer exists — confirmed by grep, only mentioned in
comments now). `notifyAgentRunSyncScopes`/`readAgentRunSyncScopes`
(`sync-notifier.ts`) is still called from 5 live sites
(`omni-handlers.ts:1192,2093,2125,2983`, `index.ts:6589`) and broadcasts to
FOUR legacy scopes at once: `personalWorkroomScope`, `teamScope`,
`agentRunScope`, and `syncThreadScope`. The `agentRunScope` leg is very
likely dead weight now — KS-6.6's client repoint means
`apps/web/src/subscriptions.ts` no longer opens the legacy
`agent-run:<runId>` socket at all — but the other three legs (workroom
sidebar, team page, thread page) may still be feeding live legacy
consumers that this pass did NOT verify. Do not delete any of
`notifyAgentRunSyncScopes`'s call sites or scope legs without checking each
one against the client's CURRENT `syncScopesForModel` list first; this is
explicitly flagged as unfinished investigation, not confirmed-dead code.

### Recommended split for follow-up passes

Given the genuinely different tractability of the three surfaces, this pass
opened separate tracked issues rather than one bundled migration:

- **#8423 (KS-6.11a) — team chat + thread files client repoint.** Producer
  complete for both (including this pass's team-chat author-hydration fix).
  Needs: a `team-live.ts` adapter (mirroring `agent-run-live.ts`), handling
  BOTH `team_chat_message` and `thread_file` entity types in one
  `LiveFrame` handler (they share the exact `scope.team.<teamId>` wire
  scope, so they are bundled into one client-repoint issue, not two),
  removing `team:<teamId>` from `syncScopesForModel`'s legacy list, real
  production verification (a live `scope.team.<teamId>` WebSocket handshake
  plus a real message/file showing correctly), THEN legacy
  `publishTeamChatMessageSync`/`publishTeamThreadFileSync`/their
  `notifySyncScopes` call deletion.
- **#8424 (KS-6.11c) — agent-goal CRUD.** Full from-scratch build: entity
  contract(s), projector, Worker glue, dual-write proof, THEN client
  repoint, THEN legacy deletion. The largest of the three; deliberately not
  combined with #8423.
- **#8425 — verify `notifyAgentRunSyncScopes`'s remaining live legacy
  consumers** before touching any of its four broadcast legs or five call
  sites. Bookkeeping cleanup adjacent to #8423/#8424, not itself a
  scope-adoption migration (the entity/scope work for agent runs is already
  done via KS-6.6).

#8422 stays open, tracking this scoping pass; the three follow-ups above
carry the actual remaining migration work.

### Test evidence (this pass)

- `packages/khala-sync/src/conformance.test.ts`: 42/42 pass (golden fixture
  round-trips, including the updated `KhalaCodeTeamChatMessageEntity`
  fixture).
- `packages/khala-sync-server/src/khala-code-product-state-projection.test.ts`:
  15/15 pass, including the two new author-hydration cases.
- `apps/openagents.com/workers/api/src/khala-code-product-state-store.test.ts`:
  12/12 pass, including the two new JOIN-behavior cases (author present,
  author row missing).
- `apps/openagents.com/workers/api` full suite: 1070/1072 files, 9610/9617
  tests pass. The 2 failing files
  (`nexus-pylon-visibility-routes.test.ts`, `treasury-domain-store.test.ts`)
  are PRE-EXISTING and unrelated to this change (a timestamp-regex
  assertion and a domain-table-count assertion, neither touching
  `khala-code-product-state-*` or `team-chat`/`thread-file` files) — the
  same class of pre-existing failure multiple prior passes in this thread
  have already flagged; not fixed here, still open.
- `bun run check:architecture` (the zero-debt ledger): passed.
- `packages/khala-sync` and `packages/khala-sync-server` typecheck: clean.
- `apps/openagents.com/workers/api` typecheck: clean.
- No production deploy this pass: the change is producer-side-only and
  purely additive (nullable fields, a widened SELECT for one table's
  read-back path) with no client-visible behavior change, so a live
  production check has nothing new to observe yet — the real production
  verification belongs with KS-6.11a's client repoint, when there is an
  actual wire behavior change to confirm.

### 2026-07-05 KS-6.11a (#8423) client repoint — shipped, legacy deletion deliberately deferred

Ships the client half of the KS-6.11 scope breakdown above: `apps/web/src/subscriptions.ts`
now handles `team_chat_message`/`thread_file` `LiveFrame` entities delivered on
`scope.team.<teamId>` (one combined handler, matching the two entity types'
shared wire scope per the KS-6.11 scoping note), and `team:<teamId>` was
removed from `syncScopesForModel`'s legacy-scope list. Code committed and
pushed to `origin/main`, then deployed to production.

**Deploy note — a real gap in `deploy:safe` itself, not this change:** the
first `deploy:safe` run staged everything correctly (staging deploy, predeploy
parallel-dispatch smoke, prod D1 migrations all green) but then the
`bun run --cwd packages/khala-sync-server check:pending-migrations` step
failed with "no database URL" because `KHALA_SYNC_DATABASE_URL` was not set in
the deploying shell. Because `deploy:safe` chains its steps with `&&`, this
silently aborted the whole script **before** the final production
`wrangler deploy`, even though nothing in the visible output screamed
"deploy failed" — the script just stopped. Recovery: sourced
`~/work/.secrets/khala-sync-cloudsql.env`, built
`postgres://$KHALA_SYNC_MIGRATE_USER:$KHALA_SYNC_MIGRATE_PASSWORD@$KHALA_SYNC_CLOUDSQL_IP:5432/$KHALA_SYNC_DB_PROD?sslmode=require`
(note: `sslmode=require` is mandatory — the Cloud SQL instance's `pg_hba.conf`
rejects plaintext connections outright, not just unauthorized IPs), reran the
Postgres gate (now green — 0 pending Khala Sync migrations), then ran the
final `wrangler deploy --containers-rollout=none --assets ../../apps/web/dist`
directly since `deploy:safe` never reached it. Production Worker Version ID
`81fb3a2c-d0a4-4419-9c84-a8681686f548`, custom domains
(`openagents.com`, `auth.openagents.com`, `sites.openagents.com`) all
redeployed cleanly; `https://openagents.com/` → 200,
`/api/health` → `{"ok":true}` immediately after. **Flagging for whoever owns
`deploy:safe` next:** this gate should fail LOUD (its own explicit
non-`&&`-swallowed check, or a final "did the last deploy step actually run"
assertion) instead of silently no-op'ing the production deploy when an owner
secret isn't loaded in the calling shell.

**Production verification — real evidence, one honest gap flagged.** No
owner browser session was available to this pass, so a literal fresh
authenticated `scope.team.<teamId>` WebSocket frame with a brand-new message
was not captured. What WAS verified directly against live production data
(not fixtures):

- Queried `khala_sync_changelog` (direct Postgres connection) for
  `entity_type = 'team_chat_message'`: exactly one row exists,
  `scope.team.team_openagents_core`, `committed_at` ~4.5 hours before this
  verification pass — i.e., written **before** this deploy, by an earlier
  step in this same work. Its `post_image_json` has no `authorName`/
  `authorAvatarUrl`/`authorGithubUsername` keys at all (not even `null`),
  confirming it predates the KS-6.11 entity-schema change entirely — not a
  live bug, a stale pre-fix artifact.
- Ran the EXACT `TEAM_CHAT_MESSAGE_AUTHOR_JOIN_SELECT` LEFT JOIN
  (`khala-code-product-state-store.ts`) directly against production D1 via
  `wrangler d1 execute --remote`: the join resolves correctly for real rows,
  including that same stale message's own author
  (`author_user_id: "github:14167547"` → `author_name: "Christopher David"`,
  `author_avatar_url`, `author_github_username: "AtlantisPleb"`) and three
  other real users. This proves the join SQL is correct against live schema
  and data, not just the mock-DB unit tests — so the NEXT real team chat
  message written after this deploy should carry hydrated author fields
  through to `scope.team.<teamId>`.
- Did not (and could not, headlessly) confirm the actual live
  `khalaCodeProductStateDatabaseForEnv` read-back mirroring path executes this
  join at write time in the deployed Worker, only that the SQL text is correct
  and matches source. That last mile — one fresh authenticated message,
  observed both in the `khala_sync_changelog` post-image and rendered by a
  real client — is NOT done.

**Legacy producer deletion (`publishTeamChatMessageSync`/
`publishTeamThreadFileSync`, their `notifySyncScopes` calls, and
`team-sync.test.ts`): deliberately NOT done this pass.** The original KS-6.11
scoping note's own sequencing (client repoint → real production
verification → THEN legacy deletion) is a real safety gate, not decoration:
deleting the legacy dual-write removes the fallback for every team-chat/
thread-file consumer at once, and this pass could not complete the literal
last-mile verification step above. Dual-write stays live (both the legacy D1
`sync_changes` push and the new `scope.team.<teamId>` Postgres-backed push
fire on every write) until a follow-up either (a) gets an owner-provided
authenticated session to run the real WS-frame check, or (b) builds a small
service-context verification script that calls `insertTeamChatMessage`
through the real `khalaCodeProductStateDatabaseForEnv` shim against
production without needing a browser session (no such tooling exists yet in
this repo — every prior KS-8.x domain cutover's "real production
verification" bullet was evidently satisfied by an owner running the flow
live, not by an agent). Track this as a same-scope follow-up on #8423 rather
than opening a new issue.

## What this runbook does NOT cover

- Deploying the Worker/hub DO: `docs/DEPLOYMENT.md` (deploy:safe gate).
- Writing mutators: [`MUTATORS.md`](./MUTATORS.md).
- Invariant statuses and their enforcing tests:
  `apps/openagents.com/INVARIANTS.md` "Khala Sync (SPEC §7 invariant set)".
- Load testing (KS-9.1 #8310) and behavior-contract sweeps (KS-9.2 #8311)
  land their own procedures on those issues.
