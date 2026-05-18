# Pylon/Nexus Earning Release Runbook

2026-05-16 LDK v0.2 notice: Pylon v0.2 registration must use an LDK-compatible
target (`bolt12_offer`, `bolt11_invoice`, `bip353_name`, or `lnurl_pay`). Do not
add Spark destination creation or Spark drain modes back into public Pylon or
Nexus release paths.

This runbook captures the practical release and proof lessons from the April
21, 2026 Issue #4413 work. Its purpose is to keep future agents from repeating
the same build, release, deploy, and proof mistakes when changing the public
Pylon earning loop or the Nexus hosted-homework payout path.

## Scope

Use this runbook when a change affects any of these paths:

- public `pylon` install or launch behavior
- `@openagentsinc/pylon` bootstrap releases
- GitHub `pylon-v*` release assets
- Nexus hosted starter or homework dispatch
- training worker or validator closeout
- accepted-work payout projection or treasury dispatch
- Issue closeout requiring proof that a normal user can run `pylon` and get
  paid for available hosted training work

Do not use this as a shortcut around the local proof runtime. For distributed
training, homework, Nexus authority, Pylon fleet, artifact, validator,
reconcile, closeout, or payout-proof work, the primary development loop is
still the local proof runtime from Issue #4385. Production Nexus is final proof,
not the debugger for ordinary scheduler, artifact, validator, or payout bugs.

## LDK Payout Target Gate

Nexus must not admit, auto-launch, or dispatch new paid starter/homework work
for a Pylon unless treasury has a registered LDK v0.2-compatible payout target
for that Pylon identity. The registered treasury target is the source of truth.
Do not treat older node heartbeat settlement fields as sufficient eligibility;
those fields can be stale and must not create paid-work pressure.

The expected blocker reasons are:

- `training_scheduler_payout_target_requires_ldk_v0_2` for default hosted
  CS336 lease claims.
- `homework_worker_payout_target_requires_ldk_v0_2` in homework worker
  eligibility metrics.
- `homework_launch_target_payout_target_requires_ldk_v0_2` in manual launch
  target rejection samples.

When these blockers appear, fix the Pylon registration path first. Do not work
around them by enabling a fallback payout rail.

## Hosted Psionic Runtime Gate

A hosted Pylon that is expected to accept retained training work must have the
packaged Psionic runtime surface installed. A standalone `/usr/local/bin/pylon`
binary is not enough. Use the hosted install script after building a reviewed
runtime archive:

```bash
NEXUS_PYLON_RUNTIME_ARCHIVE=/tmp/psionic-runtime-<psionic-sha>.tar.gz \
scripts/deploy/nexus/29-install-pylon-psionic-runtime.sh
```

Then verify at least these fields on every host:

```bash
sudo -u pylon /usr/local/bin/pylon training status --json
```

- `runtime_surface_detected: true`
- `psionic_repo_root: "/var/lib/pylon/psionic"`
- `psionic_repo_source: "env_override"`
- `/var/lib/pylon/psionic/.openagents-psionic-revision` exists and contains
  the clean Psionic Git revision used for admission identity. Hosted runtime
  archives are not Git worktrees; Pylon must use this marker instead of calling
  `git rev-parse HEAD` during training assignment intake.
- `/var/lib/pylon/psionic/fixtures/training/cs336_a1_reference_tiny_corpus.txt`
  exists. The bounded CS336 A1 paid-work smoke lane reads this tiny corpus from
  the packaged runtime root, never from a developer machine checkout path.

This gate fixes runtime discovery only. If `homework_worker_eligible_pylons`
is still zero and the blocker reason is
`homework_worker_payout_target_requires_ldk_v0_2`, the remaining problem is
the LDK payout target, not the Psionic runtime.

The 2026-05-18 LDK proof demonstrates why this gate is mandatory. Updating the
Pylon binary to `ad27f320b` without updating `/var/lib/pylon/psionic` left
validators on stale Psionic revision `09b71872b24a934228f61c28e65e3aa544025f54`
and caused validator replay to fail with:

```text
failed to resolve machine runtime identity:
failed to resolve psionic repo root: No such file or directory
```

The passing proof installed Psionic revision `55e4b66f` from
`psionic-runtime-55e4b66f.tar.gz` across all hosted Pylons. The final accepted
run was `run.cs336.a1.ldk-proof-20260518151532`; it reconciled
`window.cs336.a1.ldk-proof-20260518151532.0001` as rewarded and settled a
25-sat `accepted_work` payout through LDK. The full report is
`docs/reports/nexus/2026-05-18-ldk-accepted-work-production-proof.md`.

## Non-Negotiable Completion Rule

An issue is not complete because a temporary worktree has code, a feature branch
has a commit, or a live machine happened to pass once. Close an issue only after
all of these are true:

- the relevant code and docs are committed and pushed to `origin/main`
- the public Pylon release that users need has been published, if the issue
  depends on public install behavior
- the production Nexus image is built from the pushed `main` commit, if Nexus
  behavior changed
- the production service is deployed onto that exact image
- a fresh public-style Pylon home proves the user path by running bare `pylon`
- hosted work reaches the expected terminal state
- accepted work creates and dispatches the payout required by the issue
- proof artifacts, deployment receipts, and issue comments name the exact
  commit, image, release, run id, contribution id, node id, and payout state

Branch work is evidence. It is not closeout.

## Correct Release Order

Use this sequence for public earning-loop changes:

1. Make the Pylon/Nexus changes in `openagents`.
2. Run the relevant local proof runtime and focused Rust tests.
3. Bump the workspace version and `packages/pylon-bootstrap/package.json` when
   the public Pylon binary behavior changes.
4. Refresh deploy locks before any Nexus Cloud Build if workspace package
   versions changed.
5. Commit and push the exact release candidate to `origin/main`.
6. Publish the GitHub `pylon-vX.Y.Z` release assets and npm bootstrap package.
7. Build the Nexus image from the pushed commit.
8. Deploy the Nexus image through the scripted production path.
9. Prove the user path from a fresh Pylon home using the public npm/bootstrap
   lane or already-installed release binary.
10. Record receipts, update docs, comment on issues, and close only after
    payout proof is visible.

Do not reverse steps 5 and 7. A Cloud Build image from an unpushed detached
worktree can run, but it cannot honestly close the issue.

Npm bootstrap-only changes can publish a new `@openagentsinc/pylon` package
without cutting a new GitHub `pylon-vX.Y.Z` binary release when the Rust Pylon
binary did not change. As of `@openagentsinc/pylon` `0.1.14`, the default
launcher keeps the cached standalone binary current by checking GitHub Releases
on a six-hour background cadence while `pylon-tui` is open, accepting only
`pylon-v...` releases whose GitHub release author is `AtlantisPleb`, then
restarting the dashboard from the newly installed cache path. `GITHUB_TOKEN` or
`GH_TOKEN` authenticates those GitHub release lookups for shared-network
operators. `pylon --no-updates` disables background polling, and
`pylon --version <x.y.z>` remains a pinned run that does not auto-upgrade.

When cutting a native Windows binary release, package it on a clean native
Windows x86_64 host. The release-asset contract is:

- `pylon-vX.Y.Z-windows-x86_64.zip`
- `pylon-vX.Y.Z-windows-x86_64.zip.sha256`

The npm bootstrap now resolves that asset on native Windows x86_64 and installs
`pylon.exe` plus `pylon-tui.exe` from the unpacked cache path.

Directly extracted release assets are outside that auto-update contract. A
standalone `./pylon` or `./pylon-tui` launched from an archive reports its
compiled version until the operator manually replaces the archive or moves back
to the npm/bun-managed launcher. Do not close a stale-version incident by
editing stats display logic to hide those rows; `pylon_client_version_counts`
must continue to expose the live heartbeat versions that Nexus actually sees.

## Version Floor Rules

For the current hosted training earning path, the minimum public Pylon release
is `pylon-v0.1.16`. The current package-managed launcher should be
`@openagentsinc/pylon` `0.1.17` or newer so CLI subcommands are forwarded to
the installed binary and background GitHub update checks use the bounded
cadence.

Older versions are useful historical proof points but not sufficient for final
closeout:

- `0.1.4` proved public install plus worker artifact sealing.
- `0.1.5` proved the package path could launch the earning loop, but retained
  failed validator leases could still block fresh worker intake.
- `0.1.6` fixed the worker-first and validator-default problems, but terminal
  validator closeout could still block behind artifact/TRN publication.
- `0.1.7` reports terminal worker and validator authority state before slower
  artifact/TRN publication and bounds publication attempts, so accepted-work
  payout projection is not wedged by a slow signed-URL upload.
- `0.1.10` adds the Autopilot-controlled earning proof fixes: default Spark
  payout destination creation in the long-lived Pylon serve path, retained
  snapshot reuse for validator replay retries, and stricter Autopilot paid
  status projection.
- `0.1.11` makes the public user path match the actual operator expectation:
  `pylon` opens a minimal homework-earning TUI, the TUI starts and supervises
  the earning worker, the default bootstrap path launches that TUI, Gemma
  diagnostics/downloads are opt-in, and new hosted homework runs should avoid
  older TUI-only clients that never started the worker.
- `0.1.12` fixes issue #4414 for Psionic-backed homework/training jobs on Mac:
  Pylon prefers a current `target/release/psionic-train` binary when it exists,
  falls back to `cargo run --release` instead of debug Cargo, and records
  signal/log-tail diagnostics when the supervisor exits without a normal code.
- `@openagentsinc/pylon` `0.1.14` is a package-managed launcher update, not a
  Rust binary release: it adds the trusted GitHub release auto-updater and the
  `--no-updates` escape hatch. The current launcher uses a bounded background
  cadence instead of 30-second polling and still runs the latest trusted
  `pylon-v...` standalone binary asset available for the machine.
- `0.1.15` fixes issue #4449 for terminal homework closeout: Pylon uploads and
  verifies the worker contribution artifact bundle before sealing the window,
  so validators do not replay a sealed contribution whose signed artifact fetch
  still returns 404.
- `0.1.16` fixes the issue #4451 packaging gap for standalone homework-worker
  admission: the Pylon archive includes the minimal `./psionic` runtime surface
  and `psionic/target/release/psionic-train`, so normal npm-installed Pylons
  can advertise homework training capability without a sibling checkout.
- `@openagentsinc/pylon` `0.1.17` is a package-managed launcher update, not a
  Rust binary release: it forwards subcommands such as `pylon status --json` to
  the installed standalone binary, uses `GITHUB_TOKEN` or `GH_TOKEN` for
  authenticated GitHub lookups, and reduces background release polling from the
  old 30-second loop to a six-hour cadence.

Nexus must enforce the same floor for new hosted starter runs:

```text
min_pylon_version=0.1.16
```

If the code changes the earning-loop behavior again, update this floor, the
Pylon docs, the Nexus treasury docs, the audit, and the issue comments together.

The published `0.1.16` release receipt is
`docs/reports/nexus/20260427-pylon-v0.1.16-release.json`. It proves the
packaged Psionic runtime release asset, npm bootstrap smoke, and the
worker-admission packaging bridge for issue #4451. The prior `0.1.15` receipt
is `docs/reports/nexus/20260426-pylon-v0.1.15-release.json`; it proves the
issue #4449 artifact-before-seal regression and production earning drill.
The package-only `0.1.17` launcher receipt is
`docs/reports/nexus/20260427-pylon-bootstrap-v0.1.17-release.json`; it proves
issue #4463 and #4464 without cutting a new standalone binary release.

The prior `0.1.12` release-smoke receipt is
`docs/reports/nexus/20260423-issue-4414-pylon-v0.1.12-release.json`. It proves
the release assets, npm bootstrap, and issue #4414 training launch regression.

The prior `0.1.11` release-smoke receipt is
`docs/reports/nexus/20260423-072712-pylon-v0.1.11-release.json`. It proves the
release asset, npm bootstrap, TUI-managed worker, no default Gemma model
download, and production homework lease intake/seal path.

The current full production earning receipt is
`docs/reports/nexus/20260423-084113-pylon-v0.1.11-post-health-prod-e2e.json`.
It proves fresh npm-installed `0.1.11` worker and validator binaries against
the deployed `bee1c593b9ba` Nexus image, Nexus online presence as
`pylon/0.1.11`, an admin-triggered 25-sat CS336 homework run, rewarded
closeout, confirmed and settled accepted-work treasury payout, a worker wallet
balance increase from `0` to `25` sats, `nexus_payout_loop_health: idle`, no
treasury degraded reason, and no default Gemma model download. Use this receipt
as the current release closeout proof. The earlier
`docs/reports/nexus/20260423-080422-pylon-v0.1.11-prod-e2e.json` receipt
remains the first successful full `0.1.11` production earning proof.

## Version Telemetry Rules

Do not use `recent_pylons` as a fleet version count. Nexus intentionally caps
that array to a small recent sample for UI/debug rows, and production can have
dozens of live sessions whose newest eight heartbeats are not representative.
The stats-page source of truth for online client versions is
`pylon_client_version_counts` from `GET /api/stats`. That histogram is computed
across every live provider-presence row inside the configured stale window and
counts online sessions by the `client_version` value that Pylon sends in the
heartbeat body.

If `pylon_client_version_counts` still includes `pylon/0.1.14` after
`pylon-v0.1.15` is published, the correct interpretation is: an old process is
still heartbeating, a launcher run is pinned via `--version` or `--no-updates`,
or the operator is running a directly extracted release asset. The stats page
should explain that distinction; Nexus should not rewrite the reported client
version to the latest release.

The installed-release telemetry on `openagents.com/stats` is a separate source:
it comes from first-party npm/bootstrap `installer_finished` events and proves
which release assets users installed. It can correctly show
`pylon-v0.1.12` while online presence still has older `pylon/0.1.1` rows,
because old GCP or local fleet processes may still be heartbeating. If the
website shows only `pylon/0.1.1` online while a newer local Pylon is running,
first verify that production Nexus exposes `pylon_client_version_counts`, then
verify the website is reading that field rather than recomputing counts from
`recent_pylons`.

`pylon-v0.1.12` advertises `client_version=pylon/0.1.12` in the provider
presence heartbeat body. It also uses the compiled package version for Pylon
HTTP user agents. The online-version source of truth remains the provider
presence heartbeat, not model-download user agent strings.

## Build And Lockfile Pitfalls

The staged Nexus build context does not use only the repo-root `Cargo.lock`.
It also uses `apps/nexus-relay/deploy/Cargo.nexus.lock`. Refresh the deploy
lock from the staged context before building:

```bash
tmp_context="$(mktemp -d /tmp/openagents-nexus-lock-check.XXXXXX)"
scripts/deploy/nexus/stage-build-context.sh "$tmp_context"
(cd "$tmp_context" && cargo generate-lockfile)
cp "$tmp_context/Cargo.lock" apps/nexus-relay/deploy/Cargo.nexus.lock

verify_context="$(mktemp -d /tmp/openagents-nexus-lock-verify.XXXXXX)"
scripts/deploy/nexus/stage-build-context.sh "$verify_context"
(cd "$verify_context" && cargo fetch --locked)
```

The staged context must stay LDK-only. If
`scripts/deploy/nexus/test-ldk-deploy-invariants.sh` fails, remove the caller
or artifact instead of adding another runtime flag.

Do not run a fresh resolver pass against the full repo-root workspace as the
first repair. The deploy lock must reflect the staged Nexus image, not every
repo-root workspace member. If the deploy-lock diff rewrites large dependency
sections instead of mostly updating owned workspace package versions, stop and
inspect before deploying.

Before publishing Pylon binaries, run:

```bash
bash -n scripts/release/pylon-binary-release.sh
```

The release script must tolerate empty release flag arrays under `set -u`.

## Process And State Pitfalls

Do not run standalone `pylon training intake` or `pylon training sync` against
the same `PYLON_HOME` while a bare `pylon` process is running. The standalone
CLI and the long-running process share files but not the in-memory supervisor
slot. During the `0.1.6` proof this produced confusing validator behavior,
including overwritten invocation manifests and stale challenge leases.

If a bare `pylon` process is running, inspect and drive it through its admin
endpoint instead:

```bash
curl -fsS http://127.0.0.1:9468/v1/training/status | jq .
curl -fsS -X POST http://127.0.0.1:9468/v1/training/sync | jq .
```

If explicit standalone commands are required, stop the bare `pylon` process
first or use a completely separate `PYLON_HOME`.

Use a fresh proof root for public-style proof so stale local state cannot
satisfy the run accidentally:

```bash
PROOF_ROOT="var/proof/issue-4413-public-prod-017-$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -p "${PROOF_ROOT}/logs"

HOME="${PWD}/${PROOF_ROOT}/home" \
OPENAGENTS_DISABLE_TELEMETRY=1 \
npx --yes @openagentsinc/pylon@0.1.16 --version 0.1.16 \
  --pylon-home "${PWD}/${PROOF_ROOT}/home/.openagents/pylon" \
  --install-root "${PWD}/${PROOF_ROOT}/install" \
  --skip-diagnostics \
  --no-launch \
  --json | tee "${PROOF_ROOT}/bootstrap.json"

PYLON_DIR="${PWD}/${PROOF_ROOT}/install/versions/pylon-v0.1.16-darwin-arm64"
PYLON_HOME="${PWD}/${PROOF_ROOT}/home/.openagents/pylon"
HOME="/Users/christopherdavid" \
OPENAGENTS_PYLON_HOME="${PYLON_HOME}" \
OPENAGENTS_PYLON_CONFIG_PATH="${PYLON_HOME}/config.json" \
OPENAGENTS_DISABLE_TELEMETRY=1 \
PATH="${PYLON_DIR}:${PATH}" \
pylon 2>&1 | tee "${PROOF_ROOT}/logs/pylon-bare.log"
```

Do not isolate the entire process `HOME` while running the actual earning loop
unless you also provide `CARGO_HOME` and `RUSTUP_HOME`. The `0.1.7` public
proof failed once because synthetic `HOME` hid the installed Rust toolchain
from `rustup`; the correct isolation boundary is fresh Pylon state via
`OPENAGENTS_PYLON_HOME`, while preserving the normal user runtime prerequisites.

For a validator proof, use a second fresh `PYLON_HOME` and distinct admin and
checkpoint ports. Do not reuse the worker proof root:

```bash
pylon config set admin_listen_addr 127.0.0.1:9469
pylon config set training.checkpoint_serve_addr 127.0.0.1:9571
pylon config set training.role_claims validator
pylon
```

After accepted work appears, stop the proof Pylons so automatic starter work
does not keep generating extra accepted-work payouts. Then verify the worker
wallet directly:

```bash
pylon wallet balance --json
pylon wallet history --limit 20 --json
```

The strongest public-user proof is the worker Pylon wallet showing completed
receives whose payment ids match confirmed `accepted_work` Treasury records.

## Secret Handling

Workspace-local secret files are operator credentials, not complete shell
profiles. Preserve `PATH` before sourcing them:

```bash
old_path="$PATH"
set -a
source /Users/christopherdavid/work/.secrets/nexus-admin.env
set +a
PATH="$old_path"

token="${NEXUS_ADMIN_BEARER_TOKEN:-${NEXUS_CONTROL_ADMIN_BEARER_TOKEN:-}}"
```

Never paste raw bearer tokens, wallet mnemonics, or API keys into docs, issue
comments, receipts, commit messages, or terminal summaries.

## Production Deploy Checks

Use the scripted Nexus path only:

```bash
bash scripts/deploy/nexus/01-build-and-push-image.sh
DEPLOY_IMAGE="us-central1-docker.pkg.dev/openagentsgemini/openagents-nexus/nexus-relay:<git-short-sha>" \
  bash scripts/deploy/nexus/03-configure-and-start.sh
DEPLOY_IMAGE="us-central1-docker.pkg.dev/openagentsgemini/openagents-nexus/nexus-relay:<git-short-sha>" \
  bash scripts/deploy/nexus/04-verify-gates.sh
```

If the deploy wrapper waits on post-deploy payout smoke, do not immediately add
funds or redeploy. Inspect treasury status first. Pay attention to wallet
balance, wallet runtime status, active continuity alerts, accepted-work ledger
counts, and recent training payouts:

```bash
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

Accepted-work payout accounting must distinguish these states:

- `dispatching` rows reserve current wallet balance because a send is actively
  in flight.
- `dispatched` rows are reconciliation work and must not reserve current
  spendable balance forever.
- `confirmed` rows count against daily cap and reconciliation surfaces but do
  not reserve current spendable balance.

Only fund the wallet after the status surfaces prove the wallet is actually
short. A queued payout can be a dispatcher accounting bug even when the wallet
has enough sats.

## Production Latency And Treasury State Churn

If `04-verify-gates.sh` fails only on latency after a successful deploy and
treasury status shows the paid funding invoice has landed, inspect live disk
write churn before changing latency thresholds or redeploying blindly. A Nexus
relay that repeatedly rewrites a large treasury state file can make health,
stats, provider, and training rollout probes miss the gate even when CPU and
memory look healthy.

Use the VM-local process I/O counters and state-file size as the first check:

```bash
gcloud compute ssh nexus-mainnet-1 \
  --tunnel-through-iap \
  --project openagentsgemini \
  --zone us-central1-a \
  --command 'pid=$(pgrep -f /usr/local/bin/nexus-relay | head -n1); sudo awk "/write_bytes|wchar|syscw/ {print}" /proc/$pid/io; sleep 10; sudo awk "/write_bytes|wchar|syscw/ {print}" /proc/$pid/io; sudo ls -lh /var/lib/nexus-relay/treasury/treasury-state.json'
```

If writes grow by hundreds of megabytes or gigabytes over seconds, inspect the
hot file path with a bounded `strace`:

```bash
gcloud compute ssh nexus-mainnet-1 \
  --tunnel-through-iap \
  --project openagentsgemini \
  --zone us-central1-a \
  --command 'pid=$(pgrep -f /usr/local/bin/nexus-relay | head -n1); sudo timeout 5 strace -ff -tt -T -e trace=pwrite64,write,writev,openat,rename,fsync,fdatasync -p $pid -o /tmp/nexus-relay-strace; sudo grep -hE "treasury-state|pwrite64|write\\(|openat|rename|fsync|fdatasync" /tmp/nexus-relay-strace* | head -n 120'
```

The expected fixed behavior is that no-op payout queue refreshes do not rewrite
`treasury-state.json`, stale compactable `placeholder_liveness` payout records
are pruned, and accepted-work homework records are retained. If a future image
reintroduces repeated writes of `treasury-state.tmp` followed by rename to
`treasury-state.json`, treat it as a production bug in treasury state
persistence or retention. Do not call the issue complete by relaxing deploy
latency gates.

One common source of repeated treasury writes is payout-target registration
traffic from online Pylons after a restart. Challenge issuance should be
in-memory, and registering the same LDK payment target for the same node
identity should be an idempotent verification, not a fresh persistent receipt
on every heartbeat. If `strace` shows `treasury.payout_target.registered`
receipts or `treasury-state.tmp` writes at provider heartbeat pace while the
target address is unchanged, fix the idempotency path before rerunning deploy
gates.

## Treasury Funding Invoices

When the production treasury wallet is underfunded, generate a fresh Lightning
invoice through the hosted LDK funding-target path. Do not inspect or copy the
treasury mnemonic, do not manually edit wallet files, and do not infer payment
from invoice creation. Non-LDK funding targets are not part of new Pylon/Nexus
payment operations.

The detailed funding-invoice runbook is
`docs/deploy/NEXUS_TREASURY_FUNDING_INVOICE_RUNBOOK.md`.

For a `50,000` sat invoice:

```bash
curl -fsS -X POST "https://nexus.openagents.com/v1/treasury/funding-target" \
  -H "Authorization: Bearer ${token}" \
  -H "Content-Type: application/json" \
  --data '{
    "amount_sats": 50000,
    "description": "OpenAgents Nexus treasury funding",
    "expiry_seconds": 3600
  }' |
  jq '{bolt11_invoice, provider_payment_id_hash, phase_timings}'
```

The returned `bolt11_invoice` is the live payment request to hand to the payer.
It is not a secret, but it should still be handled as live payment material
rather than checked into docs, commits, or issue comments after use.

Important operational details:

- Include a positive `amount_sats` when the payer needs a BOLT11 invoice.
  Calling the endpoint without an amount is not a passing funding-invoice
  smoke. Standard LDK responses should include `bolt11_invoice`,
  `provider_payment_id_hash`, and `phase_timings`.
- A `504` from this endpoint means the bounded funding-target wallet operation
  timed out, often because the service just restarted or the wallet is busy. It
  is not proof that the wallet is unusable and it is not proof the invoice was
  paid. Wait for the service to settle, retry once, or use the private treasury
  runner if production keeps timing out.
- Do not claim an invoice was paid because `wallet_balance_sats` moved by a
  small amount during refresh. Balance refresh, cached wallet state, and old
  receives can change independently. Payment proof requires a status snapshot
  after the payer pays showing the funded balance or receive evidence, and the
  real closeout proof requires queued accepted-work payouts to dispatch or
  confirm.
- If a paid invoice does not appear while accepted-work payouts are queued with
  `wallet_balance_insufficient`, verify the live image includes the wallet
  refresh fix that treats those queued payouts as reconciliation work. Older
  images can record a cached funding-target balance and then skip later wallet
  refreshes because no payment has reached `dispatched` yet. That stale state
  keeps the wallet looking underfunded even after the invoice is paid.
- If deploy smoke rolls back because the wallet was underfunded, fund the
  wallet first, confirm `/v1/treasury/status`, and then redeploy the exact
  image. Do not treat rollback to an older image as a fix for insufficient
  funds.

## Final Proof Report

The closeout report and issue comment should include:

- pushed commit SHA
- Nexus image tag and digest
- deployed service verification receipt
- public Pylon release and npm version
- fresh proof root path
- worker node pubkey or npub
- validator node pubkey or npub when validator proof was required
- training run id
- window id
- contribution id
- final contribution outcome
- payout id or payment id
- treasury status showing dispatch or confirmation
- known gaps, if any remain

If any of these are missing, say that directly. Do not close the issue.
