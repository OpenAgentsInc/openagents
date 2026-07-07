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
1. **Money audit/ledger writes** (task `a1468147dfddadc67`, DONE): **LEDGER (`token_usage_events`, the public
   tokens-served counter source) IS NOW CUT** — flag `KHALA_SYNC_LEDGER_WRITES=postgres`, commit `f79de61806`,
   Postgres parity verified (311,783 rows, 0 dupes/nulls/negatives, PK+UNIQUE+CHECK constraints match D1). Writes had
   been dead since 2026-07-06 13:30 UTC → this RESUMES lost capture. **DEPLOYED by parent this session**: monolith
   Cloud Run revision `openagents-monolith-00028-9cm` (100% traffic, `/internal/healthz` green), flag armed. VERIFY
   STATUS: Postgres-internal parity confirmed pre-flip; end-to-end confirmed by the first organic `token_usage_events`
   row with `observed_at` after 2026-07-07 16:04Z (the serving fleet had produced no rows for ~26.5h, so the first
   post-deploy inference is the proof — re-check `max(observed_at)` via the proxy). Rollback if a dialect throw appears
   in logs: set `KHALA_SYNC_LEDGER_WRITES=d1` in wrangler + redeploy (but D1 is dead, so d1 = the same data loss). The remaining money domains were NOT cut (each has a clean lever documented — customer
   credits/pay_ins/agent_balances are ALREADY Postgres-authoritative; DO NOT touch that path):
   - **Treasury** (`treasury-domain-store.ts:866 const d1 = openAgentsDatabase(env)`) — single clean handle; swap to
     `makeKhalaSyncWritesDatabase(env)` gated on `KHALA_SYNC_TREASURY_WRITES`, disable the redundant mirror on that
     path, verify 27-table twin-schema + txn semantics, add the wrangler flag. (`_READS=postgres` already set.)
   - **Artanis** (`artanis-domain-store.ts:901 const d1 = options.d1 ?? openAgentsDatabase(env)`) — same recipe +
     `KHALA_SYNC_ARTANIS_WRITES`. `artanis-fleet-overseer-tick.ts` uses `json_extract` (now adapter-translatable).
   - **Business** (`business-domain-store.ts:1054`) — same lever + `KHALA_SYNC_BUSINESS_WRITES`, BUT
     `business-factory-metrics.ts` uses `?1/?2` numbered placeholders + relative-datetime modifiers → the generic
     adapter THROWS on those; handle that module per-call-site, not through the adapter.
   - **Billing** (`billing-store.ts`/`billing-routes.ts`) — **NOT a native write store**: `makePostgresBillingStore`
     is a converge/mirror + reads store. Billing is D1-authoritative + best-effort mirror via `BillingRuntime.mirror`;
     real writes are scattered across `billing.ts`, `stripe-billing.ts`, pay-ins, buyer-ledger, paid-plan-intents,
     `billing-routes.ts` (9 `openAgentsDatabase(` sites). No single handle to swap — decide per-writer, verifying each
     statement's dialect. `billing_ledger_entries` feeds the balance SUM (reads already Postgres) so its dead writes
     mean balance mutations are currently LOST — real, but needs careful per-writer surgery. Do NOT assume ledger's
     "native store" shape here.
   - **Forum money/labor** (`forum/tip-earnings.ts`, `forum/repository.ts`) — 0 direct `openAgentsDatabase(`; money
     writes route through the forum-domain handle. Trace how tip-earnings writes reach D1 and cut only the money/labor
     write statements; don't conflict with the already-Postgres `forum-postgres-serving.ts` content path.
   Binding reality: **393** live `openAgentsDatabase(` call sites remain in non-test `workers/api/src` — the binding is
   far from droppable; `OPENAGENTS_DB` still declared in wrangler prod line 519 / staging line 796.
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
- **#8510 / #8506** (Maestro smoke): **NOT closed yet — Maestro is GREEN, 3 code steps remain to honestly close.**
  Task `abe969e44a01f56c0` (DONE) proved `SignedInThreadSmoke` GREEN on a **Release** iPhone-17-Pro sim, auto-signed-in
  as AgentFlampy (two passing runs). Committed+pushed `cd3122682c` (fixed flow + runner
  `clients/khala-mobile/scripts/signed-in-thread-smoke-run.sh` + receipt `docs/khala-mobile/2026-07-07-signed-in-thread-smoke-receipt.md`).
  Facts: AgentFlampy user id **`github:300914913`** (id format is `github:<id>`, NOT `user_…`); token minted via
  `POST /api/agents/register` then linked with `UPDATE agent_credentials SET openauth_user_id='github:300914913' …`
  (the app's owner-claim/`linkOpenAuthAgent` route writes D1 not Postgres, so the direct Postgres update is the correct
  path under the #8515 split); creds in `~/work/.secrets/khala-maestro.env` (gitignored, never committed). Seeded thread
  `Maestro smoke thread` (`scope.thread.maestro-smoke-thread-20260707`).
  **The 3 steps to close** (from the agent's report — do these then close #8510, then #8506 its last child):
  1. Promote `khala_mobile.platform.launched_app_interaction_smoke.v1` in
     `clients/khala-mobile/src/contracts/ux-contracts.ts` to `state:"enforced"`, `enforcementTier:"nightly"` + ONE oracle.
     Constraint: `tests/ux-contracts.test.ts` requires an enforced oracle to be `kind:"bun-test"`/`"qa-scenario"` that
     resolves AND contains the contractId string (a `visual-smoke` oracle → `skipped_kind` → fails). So add a new bun-test
     (e.g. `clients/khala-mobile/tests/signed-in-thread-smoke-receipt.test.ts`) referencing the contractId + asserting the
     receipt doc records PASS; point the oracle `ref` at it; add receipt to `evidenceRefs`; drop
     `blocker.khala_mobile.needs_seeded_public_safe_test_github_account`. Run `bun test clients/khala-mobile/tests/ux-contracts.test.ts`.
     Keep the statement honest: this is a Release **simulator** run (Android already has a real-emulator receipt).
  2. Add a first mobile row to `docs/qa/khala-code-nightly-matrix.md` + a `scripts/qa-nightly-matrix.ts` step invoking
     `clients/khala-mobile/scripts/signed-in-thread-smoke-run.sh` (precondition: booted sim + installed Release build).
  3. Commit to main (clean worktree), close #8510 citing the receipt + `cd3122682c`, close #8506 cross-referencing #8510.
  Real-behavior finding: sending starts a `runtime_turn` on `hosted_khala`; no Pylon services this test account so it stays
  `queued` → composer flips to Steer/Queue and the message's optimistic overlay clears (the "shows briefly then reverts to
  No messages yet" the owner saw — the message DOES persist and renders on re-open). The runner closes active turns first.
- **#8503 / #8477** (Agent Computers): the Firecracker-on-GCE substrate is **PROVEN**, and task `a18ae8e967c0a28e9` (DONE)
  got materially further: **the microVM boots from a baked image, the vsock guest agent comes up, and the real #8475-based
  turn-runner executes inside the microVM.** ONE blocker remains before a green in-guest checkout: **in-guest egress**
  (image enables `systemd-networkd` with no config, clearing the kernel `ip=` on eth0 → git fetch `fetch_failed`).
  Commits pushed: openagents `e79571071a` (`apps/pylon/deploy/agent-computer/turn-runner.ts` + manifest w/ real digests),
  cloud `3a8b5bd` (CND-056 progress). Baked image `/srv/openagents/cloud-vm/agent-computer-rootfs.ext4` (577M, sha256
  `3e612f6f…a63cf0b`; git+bun+python3+vsock guest-agent `agent-guest.service`+turn-runner), kernel `/srv/openagents/cloud-vm/vmlinux`.
  **EXACT next commands** (from the agent's report):
  1. Fix egress: mount the ext4 loopback, `systemctl disable`+`mask systemd-networkd` in chroot, write `/etc/resolv.conf`
     `nameserver 8.8.8.8`, `umount`, `e2fsck -fy`. (Full one-liner in the agent's task output.)
  2. Re-run `sudo python3 /tmp/vsock-turn-proof.py` — success = `TURN-PROOF-RESULT: PASS` with `baseCommit` = the pinned
     commit + `result.json` extracted microVM→host (a real coding turn: checkout + staged diff + events + reclaim). Re-pin the new image sha into the manifest.
  3. Increment 2: port the vsock protocol (`/tmp/vsock-turn-proof.py`, local copy in scratchpad) into
     `cloud/crates/oa-codex-control/src/cloud_vm.rs` `guest_exec`/`guest_copy_out`/`wait_guest_ready` (run firecracker
     directly, not jailer, for a predictable UDS path); then `cargo test -p oa-codex-control live_cloud_vm_session… -- --ignored`.
  4. Increment 3: add the agent-computer placement path to `/v1/placement` (today it binds a full-GCE Codex runner lane) + arm staging.
  DoD gaps: turn-runner does a deterministic coding step, NOT an LLM turn — a real model-token receipt
  (`/api/khala/cloud/runtime-turn-usage`) needs a Codex/Claude login baked in or the hosted Khala gateway. #8477
  (branch/PR writeback) code is done; it closes when #8503's executor calls it from a real run.
  **⚠️ POSTURE REGRESSION TO REVERT** (the agent added this to escape flaky IAP): host `agent-computer-gce-1` has a
  temporary external IP + firewall rule `agent-computer-tmp-ssh` (tcp:22 from a single `/32`). It's narrow (one owner IP,
  one port) so it's left in place ONLY so the next microVM agent keeps SSH; **revert the moment #8503 lands**:
  `gcloud compute firewall-rules delete agent-computer-tmp-ssh --project openagentsgemini` and
  `gcloud compute instances delete-access-config agent-computer-gce-1 --zone us-central1-a --project openagentsgemini`.

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
