# Session handoff — Khala Code mobile + D1 evacuation (2026-07-07)

Long session; hard-stopped on session limits. This is the synthesis + exact next steps.
Collect the 4 stopped subagents' final reports (in the task transcripts) alongside this.

## TL;DR live state
- **Mobile GitHub login WORKS** (new + existing accounts). **$10 signup credit works** (re-login grants it).
  Personalized greeting, navy background, fixed cyan buttons — all shipped or committed.
- **D1 evacuation ~80% done**: reads cut for ~all domains; writes cut for 7 domains; money-writes +
  sites/crm + a few reads remain before the `OPENAGENTS_DB` binding can be dropped. d1-http 401s ≈ 25/3min (was ~100).
- Monolith is on **Google Cloud Run** (`openagents-monolith`), DB via **Cloud SQL connector** (public ingress CLOSED).

## Build / runtime facts (READ FIRST)
- iOS app: **build 17** on TestFlight, runtime fingerprint **`a60ce3456ef97f7a3b0ed52a5063da5ded5de106`**.
- HEAD's fingerprint is now **`bbaec6d4…`** (a "storybook" commit shifted native deps AFTER build 17).
  All current fixes are JS/asset-only → OTA-compatible with build 17, so **OTAs MUST be seeded under `a60ce345`**,
  NOT the computed HEAD fingerprint. Replicate OTA revs 00076/00077: run publish-ota.sh's export/config steps,
  then `apps/oa-updates/scripts/deploy-cloudrun.sh` with `OA_SEED_RUNTIME=a60ce3456ef97f7a3b0ed52a5063da5ded5de106`.
- **Next native build cut from HEAD will be runtime `bbaec6d4`** → future OTAs target that once a new TestFlight build ships.

## Deploy process (the main checkout is PERPETUALLY DIRTY from concurrent agents)
- **Monolith deploy**: from a CLEAN worktree at origin/main:
  `git worktree add --detach /tmp/oa-deploy origin/main && cd /tmp/oa-deploy && bun install && \
   bash apps/openagents.com/workers/api/scripts/deploy-cloudrun.sh production --with-scheduler` → then `git worktree remove`.
  The Cloud SQL connector env (PGHOST/PGUSER/PGPASSWORD + `--add-cloudsql-instances`) is baked into wrangler.jsonc +
  the deploy script, so source deploys preserve it. Verify after: `/internal/khala-sync/db-smoke` (admin bearer) → `{ok:true, khalaSyncTables:20}`.
- **Migrations**: `cloud-sql-proxy openagentsgemini:us-central1:khala-sync-pg --port <p> --token "$(gcloud auth print-access-token)"`
  (ADC is broken — you MUST pass `--token`), then `bun packages/khala-sync-server/scripts/migrate.ts` with
  `KHALA_SYNC_DATABASE_URL=postgresql://<migrate-user>:<pw>@127.0.0.1:<p>/khala_sync_<env>?sslmode=require` (via proxy: sslmode=disable).
  Creds: `.secrets/khala-sync-cloudsql.env`. **Latest applied migration: 0047.** Apply to staging THEN prod.
- **Mobile OTA**: see build facts above — force `OA_SEED_RUNTIME=a60ce345…`.

## D1 evacuation (#8515) — precise state
Reads: cut to Postgres for ~all domains (flags in `apps/openagents.com/workers/api/wrangler.jsonc` prod `vars`).
WRITES cut to Postgres (flag `KHALA_SYNC_<DOMAIN>_WRITES=postgres`): **agent-credentials, agent-runtime,
forge-git-canonical, gym-evals, pylon (+spark-payout), supervision, training**; plus **forum content, event-ledger,
entitlements-gate, github-signup-grant** (all deployed).

WRITES still on DEAD D1 (remaining work to drop the binding):
1. **Money audit/ledger writes**: billing, ledger (`token_usage_events` — the public tokens-served counter source),
   treasury (27 tables/6 crons), artanis, business, forum-money. A subagent (task `a1468147dfddadc67`) was mid-cutover
   at stop — READ ITS HANDOFF for which committed vs reverted. These are audit tables whose writes currently FAIL
   silently (data loss); cutting to Postgres resumes them. **Customer credits/pay_ins/agent_balances are ALREADY
   Postgres-authoritative — DO NOT touch that path.** Verify Postgres-internal invariants (no orphans) before flipping.
2. **Sites + CRM**: SKIPPED deliberately — their write handles carry secret columns (`site_environment_values.plain_value`,
   CRM token hashes) that a raw `makePostgresD1Database` swap would LEAK to Postgres (the mirror redacts them, the adapter
   doesn't). Needs redaction-aware authority-path work before cutting.
3. `approveClaim` + `openauth_agent_links` (auth-linking seam; coordinate with mobile-auth).
4. Activity-timeline cron sources; `tassadar-run-summary` read (still 500 — a public stats route, low priority).

TOOLS to reuse for the remaining cuts:
- `apps/openagents.com/workers/api/src/postgres-d1-adapter.ts` — `makePostgresD1Database` + dialect translator
  (`json_extract`/`datetime('now')`/`julianday`/`strftime('%s')`/`INSERT OR REPLACE`-with-explicit-ON-CONFLICT).
  Still throws on: non-`%s` strftime, relative datetime modifiers, `?1/?2` numbered placeholders (`business-factory-metrics.ts`).
- `apps/openagents.com/workers/api/src/khala-sync-domain-writes-database.ts` — `makeKhalaSyncWritesDatabase(env)` +
  `parseKhalaSyncWritesMode` + the `KHALA_SYNC_<DOMAIN>_WRITES` flag pattern (default postgres, `d1` rolls back).

BINDING-REMOVAL PLAN (do ONLY after logs show zero live D1 traffic):
1. `cloudrun/env.ts`: remove `OPENAGENTS_DB: d1FromProcessEnv(...)` + its import; delete the bridge `cloudrun/d1-http.ts`
   + its test refs. 2. `runtime.ts` `openAgentsDatabase` + `bindings.ts` `OPENAGENTS_DB` field. 3. wrangler `d1_databases`
   + `SYNC_ROOM`/`KHALA_SYNC_HUB`/`EVENT_LEDGER_OWNER` DO bindings (already shimmed unavailable in `cloudrun/do-shims.ts`).
   Then every remaining `OPENAGENTS_DB`/`openAgentsDatabase()` ref is dead code.

## Open issues
- **#8515** (Cloudflare→GCP): in progress — money writes + sites/crm redaction + binding drop remain (above).
- **#8467** (mobile MVP): login ✅, credits ✅, greeting ✅. Needs one real end-to-end coding task verified on device.
- **#8510 / #8506** (Maestro smoke): task `abe969e44a01f56c0` was minting AgentFlampy's token + running the Maestro flow.
  READ ITS HANDOFF for AgentFlampy's `user_id`, how the `oa_agent_` token was minted/verified, the seeded thread title,
  the `.secrets/` file path, and the Maestro result. Close #8510 (then #8506, its last child) on a green Maestro run.
- **#8503 / #8477** (Agent Computers): the Firecracker-on-GCE substrate is **PROVEN** (real microVM boots + runs code +
  egress + copy-out + reclaim on host `agent-computer-gce-1`). Task `a18ae8e967c0a28e9` was building the 3 remaining
  increments: (a) baked agent rootfs (Ubuntu+git+bun+#8473 executor+guest agent), (b) `guest_exec`/`guest_copy_out`
  transport in `cloud/crates/oa-codex-control/src/cloud_vm.rs` over the proven ssh-on-tap bridge, (c) `/v1/placement`
  path that boots the microVM (today it boots a full GCE VM). READ ITS HANDOFF for artifact paths on the host + next command.
  DoD = one real coding turn inside a microVM with the receipt bundle. #8477 (branch/PR writeback) code is done; it closes
  when #8503's executor calls it from a real run.

## Infra / access
- Cloud SQL: `khala-sync-pg` (us-central1-a), public ingress CLOSED (connector only). Proxy with `--token`.
  DBs `khala_sync_prod` / `khala_sync_staging`. Creds `.secrets/khala-sync-cloudsql.env`.
- Agent Computer host: `agent-computer-gce-1` (us-central1-a), Firecracker v1.16.1 + jailer installed;
  kernel `vmlinux-5.10.223` + baseline rootfs staged at `/srv/openagents/cloud-vm/`.
- Admin bearer: Secret Manager `openagents-monolith-admin-token-prod`. Project `openagentsgemini`, gcloud authed as `chris@openagents.com`.

## Immediate next-agent checklist
1. **Publish the pending settings OTA** (commit `57dcd942a4`: visible Sign out + Delete account moved to bottom):
   export/config + `deploy-cloudrun.sh` with `OA_SEED_RUNTIME=a60ce345…` (see build facts). No monolith redeploy.
2. Read the 4 stopped agents' final reports; **deploy any committed money-write cutovers** (clean worktree) + verify
   money audit writes land in Postgres (proxy + `psql`).
3. Do sites/crm with redaction-aware mirror; fix the `tassadar-run-summary` 500.
4. When zero live `OPENAGENTS_DB` refs remain → drop the binding + CF DO bindings (plan above).
5. Continue #8503 microVM build per task `a18ae8e967c0a28e9`'s handoff; close #8510 per task `abe969e44a01f56c0`'s handoff.
