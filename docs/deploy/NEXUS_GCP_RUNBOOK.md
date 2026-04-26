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

### 3.1) Image build mistakes to avoid

These notes came from the `2026-04-21` Pylon `0.1.5`, `0.1.6`, and `0.1.7`
/ CS336 homework payout deploys. Keep them in this runbook so future agents do
not rediscover the same failures in production.

Build from an integrated commit. The image tag defaults to the current Git
short SHA. If you build from a detached temporary worktree that has local
commits not pushed to `main`, the image may be technically buildable but the
issue is not closeable. Push the exact commit to `origin/main` before treating
the image as a release candidate.

The staged Nexus build context uses `apps/nexus-relay/deploy/Cargo.nexus.lock`,
not only the repo-root `Cargo.lock`. When workspace package versions change,
especially for Pylon releases, refresh and commit this deploy lockfile before
Cloud Build. A stale deploy lock fails in the dependency layer with:

```text
cargo fetch --locked
error: the lock file /work/Cargo.lock needs to be updated but --locked was passed
```

Do not fix that by blindly running `cargo generate-lockfile` in the staged
context. The Spark SDK tree currently depends on a yanked but already-locked
transitive crate, so a fresh resolver pass can fail on
`frost-secp256k1-tr-unofficial = "^2.2.0"` even when the committed lockfile is
still valid for builds. The safe repair path is:

```bash
tmp_context="$(mktemp -d /tmp/openagents-nexus-lock-check.XXXXXX)"
scripts/deploy/nexus/stage-build-context.sh "$tmp_context"
cp Cargo.lock "$tmp_context/Cargo.lock"
(cd "$tmp_context" && cargo fetch --offline)
cp "$tmp_context/Cargo.lock" apps/nexus-relay/deploy/Cargo.nexus.lock

verify_context="$(mktemp -d /tmp/openagents-nexus-lock-verify.XXXXXX)"
scripts/deploy/nexus/stage-build-context.sh "$verify_context"
(cd "$verify_context" && cargo fetch --locked)
```

Review the resulting deploy-lock diff before committing it. A normal version
bump should show only owned workspace packages moving to the new version. If
the diff rewrites large dependency sections, stop and understand why before
deploying.

For Pylon releases, publish the CLI release and npm bootstrap before using a
production Nexus image as final proof. The end-to-end closeout path depends on
public users being able to run `npx @openagentsinc/pylon` or an already
installed `pylon`, download the matching GitHub release asset, and then run the
bare `pylon` command. Server-side Nexus changes alone do not prove the public
onboarding claim.

If `03-configure-and-start.sh` gets stuck in `Waiting for post-deploy payout
smoke`, do not assume the image failed and do not blindly add funds first.
Inspect `GET /v1/treasury/status`, especially `active_continuity_alerts`,
`training_payout_ledger_summary`, and `recent_training_payouts`. During the
Issue #4413 proof, the service was healthy on the new image but smoke waited
because an accepted-work payout was still queued. The root cause was not the
container startup path; it was treasury dispatch accounting that treated
already confirmed 24-hour payouts and old `dispatched` rows as current wallet
reservations. Confirmed and already-`dispatched` payouts should count against
the daily cap and reconciliation surfaces, while only `dispatching` payouts
should reserve the current wallet balance.

After editing release scripts, run at least:

```bash
bash -n scripts/release/pylon-binary-release.sh
```

The stable-tag release path must tolerate an empty release-flag array. Do not
use an unguarded `${RELEASE_FLAGS[@]}` expansion with `set -u`.

Do not run standalone `pylon training intake` or `pylon training sync` against
the same Pylon home while a bare `pylon` process is already running. The
standalone CLI and the long-running process share the same state directory but
do not share the in-memory supervisor slot. During the `0.1.6` proof this
created confusing validator state, including overwritten invocation manifests
and stale challenge leases. If a bare `pylon` process is running, inspect it
through its admin endpoint instead:

```bash
curl -fsS http://127.0.0.1:9468/v1/training/status | jq .
curl -fsS -X POST http://127.0.0.1:9468/v1/training/sync | jq .
```

If you need explicit standalone commands, stop the bare `pylon` process first
or use a completely separate `PYLON_HOME`.

Terminal training closeout must not be blocked behind artifact/TRN
publication. Direct Nexus authority writes, validator finalization, window
reconciliation, and accepted-work payout projection are the primary earning
path. Artifact/TRN publication is evidence and should retry, but a slow signed
URL upload must not wedge the user-visible earning loop. This is why
`pylon-v0.1.7` moved terminal authority reporting before artifact publication
and bounded the terminal publication attempt.

When sourcing workspace-local secret files for live checks, preserve `PATH`
first. Some local env files are intended for operator credentials, not as full
shell profiles:

```bash
old_path="$PATH"
set -a
source /Users/christopherdavid/work/.secrets/nexus-admin.env
set +a
PATH="$old_path"
```

Never paste raw bearer tokens, wallet mnemonics, or API keys into runbooks,
issue comments, receipts, or normal terminal output.

### 3.1) Public outage emergency rule

Treat public Nexus reachability failures as emergency production work.

Symptoms that qualify:

- `https://nexus.openagents.com/api/stats` returns Cloudflare `530` / `1033`
- the public provider heartbeat path fails from live Pylons
- the hosted online fleet suddenly drops because the public Nexus hostname is
  unreachable

Do not just note that Nexus is degraded and continue other work. Restore the
public path first, then return to secondary issue work.

Fast triage order:

```bash
curl -I https://nexus.openagents.com/api/stats

gcloud compute ssh nexus-mainnet-1 --tunnel-through-iap \
  --project openagentsgemini --zone us-central1-a \
  --command='sudo systemctl --no-pager --full status nexus-relay nexus-cloudflared | sed -n "1,120p"'

gcloud compute ssh nexus-mainnet-1 --tunnel-through-iap \
  --project openagentsgemini --zone us-central1-a \
  --command='curl -fsS http://127.0.0.1:8080/healthz'
```

If the VM-local origin is healthy but the public host is still down, repair the
tunnel path first. If the guest network stack itself is broken and the VM
cannot reach metadata or the public internet, reset the VM immediately instead
of waiting for the condition to clear on its own.

### 3.1.1) Redacted health snapshot command

`nexus-control` now has an observation-only health snapshot command for
operators and future health agents:

```bash
cargo run -p nexus-control --bin nexus-control -- health snapshot --pretty
```

The command probes:

- `https://nexus.openagents.com/healthz`
- `https://nexus.openagents.com/api/stats`
- `https://nexus.openagents.com/v1/treasury/status`

It emits one redacted JSON object on stdout with stable top-level sections:

- `classification`
- `verification_gates`
- `endpoints`
- `treasury`
- `training`
- `fleet`
- `website`
- `infra`
- `issues`

The classifier is deterministic and does not call Probe, an LLM, GCP, or any
recovery action. It maps the normalized snapshot into these states:

- `healthy`: all required predicates passed.
- `watch`: only warning-level predicates failed, such as low balance runway or
  stale public stats.
- `degraded`: an error-level predicate failed, such as payout dispatch,
  confirmation, training launch, or payout queue health.
- `incident`: public Nexus reachability failed, including Cloudflare `530` or
  `1033`.
- `needs_operator`: treasury, wallet, VM, relay, tunnel, restart-loop, or OOM
  predicates require operator action.
- `recovering`: a recovery marker is present while blocking predicates are no
  longer failing.
- `verified_closed`: all predicates passed and the incident has a verified
  closure marker.

`classification.failed_predicates` includes `predicate_id`, `severity`,
`status`, `detail`, and `remediation_hint` for every failed predicate. The
current verification gates are:

- `payout_capability`
- `training_dispatch`
- `website_stats_freshness`
- `infra_availability`

Each gate returns `passed`, `status`, `checked_predicates`, and the failed
predicate objects that made the gate fail. Future Forge, openagents.com, and
Autopilot health surfaces should consume these gates instead of reclassifying
raw stats independently.

Use the deterministic fake mode for local tests, docs, and CI-like smoke
checks:

```bash
cargo run -p nexus-control --bin nexus-control -- health snapshot --fake --pretty
```

Use `--base-url <url>` for a local or staging Nexus and `--timeout-ms <ms>` to
bound public probes. This command only observes and normalizes state. It does
not restart services, refresh wallets, create invoices, dispatch work, or run
recovery actions.

The hosted health-runner lane for this command is documented in:

- `docs/deploy/NEXUS_HEALTH_RUNNER_GCP_RUNBOOK.md`

That lane runs `/usr/local/bin/nexus-health-agent` from the Nexus image as a
Cloud Run Job for one-shot proof/actions and as
`/usr/local/bin/nexus-health-agent-server` in a warm Cloud Run Service for the
recurring monitor. Use it when the proof must not depend on an operator
laptop's local `gcloud` OAuth session. The default job remains monitor-only;
leased recovery job args can now record bounded actions such as
`treasury_refresh`, while service restarts and VM mutations stay routed through
Forge/Probe executors rather than local shell commands.

### 3.2) Issue #4413 live proof checklist

This checklist captures the operational mistakes and recovery path from the
Issue #4413 Pylon public-onboarding and CS336 homework payout proof. Use it
before handing future agents a live Nexus deploy or issue-closeout thread.

Start from the ownership rule, not from whatever worktree happens to be open.
Issue #4413 touched public Pylon onboarding, Nexus training dispatch, and live
treasury payout continuity, so the closeout required all of these to be true at
the same time:

- the public Pylon version was released as both GitHub release assets and an
  npm bootstrap version
- the exact code being deployed was pushed to `origin/main`
- the Nexus image was built from that pushed commit
- the production service was restarted onto that exact image
- a user-style `pylon` invocation, not a source-tree shortcut, claimed and
  completed real homework work against production
- the accepted-work payout reached live treasury dispatch and reconciliation
  surfaces

Do not close the issue from a feature branch, detached worktree, or local-only
proof. Branch work is evidence, not completion.

Keep the Pylon proof public-style. The proof command should be as close as
possible to what a real user runs: install via `npx @openagentsinc/pylon` or
the already-installed `pylon`, then run bare `pylon`. Avoid proving success
with `cargo run`, repo-local aliases, or manually seeded state unless the issue
explicitly asks for a source-developer lane. For the `0.1.7` release proof, the
minimum acceptable public binary version is `0.1.7`; older binaries either lack
worker-first retained-training behavior or can block terminal closeout behind
artifact/TRN publication.

If the live service has a wallet balance but accepted-work payouts remain
queued, inspect the payout record classes before assuming the wallet needs more
funding. In Issue #4413 the wallet had enough sats for the next 1-sat homework
payout, but old rows made the dispatcher believe the spendable balance was
already reserved. The correct split is:

- `dispatching` rows reserve current wallet balance because the send is still
  actively in flight
- `dispatched` rows are reconciliation work and should not reserve spendable
  balance forever
- `confirmed` rows count against the daily budget cap but do not reserve
  current spendable balance

When `03-configure-and-start.sh` waits on payout smoke, check these live fields
before changing code or adding funds:

```bash
old_path="$PATH"
set -a
source /Users/christopherdavid/work/.secrets/nexus-admin.env
set +a
PATH="$old_path"

token="${NEXUS_ADMIN_BEARER_TOKEN:-${NEXUS_CONTROL_ADMIN_BEARER_TOKEN:-}}"
curl -fsS -H "Authorization: Bearer ${token}" \
  https://nexus.openagents.com/v1/treasury/status |
  jq '{
    wallet_balance_sats,
    wallet_runtime_status,
    placeholder_payout_mode,
    accepted: .training_payout_ledger_summary.accepted_work,
    alerts: .active_continuity_alerts,
    recent_training_payouts
  }'
```

On the VM, inspect only summary state by default. Do not dump secrets or full
wallet material:

```bash
gcloud compute ssh nexus-mainnet-1 \
  --tunnel-through-iap \
  --project openagentsgemini \
  --zone us-central1-a \
  --command 'systemctl is-active nexus-relay; sudo docker ps --filter name=nexus-relay --format "{{.Image}} {{.Status}}"'
```

If the deploy wrapper is about to roll back but the service is known-good and
the remaining problem is a diagnosed treasury logic bug or data-state blocker,
stop and fix the bug rather than letting an unrelated rollback obscure the
active image. Record the active image digest and service state in the issue
comment or proof report before proceeding.

For final proof, prefer a fresh isolated proof root so stale local Pylon state
cannot satisfy the run accidentally:

```bash
PROOF_ROOT="var/proof/issue-4413-public-prod-016-$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -p "${PROOF_ROOT}/logs"
HOME="${PWD}/${PROOF_ROOT}/home" \
OPENAGENTS_DISABLE_TELEMETRY=1 \
npx --yes @openagentsinc/pylon@0.1.7 --version 0.1.7 \
  --pylon-home "${PWD}/${PROOF_ROOT}/home/.openagents/pylon" \
  --install-root "${PWD}/${PROOF_ROOT}/install" \
  --skip-diagnostics \
  --no-launch \
  --json | tee "${PROOF_ROOT}/bootstrap.json"

PYLON_DIR="${PWD}/${PROOF_ROOT}/install/versions/pylon-v0.1.7-darwin-arm64"
HOME="${PWD}/${PROOF_ROOT}/home" \
OPENAGENTS_DISABLE_TELEMETRY=1 \
PATH="${PYLON_DIR}:${PATH}" \
pylon 2>&1 | tee "${PROOF_ROOT}/logs/pylon-bare.log"
```

The final issue-closeout comment should name the deployed commit, image tag or
digest, Pylon version, training run id, node pubkey, accepted outcome, payout
record or payment id when available, and the verification commands or receipts
that prove those facts. If any of those are missing, the issue is not honestly
closed yet.

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

The same deploy path now installs a public reachability watchdog. It checks the
VM-local `/healthz`, the public `https://nexus.openagents.com/api/stats` path,
and both `nexus-relay` and `nexus-cloudflared` systemd services. If local
origin health fails it restarts `nexus-relay`; if the local origin is healthy
but the public host returns `530` / `1033` or goes dark, it restarts
`nexus-cloudflared`. Refresh only that watchdog with:

```bash
scripts/deploy/nexus/16-install-public-watchdog.sh
```

The hosted `nexus-health-agent` Cloud Run runtime is separate from the VM-local
systemd watchdogs. Use the health-runner Job for one-shot smoke or
lease-gated actions, and the warm health-runner Service for recurring
GCP-origin public-edge probes and Forge health events:

```bash
scripts/deploy/nexus/17-provision-health-runner-identity.sh
scripts/deploy/nexus/18-deploy-health-runner-job.sh
scripts/deploy/nexus/19-smoke-health-runner-job.sh
scripts/deploy/nexus/21-deploy-health-runner-service.sh
scripts/deploy/nexus/20-deploy-health-runner-scheduler.sh
```

Cloud Scheduler is minute-granularity. The recurring monitor should target the
warm Cloud Run Service `/run` endpoint with OIDC so the loop does not backlog
behind Cloud Run Job provisioning latency:

```bash
scripts/deploy/nexus/21-deploy-health-runner-service.sh

SERVICE_URL="$(gcloud run services describe nexus-health-runner-service \
  --project openagentsgemini \
  --region us-central1 \
  --format 'value(status.url)')"

NEXUS_HEALTH_RUNNER_SCHEDULER_AUTH_MODE=oidc \
NEXUS_HEALTH_RUNNER_SCHEDULER_URI="${SERVICE_URL}/run" \
NEXUS_HEALTH_RUNNER_SCHEDULER_OIDC_AUDIENCE="${SERVICE_URL}" \
scripts/deploy/nexus/20-deploy-health-runner-scheduler.sh
```

The same deploy script now installs the default hosted homework auto-dispatcher
runtime. Production defaults are:

```text
NEXUS_CONTROL_CS336_HOMEWORK_AUTO_DISPATCH_ENABLED=true
NEXUS_CONTROL_CS336_HOMEWORK_AUTO_DISPATCH_INTERVAL_SECONDS=600
NEXUS_CONTROL_CS336_HOMEWORK_AUTO_DISPATCH_AMOUNT_SATS=25
NEXUS_CONTROL_CS336_HOMEWORK_AUTO_DISPATCH_MAX_CONTRIBUTORS=256
NEXUS_CONTROL_CS336_HOMEWORK_AUTO_DISPATCH_TOTAL_BUDGET_SATS=6400
NEXUS_CONTROL_CS336_HOMEWORK_AUTO_DISPATCH_MIN_PYLON_VERSION=0.1.15
NEXUS_CONTROL_CS336_HOMEWORK_AUTO_DISPATCH_REQUIRE_UPDATED_BUILD=false
NEXUS_CONTROL_CS336_HOMEWORK_AUTO_DISPATCH_WINDOW_DURATION_SECONDS=1800
```

The automatic dispatcher runs one cycle immediately after `nexus-relay` starts,
then every 600 seconds. It creates a fresh CS336 A1 homework run, targets online
eligible Pylons on the default demo training network, and pays only accepted
homework closeouts. The manual admin endpoint remains available for bounded
proofs and temporary pacing overrides.

The validator selection path must treat automatic homework as the freshest paid
work class. Current ordering is `homework_auto_dispatch`, then manual
`homework_dispatch`, then normal training runs, then hosted starter backlog.
Without that ordering a validator can spend time on stale historical homework
or starter challenges while a fresh automatic paid run is sealed and awaiting
payout.

The first production proof for the 10-minute automatic loop is
`docs/reports/nexus/20260423-101209-cs336-auto-dispatch-prod-e2e.json`.
It used image
`us-central1-docker.pkg.dev/openagentsgemini/openagents-nexus/nexus-relay:33ee54cf6c51`
and two npm-installed `pylon-v0.1.11` processes. Nexus created two consecutive
`run.cs336.a1.auto_10m_*` runs, both windows reconciled, both closeouts were
rewarded, accepted-work payout stats advanced by 50 sats, and the proof worker
wallet received two completed 25-sat Spark payments. If a future proof sees
temporary `queued_retry` / `trn_publish_retry` on contribution artifacts, keep
the Pylon process alive or rerun `pylon training publish --json`; those retries
are idempotent and recovered in the proof above.

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
export NEXUS_CONTROL_TREASURY_PLACEHOLDER_PAYOUT_MODE=presence_only
export NEXUS_CONTROL_TREASURY_DEDUPE_PLACEHOLDER_HOSTS=true
export NEXUS_CONTROL_TREASURY_WALLET_STATUS_REFRESH_SECONDS=30
export NEXUS_CONTROL_TREASURY_MAX_CONCURRENT_SENDS=4
# Leave these unset until the new multi-platform Pylon release is actually live.
# export NEXUS_CONTROL_TREASURY_MIN_NEW_ACCRUAL_PYLON_VERSION=pylon-v0.1.1-rc1
# export NEXUS_CONTROL_TREASURY_MIN_NEW_ACCRUAL_STARTED_AT_UNIX_MS=<cutover_ms>
```

Accepted-work payout note:

- `nexus-control` now hard-caps accepted-work payout sends to `4` concurrent
  Spark sends per cycle even if `NEXUS_CONTROL_TREASURY_MAX_CONCURRENT_SENDS`
  is set higher
- keep the env at `4` for production unless there is proof that the upstream
  Spark path can tolerate more without `Cancelled` / leaf-selection failures

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
- `NEXUS_CONTROL_TREASURY_PLACEHOLDER_PAYOUT_MODE=presence_only` keeps the
  current production placeholder lane aligned with raw online presence. This is
  the current operator default because the live earning flow must continue
  through Nexus recovery and should not depend on local Gemma readiness unless
  you explicitly tighten it later.
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
export NEXUS_TREASURY_RECOVERY_PARALLEL_INSPECTIONS=false
export NEXUS_TREASURY_RECOVERY_RUST_LOG=warn
export NEXUS_TREASURY_RECOVERY_REPORT_ATTEMPTS=3
export NEXUS_TREASURY_RECOVERY_REPORT_PATH=/var/lib/nexus-relay/treasury-wallet-recovery-<stamp>/recovery-report.json
scripts/deploy/nexus/09-recover-treasury-wallet.sh
```

What the wrapper does:

- takes a VM-local recovery lock so only one recovery wrapper can run at a time
- runtime-masks and stops `nexus-relay`, then removes any stale `nexus-relay`
  container before inspecting wallet storage
- performs registry login and image pull before stopping `nexus-relay`
- avoids command-substitution capture while cleanup is armed; recovery JSON is
  written through a normal temp file so a subshell cannot unmask or restart
  `nexus-relay` while the recovery inspection is still running
- has a local shell-shape regression check:
  `bash scripts/deploy/nexus/test-recover-treasury-wallet-shell-guards.sh`
- runs `nexus-control treasury recovery-cutover --report-path ... --json`
  inside the deployed Nexus image against the live data disk, using Docker
  `--entrypoint /usr/local/bin/nexus-control`
- passes `NEXUS_TREASURY_RECOVERY_INSPECTION_TIMEOUT_MS` through as
  `NEXUS_CONTROL_TREASURY_WALLET_RECOVERY_INSPECTION_TIMEOUT_MS`; the wrapper
  defaults to `120000` ms when unset and the binary clamps it to 30 minutes for
  each balance, payment-list, and unclaimed-deposit read
- passes `NEXUS_TREASURY_RECOVERY_PARALLEL_INSPECTIONS=false` by default so
  the current and rebuilt storage inspections run serially and avoid doubling
  Spark upstream sync pressure during production recovery
- defaults `RUST_LOG` to `warn` for quieter recovery report output
- retries recovery report generation up to `NEXUS_TREASURY_RECOVERY_REPORT_ATTEMPTS`
  times and removes the partial work dir between failed attempts
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
