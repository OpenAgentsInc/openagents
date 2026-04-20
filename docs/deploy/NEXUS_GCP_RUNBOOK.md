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

## 1.5) Hotfix lane decision

The current production hotfix decision is frozen in:

- `docs/deploy/NEXUS_HOTFIX_LANE.md`

The short version is:

- primary hotfix path: warm Linux builder plus binary-first deploys
- fallback path: image-first deploys, including Cloud Build

The binary lane is now implemented and has one retained bounded proof deploy in:

- `docs/reports/nexus/2026-04-16-binary-hotfix-lane-proof.md`

This runbook still documents the image-first path because it remains the
fallback and validation lane. Future Nexus deploy work should not assume Cloud
Build is still the default hotfix unblock path.

## 2) Baseline assumptions

- project: `openagentsgemini`
- region / zone: `us-central1` / `us-central1-a`
- VPC / subnet: `oa-lightning` / `oa-lightning-us-central1`
- VM: `nexus-mainnet-1`
- data disk: `nexus-relay-data-mainnet`
- default public host assumption: `nexus.openagents.com`
- default websocket assumption: `wss://nexus.openagents.com/`

The scripts are parameterized through env vars in `scripts/deploy/nexus/common.sh`.

## 3) Current image-first fallback flow

All scripts are in `scripts/deploy/nexus/`.

1. Build and push the Nexus image.

```bash
scripts/deploy/nexus/01-build-and-push-image.sh
```

The Nexus image build is now an explicit hotfix lane:

- it stages a Nexus-only source context instead of submitting the whole repo
- it fills non-Nexus workspace members in that staged context with lightweight
  placeholders so Cargo can still resolve the workspace without the full repo
- it materializes a `.nexus-build-plan/` dependency layer as the retained
  `cargo-chef` equivalent for the fallback image lane
- the Docker build now uses BuildKit registry cache, cache mounts, and
  optional GCS-backed `sccache`
- the checked-in fallback defaults now pin:
  - `E2_HIGHCPU_32`
  - `200 GB` build disk
- the default build profile is now `fast-release`

The retained hardening proof for this fallback lane is:

- `docs/reports/nexus/2026-04-16-cloudbuild-fallback-hardening.md`

Successful fallback runs now emit a JSON receipt under:

- `docs/reports/nexus/*-cloudbuild-image-<git_short_sha>.json`

`sccache` remains optional. The current fallback path leaves it off until the
Cloud Build image lane is proven stable with it enabled:

```bash
NEXUS_BUILD_SCCACHE_ENABLED=true scripts/deploy/nexus/01-build-and-push-image.sh
```

Use the normal production-optimized profile explicitly when you want it:

```bash
NEXUS_BUILD_PROFILE=release scripts/deploy/nexus/01-build-and-push-image.sh
```

For isolated validation lanes, publish a unique image tag without moving
`latest`:

```bash
NEXUS_IMAGE_TAG=nexus-hotfix-lane-$(date -u +%Y%m%d-%H%M%S) \
NEXUS_BUILD_UPDATE_LATEST_TAG=false \
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

If the change is config-only and you want to keep the current image:

```bash
scripts/deploy/nexus/03-refresh-config-and-restart.sh
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
- the public hostname fails `https://nexus.openagents.com/api/stats` or a
  public dry-run provider heartbeat against
  `https://nexus.openagents.com/api/provider-presence/heartbeat?dry_run=true`
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

## 3.5) Primary binary-first hotfix flow

The primary Nexus hotfix path is:

1. build a versioned Linux `nexus-relay` binary on a warm Linux builder
2. upload that versioned release to the Nexus VM
3. activate the release through `/opt/nexus-relay/current`
4. restart `systemd`
5. run local and public verification gates
6. keep the previous release ready for immediate rollback

The frozen operator contract for that path lives in:

- `docs/deploy/NEXUS_HOTFIX_LANE.md`

The canonical scripted operator path is:

- `scripts/deploy/nexus/11-provision-warm-builder.sh`
- `scripts/deploy/nexus/12-build-nexus-binary.sh`
- `scripts/deploy/nexus/13-upload-binary-release.sh`
- `scripts/deploy/nexus/14-activate-binary-release.sh`
- `scripts/deploy/nexus/04-verify-gates.sh`
- `scripts/deploy/nexus/15-rollback-binary-release.sh`

Detailed builder guidance lives in:

- `docs/deploy/NEXUS_WARM_BUILDER.md`

The first retained proof for this path lives in:

- `docs/reports/nexus/2026-04-16-binary-hotfix-lane-proof.md`

## 3.6) Primary binary deploy commands

Bring up or refresh the dedicated warm builder:

```bash
scripts/deploy/nexus/11-provision-warm-builder.sh
```

Build a versioned Linux `nexus-relay` binary on that builder:

```bash
scripts/deploy/nexus/12-build-nexus-binary.sh
```

Upload the built release onto the Nexus VM under
`/opt/nexus-relay/releases/<git_sha>`:

```bash
scripts/deploy/nexus/13-upload-binary-release.sh
```

Activate the uploaded release through `/opt/nexus-relay/current`:

```bash
scripts/deploy/nexus/14-activate-binary-release.sh
```

Verify the exact activated release:

```bash
VERIFY_EXPECTED_RELEASE_GIT_SHA="$(git rev-parse HEAD)" \
scripts/deploy/nexus/04-verify-gates.sh
```

Roll back to the previous binary release when a gate fails:

```bash
scripts/deploy/nexus/15-rollback-binary-release.sh
```

Roll back to a specific retained release explicitly:

```bash
NEXUS_RELEASE_GIT_SHA=<previous_git_sha> \
scripts/deploy/nexus/15-rollback-binary-release.sh
```

For bounded internal proof lanes that do not front the public hostname, disable
the public checks explicitly and point the provider-presence probe at the
bounded lane websocket URL:

```bash
NEXUS_VM=nexus-hotfix-lane-1 \
NEXUS_PUBLIC_WS_URL=wss://nexus-hotfix-lane.internal/ \
VERIFY_PUBLIC_CHECKS_ENABLED=false \
VERIFY_EXPECTED_RELEASE_GIT_SHA=<git_sha> \
scripts/deploy/nexus/04-verify-gates.sh
```

The binary lane retains local receipts for:

- warm-builder build timing
- binary release upload timing
- binary release activation timing
- binary rollback timing
- deploy verification timing and endpoint latencies

## 3.7) Warm builder bootstrap and binary artifact production

Bring up or refresh the dedicated warm builder:

```bash
scripts/deploy/nexus/11-provision-warm-builder.sh
```

Build a versioned Linux `nexus-relay` binary on that builder:

```bash
scripts/deploy/nexus/12-build-nexus-binary.sh
```

Force a cold-cache timing run:

```bash
NEXUS_BUILDER_CLEAR_CACHES=true scripts/deploy/nexus/12-build-nexus-binary.sh
```

The warm builder is now the primary binary production path. The first proof
deploy showed:

- same-SHA warm repeat build: `30.158 s`
- next-SHA warm build: `140.291 s`

The next builder improvement is to keep a stable source path so cross-SHA warm
builds reuse more cached work.

## 4) Runtime model

The deployed service is always `nexus-relay`, but the deploy unit can now be
activated in two ways:

- primary hotfix path: `/opt/nexus-relay/current/nexus-relay`
- fallback path: the `apps/nexus-relay/Dockerfile` container image

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
- `scripts/deploy/nexus/03-refresh-config-and-restart.sh` is the supported
  config-only path when the image is unchanged and you only want to rewrite the
  VM env/config files before restarting the service
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
export NEXUS_CONTROL_TREASURY_PLACEHOLDER_PAYOUT_MODE=inference_ready
export NEXUS_CONTROL_TREASURY_DEDUPE_PLACEHOLDER_HOSTS=true
export NEXUS_CONTROL_TREASURY_WALLET_STATUS_REFRESH_SECONDS=30
export NEXUS_CONTROL_TREASURY_MAX_CONCURRENT_SENDS=4
# Leave these unset until the new multi-platform Pylon release is actually live.
# export NEXUS_CONTROL_TREASURY_MIN_NEW_ACCRUAL_PYLON_VERSION=pylon-v0.1.1-rc1
# export NEXUS_CONTROL_TREASURY_MIN_NEW_ACCRUAL_STARTED_AT_UNIX_MS=<cutover_ms>
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
- `NEXUS_CONTROL_TREASURY_PLACEHOLDER_PAYOUT_MODE=inference_ready` shifts new
  placeholder windows away from pure presence and toward clients that are
  actually ready to serve the local Gemma lane. Old persisted policy blobs that
  predate this field still preserve their legacy `presence_only` behavior until
  you explicitly apply the tighter policy.
- `NEXUS_CONTROL_TREASURY_DEDUPE_PLACEHOLDER_HOSTS=true` blocks extra
  placeholder payouts when several clients appear to be the same underlying
  machine. This is only for placeholder/readiness accrual; accepted-work
  payouts remain tied to accepted contribution records.
- `NEXUS_CONTROL_TREASURY_MAX_CONCURRENT_SENDS=4` caps the per-cycle Spark
  fan-out so one stalled upstream batch cannot occupy all live payout slots at
  once.
- `NEXUS_CONTROL_TREASURY_MIN_NEW_ACCRUAL_PYLON_VERSION` plus
  `NEXUS_CONTROL_TREASURY_MIN_NEW_ACCRUAL_STARTED_AT_UNIX_MS` let Nexus stop
  awarding fresh payout windows to old `0.0.1-rc*` clients without stranding
  backlog they already earned before the cutoff.
- do not turn on the new-accrual version floor until the release tag exists
  with both `darwin-arm64` and `linux-x86_64` assets and the demo fleet has a
  working upgrade path. Once active, missing or invalid client-version claims
  are blocked for new accrual.
- `scripts/deploy/nexus/10-install-treasury-watchdog.sh` installs a systemd
  timer on the VM that runs every 5 minutes, checks the local treasury status
  plus recent completed-send journal entries, and by default restarts
  `nexus-relay` only if the service itself is inactive
- treasury runtime failures now stay in the treasury lane by default instead of
  repeatedly bouncing the public relay shell; the legacy aggressive restart
  path is still available through
  `NEXUS_TREASURY_WATCHDOG_RESTART_MODE=aggressive`
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
export NEXUS_TREASURY_WATCHDOG_RESTART_MODE=service_inactive_only
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
export NEXUS_TREASURY_RECOVERY_INSPECTION_TIMEOUT_MS=120000
export NEXUS_TREASURY_RECOVERY_RUST_LOG=warn
export NEXUS_TREASURY_RECOVERY_REPORT_PATH=/var/lib/nexus-relay/treasury-wallet-recovery-<stamp>/recovery-report.json
scripts/deploy/nexus/09-recover-treasury-wallet.sh
```

What the wrapper does:

- takes a VM-local recovery lock so only one recovery wrapper can run at a time
- runtime-masks and stops `nexus-relay`, then removes any stale `nexus-relay`
  container before inspecting wallet storage
- runs `nexus-control treasury recovery-cutover --report-path ... --json`
  inside the deployed Nexus image against the live data disk, using Docker
  `--entrypoint /usr/local/bin/nexus-control`
- passes `NEXUS_TREASURY_RECOVERY_INSPECTION_TIMEOUT_MS` through as
  `NEXUS_CONTROL_TREASURY_WALLET_RECOVERY_INSPECTION_TIMEOUT_MS`; the wrapper
  defaults to `120000` ms when unset and the binary clamps it to 30 minutes
- defaults `RUST_LOG` to `warn` for quieter recovery report output
- atomically swaps the validated rebuilt wallet storage into the active treasury
  path while preserving a rollback dir
- unmasks and starts `nexus-relay` again, then verifies the local treasury
  status endpoint

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
- new-accrual version-floor evidence, including the configured cutoff and the
  current count of online targets blocked by that floor

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
- keeps the tunnel ordered after `nexus-relay`, but no longer tears the tunnel
  down automatically just because `nexus-relay` restarts

Required local prerequisites:

- `cloudflared` installed locally
- local Cloudflare auth already present (`cloudflared login` completed previously)
- access to the `openagents.com` zone in Cloudflare

If you are reinstalling the VM-side tunnel unit from a machine that does not
have Cloudflare login state, you can still use the repo-managed script by
passing an already-issued tunnel token and skipping DNS setup:

```bash
export NEXUS_CLOUDFLARE_TUNNEL_TOKEN=...
export NEXUS_CLOUDFLARE_SKIP_DNS_SETUP=true
scripts/deploy/nexus/05-cutover-public-host.sh
```

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
