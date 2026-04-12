# Nexus GCP Runbook

Date context: March 6, 2026.

Issue tracking:
- Deployment config: https://github.com/OpenAgentsInc/openagents/issues/3047
- Staging validation: https://github.com/OpenAgentsInc/openagents/issues/3048
- Production cutover: https://github.com/OpenAgentsInc/openagents/issues/3049
- Legacy surface retirement: https://github.com/OpenAgentsInc/openagents/issues/3050
- Hardening + maintenance ops: https://github.com/OpenAgentsInc/openagents/issues/3051

## 1) Hosting decision

The first real Nexus hosting path is:

- `Compute Engine VM`
- `persistent SSD`
- `SQLite-backed durable relay store`
- one public Nexus Rust service in the container itself

Why this path:

- the imported durable relay is SQLite-first today
- the old Cloud Run-style stateless model is the wrong fit for a durable relay
- this keeps Nexus as one Rust service while giving it a stateful disk and restart persistence

Current deployment decision:

- keep relay and authority in one deployed Nexus service
- do not split them into separate deployed services unless later operational pressure justifies it

This runbook does **not** claim that public DNS/TLS cutover is complete. That is handled in later staging/cutover work. The purpose here is to make the deployed runtime itself stateful and durable.

## 2) Baseline assumptions

- project: `openagentsgemini`
- region / zone: `us-central1` / `us-central1-a`
- VPC / subnet: `oa-lightning` / `oa-lightning-us-central1`
- VM: `nexus-mainnet-1`
- data disk: `nexus-relay-data-mainnet`
- default public host assumption: `nexus.openagents.com`
- default websocket assumption: `wss://nexus.openagents.com/`

The scripts are parameterized through env vars in `scripts/deploy/nexus/common.sh`.

## 3) Scripted deployment flow

All scripts are in `scripts/deploy/nexus/`.

1. Build and push the Nexus image.

```bash
scripts/deploy/nexus/01-build-and-push-image.sh
```

2. Provision the baseline VM, service account, persistent disk, and IAP SSH access.

```bash
scripts/deploy/nexus/02-provision-baseline.sh
```

3. Configure the VM, mount the disk, and start the Nexus service.

```bash
scripts/deploy/nexus/03-configure-and-start.sh
```

`03-configure-and-start.sh` now also installs the treasury continuity watchdog
by default and runs a post-restart payout smoke check before it leaves a new
image in production. If the fresh image fails to emit completed payout sends
inside the configured smoke window, the script automatically rolls back to the
previous image. If you need to install or refresh only the watchdog without
redeploying the Nexus container:

```bash
scripts/deploy/nexus/10-install-treasury-watchdog.sh
```

4. Verify health and emit a deploy receipt.

```bash
scripts/deploy/nexus/04-verify-gates.sh
```

The verifier is now a real gate, not just a receipt dumper. By default it fails
the rollout if:

- the VM or `nexus-relay` systemd service is not healthy
- `/healthz`, `/api/stats`, or `/v1/treasury/status` latency regresses past the
  configured thresholds
- `/api/training/rollout` latency regresses past the configured threshold or
  the rollout-policy snapshot cannot be captured in the deploy receipt
- repeated local-origin probes show bad tail latency on `/healthz`,
  `/api/stats`, or `/api/provider-presence/heartbeat?dry_run=true`
- treasury policy on the live status surface drifts from
  `/etc/nexus-relay/nexus-relay.env`
- treasury snapshot freshness or wallet-sync freshness crosses the configured
  threshold
- treasury continuity exposes active critical alerts such as dispatch stalls,
  confirmation stalls, budget-cap exhaustion, or blocked policy runtime

Optional local threshold overrides:

```bash
VERIFY_HEALTH_LATENCY_MAX_MS=1000 \
VERIFY_STATS_LATENCY_MAX_MS=1000 \
VERIFY_TREASURY_LATENCY_MAX_MS=1000 \
VERIFY_LATENCY_SAMPLE_COUNT=40 \
VERIFY_HEALTH_LATENCY_P95_MAX_MS=1000 \
VERIFY_HEALTH_LATENCY_P99_MAX_MS=2000 \
VERIFY_STATS_LATENCY_P95_MAX_MS=1000 \
VERIFY_STATS_LATENCY_P99_MAX_MS=2000 \
VERIFY_TRAINING_ROLLOUT_LATENCY_MAX_MS=1000 \
VERIFY_PROVIDER_PRESENCE_LATENCY_P95_MAX_MS=1000 \
VERIFY_PROVIDER_PRESENCE_LATENCY_P99_MAX_MS=2000 \
VERIFY_TREASURY_SNAPSHOT_MAX_AGE_MS=15000 \
VERIFY_TREASURY_WALLET_SYNC_MAX_LAG_MS=15000 \
scripts/deploy/nexus/04-verify-gates.sh
```

The provider-presence probe now uses `dry_run=true` so deploy verification hits
the real heartbeat handler without polluting live public pylon counts.

The deploy receipt also captures the current `/api/training/rollout` policy
snapshot so operators can see the active rollout revision, pause state, cohort
count, and blocked build or release breakers in the same artifact as the
latency gates.

Before crowd expansion, treat
`docs/plans/transcript-222-training-launch-slos.md` as the normative threshold
sheet for those gates and
`docs/plans/transcript-222-training-incident-taxonomy.md` as the containment
taxonomy when one of them breaks.

## 4) Runtime model

The deployed service is the `nexus-relay` container built from `apps/nexus-relay/Dockerfile`.

It runs:

- durable relay storage under `${NEXUS_DATA_DIR}` (`/var/lib/nexus-relay` by default)
- the in-process authority/API routes merged into the same service
- receipt persistence through `NEXUS_CONTROL_RECEIPT_LOG_PATH`
- treasury wallet + payout state on the same persistent disk when
  `NEXUS_CONTROL_TREASURY_ENABLED=true`

The baseline bind is `0.0.0.0:8080` on the VM. Public DNS/TLS exposure is a later step; the app/runtime no longer depends on ephemeral in-memory relay storage.

Treasury deployment note:

- do not rely on the repo-relative treasury defaults in production
- the supported Breez Spark SDK floor for treasury is now `0.12.2`
- do not redeploy or roll back production Nexus onto the older `0.6.6` Spark
  pin; that version can report `0 sats` after backend enum drift even when the
  wallet still has funds
- `scripts/deploy/nexus/03-configure-and-start.sh` now writes
  `NEXUS_CONTROL_TREASURY_STATE_PATH`,
  `NEXUS_CONTROL_TREASURY_WALLET_MNEMONIC_PATH`, and
  `NEXUS_CONTROL_TREASURY_WALLET_STORAGE_DIR`
  onto `${NEXUS_DATA_DIR}/treasury/...` so the central wallet survives restarts
- `scripts/deploy/nexus/03-configure-and-start.sh` now refuses to deploy
  `nexus-mainnet-1` with `treasury_enabled=false` or `payout_sats_per_window=0`
  unless `NEXUS_ALLOW_ZERO_TREASURY_IN_PRODUCTION=true` is set explicitly
- `scripts/deploy/nexus/03-configure-and-start.sh` preserves the live
  `NEXUS_CONTROL_TREASURY_*` values from `/etc/nexus-relay/nexus-relay.env`
  unless you explicitly export replacements before redeploying
- set payout policy via env before running `03-configure-and-start.sh`, for example:
  also set the runtime wallet refresh and send-concurrency envs explicitly in
  production so payout cadence does not degrade as the eligible target count
  grows:

```bash
export NEXUS_CONTROL_TREASURY_ENABLED=true
export NEXUS_CONTROL_TREASURY_PAYOUT_SATS_PER_WINDOW=25
export NEXUS_CONTROL_TREASURY_PAYOUT_INTERVAL_SECONDS=600
export NEXUS_CONTROL_TREASURY_REQUIRE_SELLABLE=true
export NEXUS_CONTROL_TREASURY_DAILY_BUDGET_CAP_SATS=1000000
export NEXUS_CONTROL_TREASURY_WALLET_STATUS_REFRESH_SECONDS=30
export NEXUS_CONTROL_TREASURY_MAX_CONCURRENT_SENDS=16
```

Why the extra two envs matter:

- `NEXUS_CONTROL_TREASURY_WALLET_STATUS_REFRESH_SECONDS=30` keeps the wallet
  refresh loop on a lighter background cadence in production; treasury stale
  detection now tracks that configured refresh budget and the stats refresh
  path uses cached wallet balance plus bounded recent payment history instead
  of forcing a full Spark sync or full wallet-history walk every cycle
- if `${NEXUS_CONTROL_TREASURY_STATE_PATH}` picks up a malformed cached
  snapshot during a restart, the runtime now attempts to recover from the
  remaining state and at minimum preserves the persisted payout total rather
  than silently zeroing the public counter
- `NEXUS_CONTROL_TREASURY_PAYOUT_SATS_PER_WINDOW=25` and
  `NEXUS_CONTROL_TREASURY_PAYOUT_INTERVAL_SECONDS=600` are the current
  production-safe reference values for the hosted Nexus treasury. That policy
  reduces Spark transfer pressure to `0.4` sends/second even if the eligible
  set reaches `240` providers, while keeping the daily ceiling under
  `864000 sats` at that scale.
- `NEXUS_CONTROL_TREASURY_MAX_CONCURRENT_SENDS=16` remains sufficient at that
  wider cadence because the staggered per-identity phase offsets keep the due
  set small per dispatch cycle.
- `scripts/deploy/nexus/10-install-treasury-watchdog.sh` installs a systemd
  timer on the VM that runs every 5 minutes, checks the local treasury status
  plus recent completed-send journal entries, and restarts `nexus-relay` only
  when payouts have actually gone idle or the wallet/runtime has entered a hard
  error state
- the watchdog now has a startup grace window so it does not restart
  `nexus-relay` based on stale pre-restart dispatch timestamps before the first
  post-restart payout window can complete
- `03-configure-and-start.sh` now refuses to leave a new image live unless it
  produces fresh completed payout sends after restart; otherwise it rolls back
  automatically to the previous image

Watchdog env overrides:

```bash
export NEXUS_TREASURY_WATCHDOG_ENABLED=true
export NEXUS_TREASURY_WATCHDOG_INTERVAL_SECONDS=300
export NEXUS_TREASURY_WATCHDOG_MAX_IDLE_SECONDS=300
export NEXUS_TREASURY_WATCHDOG_MAX_CONFIRM_LAG_SECONDS=300
export NEXUS_TREASURY_WATCHDOG_MAX_RESTARTS_PER_HOUR=12
export NEXUS_TREASURY_WATCHDOG_STARTUP_GRACE_SECONDS=180
export NEXUS_DEPLOY_POST_RESTART_SMOKE_ENABLED=true
export NEXUS_DEPLOY_POST_RESTART_SMOKE_TIMEOUT_SECONDS=360
export NEXUS_DEPLOY_POST_RESTART_WARMUP_GRACE_SECONDS=180
export NEXUS_DEPLOY_POST_RESTART_SMOKE_POLL_SECONDS=10
```

If treasury status ever collapses to `0 sats` unexpectedly after a backend or
deploy change:

- stop treating the old local wallet storage as authoritative
- copy the mnemonic and wallet storage off the VM
- validate the copied wallet on the upgraded tree first
- if needed, compare that reused storage against a fresh storage rebuild from
  the same mnemonic before deciding whether funds were actually spent

Validated recovery/cutover flow:

1. Generate and inspect `recovery-report.json` against a copied production-like
   wallet first.
2. Copy the validated recovery work dir onto the VM data disk if needed.
3. Set the report path and run the production cutover wrapper:

```bash
export NEXUS_TREASURY_RECOVERY_REPORT_PATH=/var/lib/nexus-relay/treasury-wallet-recovery-<stamp>/recovery-report.json
scripts/deploy/nexus/09-recover-treasury-wallet.sh
```

What the wrapper does:

- stops `nexus-relay`
- runs `treasury recovery-cutover --report-path ... --json` inside the deployed
  Nexus image against the live data disk
- atomically swaps the validated rebuilt wallet storage into the active treasury
  path while preserving a rollback dir
- starts `nexus-relay` again and verifies the local treasury status endpoint

Rollback after cutover:

- stop `nexus-relay`
- move the active rebuilt wallet dir aside
- move the reported `wallet_storage_rollback_dir` back onto
  `NEXUS_CONTROL_TREASURY_WALLET_STORAGE_DIR`
- start `nexus-relay`

## 5) Deploy artifacts

Verification receipts land under:

- `docs/reports/nexus/*-deploy-receipt.json`

Deploy receipts now include:

- endpoint latency evidence for `/healthz`, `/api/stats`, and
  `/v1/treasury/status`
- explicit gate rows with pass/fail reasons
- live treasury policy as reported by `GET /v1/treasury/status`
- treasury policy parsed from `/etc/nexus-relay/nexus-relay.env`
- recent payout activity, reason breakdowns, snapshot freshness, and active
  treasury continuity alerts

## 6) Operational notes

- The baseline VM is private-by-default and intended to be reached through `gcloud compute ssh --tunnel-through-iap` until staging/public cutover is ready.
- The persistent disk is mounted at `/var/lib/nexus-relay` and should survive service restarts and VM reboots.
- The deploy path assumes the VM service account can read from Artifact Registry.
- The durable relay data path and Nexus control receipt log path both live on the persistent disk.
- Wallet recovery reports and rebuilt-storage candidates should also live on the
  persistent disk so cutover and rollback survive reboots.

## 7) Public cutover

The public cutover path uses a Cloudflare tunnel from the production VM.

Why this path:

- it keeps the durable Nexus runtime on a private stateful VM
- it avoids pushing the durable relay back behind a stateless Cloud Run shape
- it lets `nexus.openagents.com` move without needing a separate external load balancer first

Script:

```bash
scripts/deploy/nexus/05-cutover-public-host.sh
```

What it does:

- creates or reuses the named Cloudflare tunnel
- routes `nexus.openagents.com` to that tunnel
- installs a `nexus-cloudflared.service` unit on the VM
- forwards public HTTPS / websocket traffic to `http://127.0.0.1:8080`

Required local prerequisites:

- `cloudflared` installed locally
- local Cloudflare auth already present (`cloudflared login` completed previously)
- access to the `openagents.com` zone in Cloudflare

The VM remains private-by-default. Public ingress is handled through the tunnel rather than by assigning a public VM IP.

## 8) Retire the old Cloud Run surface

Once the public hostname is confirmed on the durable VM path, remove the old stateless Cloud Run Nexus surface:

```bash
scripts/deploy/nexus/06-retire-cloud-run-surface.sh
```

What it removes:

- the old `nexus.openagents.com` Cloud Run domain mapping
- `openagents-nexus-relay`
- `openagents-nexus-control`

This keeps the live infra aligned with the durable single-service Nexus runtime instead of leaving a second stale Nexus path behind.

## 9) Operator policy and limits

The durable upstream relay should run with an explicit operator policy file:

- `apps/nexus-relay/deploy/upstream-config.toml`

The current committed profile does this intentionally:

- trusts `cf-connecting-ip` as the client IP header behind the Cloudflare tunnel
- rejects events more than 30 minutes in the future
- enables `nip42_auth`
- enables scraper limiting
- sets bounded global publish / subscription rates
- keeps message and frame sizes capped at 128 KiB
- lowers broadcast and persist buffers from upstream defaults to reduce runaway memory pressure

This config is copied onto the VM by `scripts/deploy/nexus/03-configure-and-start.sh` and loaded through `NEXUS_RELAY_UPSTREAM_CONFIG_FILE`.

## 10) Backup and restore

Use the operator scripts:

```bash
scripts/deploy/nexus/07-backup-relay-data.sh
NEXUS_BACKUP_ARCHIVE=/path/to/archive.tar.gz scripts/deploy/nexus/08-restore-relay-data.sh
```

Backup behavior:

- creates a consistent SQLite backup using `sqlite3 .backup`
- includes the Nexus control receipt log when present
- writes a small metadata manifest
- copies the backup archive to a local operator directory

Restore behavior:

- stops `nexus-relay`
- preserves the pre-restore files under `${NEXUS_DATA_DIR}/pre-restore-<stamp>`
- restores the SQLite database and receipt log
- starts `nexus-relay` again

Retention policy in this pass is intentionally simple:

- no automatic event TTL pruning is enabled yet
- keep point-in-time operator backup archives outside the repo
- revisit automatic retention only after product policy for relay history is explicit

## 11) What this runbook intentionally does not cover yet

- deeper abuse controls beyond the current relay limit profile

Those are tracked in later Nexus migration issues rather than hidden behind this baseline deploy story.
