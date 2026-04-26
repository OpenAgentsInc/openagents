# Pylon Homework Dispatch Operator Runbook

Published: 2026-04-22

This runbook is for operators who need to prove the public Pylon earning path
against hosted Nexus by running Pylon from npm, waiting for the hosted automatic
homework dispatcher or triggering a bounded manual override, and verifying
accepted-work payout into the Pylon wallet.

The target user story is:

```text
npx installs Pylon -> pylon stays online -> hosted Nexus dispatches homework
work -> Pylon claims the work -> work closes out -> validation accepts it ->
treasury pays -> Pylon wallet balance increases and treasury records a settled
accepted-work payout
```

## Preconditions

Use this runbook only after the relevant local proof path or focused regression
test is green. Production Nexus is a confirmation surface, not the debugger for
ordinary scheduler and payout bugs.

Minimum runtime requirements:

- public Pylon release asset `pylon-v0.1.15` or newer. The npm bootstrap
  package may still be invoked as `npx @openagentsinc/pylon`; the important
  version for earning and validation is the resolved standalone Pylon binary.
- production Nexus running the lease-priority fix that tries existing
  schedulable runs before auto-launching fresh hosted starter work
- production Nexus running the validator-priority fix that validates automatic
  homework first, manual admin-dispatched homework next, and hosted starter
  backlog last
- production Nexus running the homework validation-policy fix that validates
  homework-dispatch windows with the aggregate challenge only
- production Nexus running the closeout fix that treats aggregate-only
  homework validation as defensible for payout
- a normal user `HOME` for the running Pylon process so Rust and Psionic
  discovery work
- an isolated `OPENAGENTS_PYLON_HOME` for the proof
- local Gemma runtime visible to Pylon only when the proof also covers sellable
  inference. Hosted homework earning itself must not require an automatic Gemma
  download or diagnostic.
- compatible Psionic checkout discoverable by Pylon
- treasury wallet status connected and funded
- placeholder/liveness payouts disabled for this proof

Before production proof, run the focused Nexus regression:

```bash
cargo test -p nexus-control default_pylon_lease_claim
```

That filter must include these passing tests:

- `default_pylon_lease_claim_prefers_admin_dispatched_homework_before_auto_starter`
- `default_pylon_lease_claim_auto_launches_hosted_cs336_starter_work`
- `default_pylon_lease_claim_creates_fresh_starter_when_prior_run_is_exhausted`

Also run:

```bash
cargo test -p nexus-control training_validator_claim_run_priority_deprioritizes_hosted_starter_backlog
cargo test -p nexus-control training_validator_prioritizes_auto_homework_before_manual_and_generic_backlog
```

And run:

```bash
cargo test -p nexus-control validation_policy
```

Also run the automatic-dispatch regression before enabling or changing the
production loop:

```bash
cargo test -p nexus-control cs336_homework_auto_dispatch_cycle_targets_all_compatible_online_pylons
```

The homework validation-policy test must show that `homework_dispatch` and
`homework_auto_dispatch` keep the aggregate validator challenge and skip
per-contribution sample challenges. Use Pylon `0.1.15` or newer for current npm
proofs. `0.1.8` fixed the validator
replay case where a retained claim can point at stale same-host local target
bytes: Pylon falls back to the bridge-inline payload or rewrites the target
artifact id to match the materialized digest. `0.1.10` adds the
Autopilot-controlled earning proof fixes: default Spark payout destination
creation in the long-lived serve path, retained snapshot reuse for validator
replay retries, and stricter Autopilot paid-state projection. Do not re-enable
sample challenges for homework dispatch until the per-contribution sample
replay path is separately fixed and proven with npm Pylon. Pylon `0.1.11`
made the TUI manage the earning worker, removed the composer/transcript from
the default homework surface, and kept Gemma diagnostics/downloads opt-in.
Pylon `0.1.12` fixed issue #4414: Mac Psionic training jobs prefer the operator's current
`target/release/psionic-train` build and fall back to `cargo run --release`
instead of debug Cargo. Pylon `0.1.15` is the current recommended user-path
floor because it also blocks terminal window seal until the worker contribution
artifact bundle has uploaded and verified, preventing validator replay from
hitting 404s against sealed contributions.

## Online Version Telemetry

The stats page has two intentionally separate Pylon version surfaces. Installed
release counts come from `openagents.com` first-party installer telemetry.
Online client-version counts come from Nexus provider presence. Do not compare
the installed-release count to `recent_pylons` directly: `recent_pylons` is a
small capped sample of the newest public rows, not the whole online fleet. A
busy set of older nodes can fill that sample and make a fresh `pylon-v0.1.12`
session appear missing.

The fleet-wide online version source is `pylon_client_version_counts` in
`GET https://nexus.openagents.com/api/stats`. That field counts every live
provider-presence session by the heartbeat body's `client_version` value during
the current stale window. Use it when validating that a newly installed public
Pylon is visible online. Use `recent_pylons` only for row-level debugging and
last-seen inspection.

If installed telemetry shows `pylon-v0.1.12` but online counts are dominated by
`pylon/0.1.1`, the likely causes are old fleet processes still heartbeating,
the website reading the old capped sample, or Nexus not yet deployed with the
online-version histogram. `pylon-v0.1.12` sends `client_version=pylon/0.1.12`
in provider presence and derives Pylon HTTP user agents from
`CARGO_PKG_VERSION`; the provider-presence histogram remains the source of
truth for online versions.

## Release `pylon-v0.1.15` Preparation

Use `pylon-v0.1.15` as the current release floor for public homework earning.
It keeps the `0.1.14` long-ID path hashing and TUI worker-exit fixes, then
fixes issue #4449: terminal workers must upload and verify the retained
contribution artifact bundle before Pylon calls the Nexus window-seal endpoint.
New homework dispatches should require `min_pylon_version=0.1.15` so validators
do not receive sealed contributions whose signed artifact fetch still returns
404.

Before tagging `0.1.15`, run:

```bash
cargo test -p pylon-tui
cargo test -p pylon --bin pylon
cargo test -p pylon training_terminal_sync -- --nocapture --test-threads=1
cd packages/pylon-bootstrap && bun test
```

Then prove from a fresh Pylon home that a `pylon-v0.1.15` worker can claim a
fresh hosted homework assignment, upload contribution artifacts before seal,
allow a separate validator to replay the contribution, close out accepted work,
and receive a settled accepted-work payout in the worker wallet.

The published `0.1.15` release receipt is:

```text
docs/reports/nexus/20260426-pylon-v0.1.15-release.json
```

## Release `pylon-v0.1.12` Preparation

Use `pylon-v0.1.12` as the prior release floor for public homework earning.
It keeps the `0.1.11` TUI-managed worker and opt-in Gemma behavior, then fixes
the issue #4414 training launch failure: Pylon uses a current
`target/release/psionic-train` binary from the compatible Psionic checkout when
present and otherwise runs `cargo run --release`. It is no longer recommended
as the dispatch floor because issue #4449 showed later terminal closeout
ordering bugs can make sealed contribution artifacts unreachable for validator
replay.

Before tagging `0.1.12`, run:

```bash
cargo test -p pylon-tui
cargo test -p pylon --bin pylon
cargo test -p pylon --lib psionic_train_supervisor_command
cargo test -p pylon --lib training_supervisor_records_logs_heartbeat_and_failure_receipt_on_failed_exit
cd packages/pylon-bootstrap && bun test
```

Then prove from a fresh Pylon home that `pylon-tui` starts a child worker, the
admin listener comes up under that worker, no Gemma model cache is created
unless `--download-curated-cache` or `--run-diagnostics` is explicitly passed,
and a compatible Psionic checkout with `target/release/psionic-train` is
launched directly for homework/training manifests.

The published `0.1.12` release receipt is:

```text
docs/reports/nexus/20260423-issue-4414-pylon-v0.1.12-release.json
```

That receipt proves the release assets, npm bootstrap path, Linux asset smoke,
and the issue #4414 supervisor-command regression.

## Release `pylon-v0.1.11` Preparation

Use `pylon-v0.1.11` as the prior release floor for public homework earning.
It keeps the `0.1.10` payout and hosted-starter fixes, then corrects the user
surface: the npm bootstrap and interactive `pylon` command open the minimal
homework dashboard, that dashboard starts and supervises the real worker, and
Gemma diagnostics/downloads stay opt-in. New homework dispatches should require
`min_pylon_version=0.1.11` so users running the TUI are actually online for
jobs.

Before tagging `0.1.11`, run:

```bash
cargo test -p pylon-tui
cargo test -p pylon --bin pylon
cd packages/pylon-bootstrap && bun test
```

Then prove from a fresh Pylon home that `pylon-tui` starts a child worker,
the admin listener comes up under that worker, the visible UI contains no
composer/transcript, and no Gemma model cache is created unless
`--download-curated-cache` or `--run-diagnostics` is explicitly passed.

The published `0.1.11` release receipt is:

```text
docs/reports/nexus/20260423-072712-pylon-v0.1.11-release.json
```

That receipt proves the release asset and npm bootstrap path, verifies that the
dashboard starts the real worker, verifies no default Gemma model cache was
created, and records a production homework lease that reached released/sealed
local closeout state.

The full `0.1.11` production earning receipt is:

```text
docs/reports/nexus/20260423-080422-pylon-v0.1.11-prod-e2e.json
```

That receipt is the current public-release proof. It used fresh
npm-installed `0.1.11` worker and validator Pylon homes, dispatched one
25-sat CS336 homework run with `min_pylon_version=0.1.11`, observed
`pylon/0.1.11` in the Nexus online-version histogram, reconciled the target
window as `rewarded`, confirmed and settled the accepted-work treasury payout,
showed the worker wallet moving from `0` to `25` sats, and verified that no
Gemma model files were downloaded by default.

## Release `pylon-v0.1.10` Preparation And Proof

The first public release that should be used for the current paid-training
claim is `pylon-v0.1.10`, published as `@openagentsinc/pylon@0.1.10`.
Future agents should not reconstruct the release from memory. Use the checked-in
receipt as source evidence:

```text
docs/reports/nexus/20260423-050434-pylon-v0.1.10-release.json
```

That release was cut from:

```text
release tag: pylon-v0.1.10
version: 0.1.10
commit: 8b814d800b6f4291892a1bcc835fb34a2b91fee1
GitHub release: https://github.com/OpenAgentsInc/openagents/releases/tag/pylon-v0.1.10
npm package: @openagentsinc/pylon@0.1.10
```

The GitHub release asset used for the Mac proof was:

```text
pylon-v0.1.10-darwin-arm64.tar.gz
sha256: a63a9ca8fa32dd05d9815f5087c19faa9a70f250b38a29d193274f07e9149e5d
```

The npm package metadata recorded in the receipt was:

```text
latest: 0.1.10
dist shasum: 1ba6c175fbf4627c8a6dc577dd6cdc3347b5327e
dist integrity: sha512-rKJq4CrmrwUXE3MhgZgkfqMsWywXGOX816Q2R6clhkc41isDZzU+s6FKpl0KayA5uSj693Mb/aI5InA0Lqx9fA==
```

The release preparation had two distinct gates.

First, run the source-level and Autopilot-control gates before tagging:

```bash
cargo test -p pylon config_set_updates_payout_destination
cargo test -p pylon default_payout_destination_uses_wallet_spark_address
cargo test -p pylon snapshot_training_retained_artifact_binding
cargo check -p pylon
cargo check -p autopilot
cargo test -p autopilot --lib
scripts/autopilot/tauri-control-smoke.sh --homework-handshake --timeout-ms 600000
```

Those gates prove the fixes that made `0.1.10` release-worthy: the long-lived
serve path creates a local Spark payout destination before advertising paid
training eligibility, retained artifact replay can reuse an existing
content-addressed snapshot instead of failing on mutable path drift, and the
Autopilot paid-state projection reports the current homework/payout state
instead of stale historical issues.

Second, after publishing the GitHub release and npm package, run the release
asset smoke from a fresh proof root with `--no-launch`:

```bash
PROOF_ROOT="/private/tmp/pylon-0.1.10-release-smoke-$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -p "${PROOF_ROOT}/logs"

HOME="/Users/christopherdavid" \
OPENAGENTS_PSIONIC_REPO="/Users/christopherdavid/work/psionic" \
npx --yes @openagentsinc/pylon \
  --version 0.1.10 \
  --pylon-home "${PROOF_ROOT}/pylon-home" \
  --config-path "${PROOF_ROOT}/pylon-home/config.json" \
  --install-root "${PROOF_ROOT}/install" \
  --skip-diagnostics \
  --no-launch \
  2>&1 | tee "${PROOF_ROOT}/logs/npm-bootstrap.log"
```

The `0.1.10` release smoke recorded `cached: false`, `install_method:
"release_asset"`, checksum verification against the published archive, working
`pylon` and `pylon-tui` help output, and a long-lived `pylon` serve log with
both of these lines:

```text
pylon: created local Spark payout destination for paid training work
pylon: node pylon is online; running default online earning loop
```

The final release proof then ran a fresh npm-installed `0.1.10` worker against
production Nexus and hosted homework dispatch:

```text
proof root: /private/tmp/pylon-0.1.10-prod-e2e-20260423T050920Z
network id: trainnet.cs336.a1.pylon-0.1.10-release.20260423T050920Z
batch id: dispatch.cs336.a1.20260423051031.fa7d95a2
training run id: run.cs336.a1.pylon010-prod-20260423051030_20260423051031_fa7d95a2_0001.20260423051031.46951e63
window id: window.cs336.a1.pylon010-prod-20260423051030_20260423051031_fa7d95a2_0001.20260423051031.46951e63.0001
worker pubkey: 0faf4e72304ea8ae0f84eb9a60df0b4d4484e3265c89e5e8a4fd332b057be90e
worker release id: openagents.pylon@0.1.10
worker build version: 0.1.10
worker build digest: sha256:7b1a2e79255ac893cdd6581a771b85293fb8485bde5892f7648ab4f79e6e1d84
```

That production run ended with:

```text
latest closeout status: rewarded
featured window status: reconciled
accepted contributors: 1
payout eligible: true
accepted outcome id: accepted.training_window.window.cs336.a1.pylon010-prod-20260423051030_20260423051031_fa7d95a2_0001.20260423051031.46951e63.0001
payout receipt id: 019db8c1-6639-7751-a717-cee14dd2012e
payout reconciliation status: settled
treasury payout class: accepted_work
treasury payout amount: 25 sats
worker wallet delta: 0 sats -> 25 sats
wallet receive status: completed
```

The proof still observed `run_backlog` and `validator_backlog` caveats while
the bounded run was moving through the queue. Those were not blockers because
the accepted-work payout was confirmed and settled, and the npm-installed
worker wallet balance increased by the paid amount. A follow-up Nexus health
fix also stopped idle stale wallet-sync metadata from surfacing as
`snapshot_stale` / `wallet_snapshot_stale` when the wallet is connected and no
accepted-work payout reconciliation is pending. Do not confuse stale snapshot
metadata with the earlier insufficient-funds failure mode.

## Check Treasury Before Dispatch

Load the operator env without printing tokens:

```bash
old_path="$PATH"
set -a
source /Users/christopherdavid/work/.secrets/nexus-admin.env
set +a
PATH="$old_path"
token="${NEXUS_CONTROL_ADMIN_BEARER_TOKEN:-${NEXUS_ADMIN_BEARER_TOKEN:-}}"
```

Check treasury status:

```bash
curl -fsS -H "Authorization: Bearer ${token}" \
  "${NEXUS_BASE_URL}/v1/treasury/status" |
  jq '{
    wallet_runtime_status,
    wallet_balance_sats,
    placeholder_payout_mode,
    accepted_work: (.training_payout_ledger_summary.accepted_work // null),
    active_continuity_alerts,
    recent_training_payouts: (.recent_training_payouts // [])[:5]
  }'
```

Do not proceed if `wallet_runtime_status` is not `connected`, if the wallet is
underfunded for the planned dispatch, or if placeholder payouts are enabled and
would confuse proof. A stale snapshot warning is not automatically fatal when
the wallet runtime is connected and accepted-work payout records can still be
verified, but record it in the proof notes.

## Start A Fresh Npm Pylon Worker

Use a fresh proof root. Keep Pylon state isolated, but keep the real user
`HOME`:

```bash
PROOF_ROOT="/private/tmp/pylon-npm-e2e-$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -p "${PROOF_ROOT}/logs"
printf '%s\n' "${PROOF_ROOT}" > /private/tmp/pylon-npm-e2e-latest-root

NETWORK_ID="trainnet.cs336.a1.$(date -u +%Y%m%dT%H%M%SZ)"
printf '%s\n' "${NETWORK_ID}" > "${PROOF_ROOT}/network-id.txt"

HOME="/Users/christopherdavid" \
OPENAGENTS_PSIONIC_REPO="/Users/christopherdavid/work/psionic" \
npx --yes @openagentsinc/pylon \
  --version 0.1.15 \
  --pylon-home "${PROOF_ROOT}/pylon-home" \
  --config-path "${PROOF_ROOT}/pylon-home/config.json" \
  --install-root "${PROOF_ROOT}/install" \
  --skip-diagnostics \
  --no-launch \
  2>&1 | tee "${PROOF_ROOT}/logs/npm-worker-bootstrap.log"
```

The bootstrap should report a fresh prebuilt release asset and checksum
verification. In a noninteractive operator shell, do not let the npm launcher
start the default TUI. The launcher opens `pylon-tui` by default, and a
noninteractive shell can fail with `Device not configured`. Use `--no-launch`
for the bootstrap, then run the installed `pylon` binary directly.

Configure the worker:

```bash
PYLON_BIN="${PROOF_ROOT}/install/versions/pylon-v0.1.15-darwin-arm64/pylon"

HOME="/Users/christopherdavid" \
OPENAGENTS_PYLON_HOME="${PROOF_ROOT}/pylon-home" \
OPENAGENTS_PYLON_CONFIG_PATH="${PROOF_ROOT}/pylon-home/config.json" \
"${PYLON_BIN}" config set training.allowed_networks "${NETWORK_ID}"

HOME="/Users/christopherdavid" \
OPENAGENTS_PYLON_HOME="${PROOF_ROOT}/pylon-home" \
OPENAGENTS_PYLON_CONFIG_PATH="${PROOF_ROOT}/pylon-home/config.json" \
"${PYLON_BIN}" config set training.role_claims worker

HOME="/Users/christopherdavid" \
OPENAGENTS_PYLON_HOME="${PROOF_ROOT}/pylon-home" \
OPENAGENTS_PYLON_CONFIG_PATH="${PROOF_ROOT}/pylon-home/config.json" \
"${PYLON_BIN}" config set training.relay_urls "wss://nexus.openagents.com/"
```

Run the worker:

```bash
HOME="/Users/christopherdavid" \
OPENAGENTS_PYLON_HOME="${PROOF_ROOT}/pylon-home" \
OPENAGENTS_PYLON_CONFIG_PATH="${PROOF_ROOT}/pylon-home/config.json" \
OPENAGENTS_PSIONIC_REPO="/Users/christopherdavid/work/psionic" \
"${PYLON_BIN}" 2>&1 | tee "${PROOF_ROOT}/logs/pylon-worker.log"
```

The worker log should report a payout destination and then:

```text
pylon: node pylon is online; running default online earning loop
```

In another terminal, check the local admin status:

```bash
curl -fsS http://127.0.0.1:9468/v1/training/status | jq .
```

## Optional Validator Process For Proof

Use this only when production has a validator backlog or when you need a
self-contained proof with one worker and one validator on the same machine.
Use a second Pylon home and distinct ports.

For the npm end-to-end proof, start the worker first, trigger the homework run,
and wait until the remote run detail shows the target window is `sealed` with
`total_contributions: 1`. Then start the validator. Starting the validator
before the worker artifact is visible can lease a challenge against an object
that has not landed in the artifact bucket yet.

Bootstrap without launching:

```bash
PROOF_ROOT="$(cat /private/tmp/pylon-npm-e2e-latest-root)"
VAL_ROOT="${PROOF_ROOT}/validator"
mkdir -p "${VAL_ROOT}/logs"

HOME="/Users/christopherdavid" \
OPENAGENTS_PSIONIC_REPO="/Users/christopherdavid/work/psionic" \
npx --yes @openagentsinc/pylon \
  --version 0.1.15 \
  --pylon-home "${VAL_ROOT}/pylon-home" \
  --install-root "${VAL_ROOT}/install" \
  --skip-diagnostics \
  --no-launch \
  2>&1 | tee "${VAL_ROOT}/logs/npm-validator-bootstrap.log"
```

Configure validator-only role claims:

```bash
VAL_BIN="${VAL_ROOT}/install/versions/pylon-v0.1.15-darwin-arm64/pylon"
NETWORK_ID="$(cat "${PROOF_ROOT}/network-id.txt")"

HOME="/Users/christopherdavid" \
OPENAGENTS_PYLON_HOME="${VAL_ROOT}/pylon-home" \
OPENAGENTS_PYLON_CONFIG_PATH="${VAL_ROOT}/pylon-home/config.json" \
"${VAL_BIN}" config set admin_listen_addr 127.0.0.1:9469

HOME="/Users/christopherdavid" \
OPENAGENTS_PYLON_HOME="${VAL_ROOT}/pylon-home" \
OPENAGENTS_PYLON_CONFIG_PATH="${VAL_ROOT}/pylon-home/config.json" \
"${VAL_BIN}" config set training.checkpoint_serve_addr 127.0.0.1:9571

HOME="/Users/christopherdavid" \
OPENAGENTS_PYLON_HOME="${VAL_ROOT}/pylon-home" \
OPENAGENTS_PYLON_CONFIG_PATH="${VAL_ROOT}/pylon-home/config.json" \
"${VAL_BIN}" config set training.role_claims validator

HOME="/Users/christopherdavid" \
OPENAGENTS_PYLON_HOME="${VAL_ROOT}/pylon-home" \
OPENAGENTS_PYLON_CONFIG_PATH="${VAL_ROOT}/pylon-home/config.json" \
"${VAL_BIN}" config set training.allowed_networks "${NETWORK_ID}"

HOME="/Users/christopherdavid" \
OPENAGENTS_PYLON_HOME="${VAL_ROOT}/pylon-home" \
OPENAGENTS_PYLON_CONFIG_PATH="${VAL_ROOT}/pylon-home/config.json" \
"${VAL_BIN}" config set training.relay_urls "wss://nexus.openagents.com/"
```

Run the validator:

```bash
HOME="/Users/christopherdavid" \
OPENAGENTS_PYLON_HOME="${VAL_ROOT}/pylon-home" \
OPENAGENTS_PYLON_CONFIG_PATH="${VAL_ROOT}/pylon-home/config.json" \
OPENAGENTS_PSIONIC_REPO="/Users/christopherdavid/work/psionic" \
"${VAL_BIN}" 2>&1 | tee "${VAL_ROOT}/logs/pylon-validator.log"
```

Check validator status:

```bash
curl -fsS http://127.0.0.1:9469/v1/training/status | jq .
```

## Trigger Manual Homework Work From A Separate Process

Use a simple slug prefix. Avoid relying on shell variables named `status` in
zsh because `status` is read-only.

```bash
PROOF_ROOT="$(cat /private/tmp/pylon-npm-e2e-latest-root)"
NETWORK_ID="$(cat "${PROOF_ROOT}/network-id.txt")"
RUN_PREFIX="codex-npm-e2e-$(date -u +%Y%m%d%H%M%S)"
printf '%s\n' "${RUN_PREFIX}" > "${PROOF_ROOT}/run-prefix.txt"

payload="$(jq -nc \
  --arg prefix "${RUN_PREFIX}" \
  --arg min_version "0.1.15" \
  --arg network_id "${NETWORK_ID}" \
  '{
    run_count: 1,
    max_contributors_per_run: 1,
    amount_sats: 25,
    total_budget_sats: 25,
    run_slug_prefix: $prefix,
    reuse_existing_run: false,
    only_online: true,
    min_pylon_version: $min_version,
    require_updated_build: false,
    network_id: $network_id,
    window_duration_seconds: 600
  }')"
printf '%s\n' "${payload}" > "${PROOF_ROOT}/dispatch-request.json"

http_status="$(curl -sS \
  -o "${PROOF_ROOT}/dispatch-response.json" \
  -w '%{http_code}' \
  -X POST "${NEXUS_BASE_URL}/v1/admin/homework/cs336-a1/dispatch" \
  -H "Authorization: Bearer ${token}" \
  -H "Content-Type: application/json" \
  -d "${payload}")"
echo "HTTP_STATUS=${http_status}"
cat "${PROOF_ROOT}/dispatch-response.json" | jq .
```

A successful response has:

- `launch_state: "created"` or `launch_state: "reused"`
- `launch_phase: "leaseable"`
- one `assigned_pylons` entry for the npm Pylon worker
- `artifact_prefix` under the intended isolated training network

Record the run id:

```bash
jq -r '.launches[0].training_run_id' "${PROOF_ROOT}/dispatch-response.json" \
  > "${PROOF_ROOT}/triggered-run-id.txt"
```

## Automatic 10-Minute Dispatch

Production Nexus now owns the normal homework pacing loop. The public user
contract remains only `pylon`: when an eligible node is online, Nexus can assign
the current hosted CS336 A1 homework work without the user running a course
opt-in command or an admin dispatch command.

The deployed loop is controlled by the Nexus runtime environment:

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

Each cycle creates one fresh `homework_auto_dispatch` run with a slug prefix
under `run.cs336.a1.auto_10m_`, targets online Pylons on the default
`trainnet.cs336.a1.demo` network, and pays only accepted homework closeouts.
The first cycle runs as soon as the Nexus process starts, then repeats every
configured interval. The in-process guard skips overlapping cycles instead of
running two dispatches concurrently after a slow cycle or delayed timer tick.

Automatic dispatch still uses the same launch, assignment, validation, closeout,
treasury, and public-stats path as the manual admin endpoint. A successful
cycle should make `/api/stats` advance through the normal sequence:

- online/assigned Pylon counts can change when the Pylon heartbeat and launch
  projection refresh
- accepted-work and strong-lane counters tick only after the homework window is
  accepted/rewarded
- `nexusPayoutSatsPaidTotal`, the accepted-work payout total, and the worker
  wallet balance tick only after treasury dispatch confirms or settles the
  accepted-work payout

For production verification, keep at least one fresh `pylon-v0.1.15` worker
online on the default network before restarting Nexus. Because the first
automatic cycle runs immediately on process start, starting the worker first
avoids waiting the full 10-minute interval for the next cycle. After the worker
seals its contribution, start or confirm a validator and use the same
"Wait For Validation And Payout" and "Verify Accepted-Work Payment" sections
below.

The `2026-04-23T10:12:09Z` production proof for this automatic path is tracked
at
`docs/reports/nexus/20260423-101209-cs336-auto-dispatch-prod-e2e.json`.
That proof deployed
`us-central1-docker.pkg.dev/openagentsgemini/openagents-nexus/nexus-relay:33ee54cf6c51`,
started a fresh npm-installed `pylon-v0.1.11` worker, and then started a
separate `pylon-v0.1.11` validator. Production Nexus created two consecutive
`run.cs336.a1.auto_10m_*` homework runs at the 10-minute cadence:
`run.cs336.a1.auto_10m_20260423095802_717db136_0001.20260423095802.d40e370b`
and
`run.cs336.a1.auto_10m_20260423100802_031fda4f_0001.20260423100802.b9748594`.
Both windows reached `reconciled`, both closeouts reached `rewarded`, both runs
accepted one contribution, the worker wallet moved from `0` to `50` sats, and
Nexus accepted-work payout stats advanced by `50` sats.

One operational detail from that proof matters for future agents: the worker
can temporarily show `trn_publish_retry` / `queued_retry` for contribution,
proof, or sealed-window TRN records after the local homework work is complete.
Do not treat that as a failed job if the local closeout bundle exists and the
issue is relay connectivity. Run `pylon training status --json`,
`pylon training publish --json`, or keep the foreground Pylon loop alive and let
it retry. In the proof above, the assignment and run metadata published first,
the contribution/proof/sealed-window records recovered on a later pass, and the
hosted run then moved from `sealed` with `replay_required=1` to `reconciled` /
`rewarded` after validator replay.

## Cron-Compatible Dispatch

The manual endpoint remains the operator override control for paid homework.
Use it when you need a bounded proof, a one-off smoke, or a temporary payout
pace different from the automatic 10-minute loop. Control payout rate with
`run_count`, `max_contributors_per_run`, `amount_sats`, and
`total_budget_sats`.

This example dispatches at most four paid workers per interval, paying 25 sats
only for accepted work. Duplicate work across intervals is allowed by design
because every invocation creates a fresh run slug.

```bash
old_path="$PATH"
set -a
source /Users/christopherdavid/work/.secrets/nexus-admin.env
set +a
PATH="$old_path"
token="${NEXUS_CONTROL_ADMIN_BEARER_TOKEN:-${NEXUS_ADMIN_BEARER_TOKEN:-}}"

batch_slug="cron.cs336.a1.$(date -u +%Y%m%d%H%M%S)"
payload="$(jq -nc \
  --arg prefix "${batch_slug}" \
  --arg min_version "0.1.15" \
  '{
    run_count: 4,
    max_contributors_per_run: 1,
    amount_sats: 25,
    total_budget_sats: 100,
    run_slug_prefix: $prefix,
    reuse_existing_run: false,
    only_online: true,
    min_pylon_version: $min_version,
    require_updated_build: false,
    window_duration_seconds: 900,
    continue_on_error: false
  }')"

curl -fsS \
  -X POST "${NEXUS_BASE_URL}/v1/admin/homework/cs336-a1/dispatch" \
  -H "Authorization: Bearer ${token}" \
  -H "Content-Type: application/json" \
  -d "${payload}" |
  jq '{
    batch_id,
    requested_run_count,
    launched_run_count,
    failed_run_count,
    max_payout_sats,
    duplicate_work_allowed,
    runs: [.launches[] | {
      training_run_id,
      launch_state,
      launch_phase,
      assigned_pylons
    }],
    errors
  }'
```

For live payout pacing, keep `total_budget_sats` equal to or lower than
`run_count * max_contributors_per_run * amount_sats`. The API rejects overbudget
payloads before creating runs. Use `only_online: true` for the current public
earning path so dispatches target Pylons that are actually online for relevant
jobs. Do not require a user to opt into CS336 homework manually; running
`pylon` and staying online is the operator-facing contract.

## Wait For Worker Contribution

Do not start the optional validator process until the worker contribution has
materialized remotely.

```bash
RUN_ID="$(cat "${PROOF_ROOT}/triggered-run-id.txt")"

until curl -fsS -H "Authorization: Bearer ${token}" \
  "${NEXUS_BASE_URL}/api/training/runs/${RUN_ID}" \
  -o "${PROOF_ROOT}/run-detail-worker-ready.json" &&
  jq -e '
    .featured_window.status == "sealed" and
    (.featured_window.total_contributions // 0) >= 1
  ' "${PROOF_ROOT}/run-detail-worker-ready.json" >/dev/null
do
  date -u +"waiting for worker contribution at %Y-%m-%dT%H:%M:%SZ"
  sleep 10
done

jq '{
  run: .run,
  featured_window: .featured_window,
  queue_pressure: .queue_pressure,
  caveats: .caveats
}' "${PROOF_ROOT}/run-detail-worker-ready.json"
```

For current homework-dispatch runs, expect one aggregate validator challenge and
no contribution-sample challenge. If a proof run shows a sample challenge for a
`homework_dispatch` run, the deployed Nexus build is stale.

If local Pylon status shows that the worker sealed the window but the hosted run
detail still shows `total_contributions: 0`, force the worker-side publication
path once:

```bash
PYLON_BIN="${PROOF_ROOT}/install/versions/pylon-v0.1.15-darwin-arm64/pylon"

HOME="/Users/christopherdavid" \
OPENAGENTS_PYLON_HOME="${PROOF_ROOT}/pylon-home" \
OPENAGENTS_PYLON_CONFIG_PATH="${PROOF_ROOT}/pylon-home/config.json" \
"${PYLON_BIN}" training sync --json | tee "${PROOF_ROOT}/training-sync-worker.json"

HOME="/Users/christopherdavid" \
OPENAGENTS_PYLON_HOME="${PROOF_ROOT}/pylon-home" \
OPENAGENTS_PYLON_CONFIG_PATH="${PROOF_ROOT}/pylon-home/config.json" \
"${PYLON_BIN}" training refresh --json | tee "${PROOF_ROOT}/training-refresh-worker.json"
```

This is a publication/relay catch-up step, not a replacement for the normal
background worker loop. Use it when the worker log shows relay-connectivity
retries or the local admin status is ahead of Nexus.

## Wait For Validation And Payout

After the optional validator is running, poll the run until the window is
reconciled and payout-eligible:

```bash
RUN_ID="$(cat "${PROOF_ROOT}/triggered-run-id.txt")"

until curl -fsS -H "Authorization: Bearer ${token}" \
  "${NEXUS_BASE_URL}/api/training/runs/${RUN_ID}" \
  -o "${PROOF_ROOT}/run-detail-accepted.json" &&
  jq -e '
    (.featured_window.status == "reconciled" or .featured_window.payout_eligible == true) and
    (.run.accepted_contributors // 0) >= 1
  ' "${PROOF_ROOT}/run-detail-accepted.json" >/dev/null
do
  date -u +"waiting for validation/payout at %Y-%m-%dT%H:%M:%SZ"
  sleep 10
done

jq '{
  run: .run,
  featured_window: .featured_window,
  treasury: .treasury,
  caveats: .caveats
}' "${PROOF_ROOT}/run-detail-accepted.json"
```

If the validator is online but the window does not move out of `sealed`, force
validator intake and sync once:

```bash
VAL_ROOT="${PROOF_ROOT}/validator"
VAL_BIN="${VAL_ROOT}/install/versions/pylon-v0.1.15-darwin-arm64/pylon"

HOME="/Users/christopherdavid" \
OPENAGENTS_PYLON_HOME="${VAL_ROOT}/pylon-home" \
OPENAGENTS_PYLON_CONFIG_PATH="${VAL_ROOT}/pylon-home/config.json" \
"${VAL_BIN}" training intake --json | tee "${VAL_ROOT}/training-intake-validator.json"

HOME="/Users/christopherdavid" \
OPENAGENTS_PYLON_HOME="${VAL_ROOT}/pylon-home" \
OPENAGENTS_PYLON_CONFIG_PATH="${VAL_ROOT}/pylon-home/config.json" \
"${VAL_BIN}" training sync --json | tee "${VAL_ROOT}/training-sync-validator.json"
```

Stop the proof Pylon processes after the evidence has been captured:

```bash
pkill -TERM -f "${PROOF_ROOT}" || true
```

## Failure Modes

Watch the local Pylon training status while the proof is running:

```bash
curl -fsS http://127.0.0.1:9468/v1/training/status |
  jq '{
    active_runtime,
    leased_assignment,
    recent_closeouts: (.recent_closeouts // [])[:5],
    recent_issues: (.recent_issues // [])[:5]
  }'
```

If the triggered run stays at `total_contributions: 0` while the Pylon keeps
executing `run.cs336.a1.starter.*`, production Nexus is missing the
lease-priority fix. The broken behavior is: default worker lease claims
auto-launch fresh hosted starter work before trying existing admin-dispatched
runs. The fixed behavior is: default worker lease claims try existing
schedulable runs first and auto-launch hosted starter work only after
`training_scheduler_run_not_found`,
`training_scheduler_run_not_schedulable`, or
`training_scheduler_assignment_unavailable`.

If the triggered run reaches `total_contributions: 1` and `status: "sealed"`
but stays at `accepted_contributions: 0` while the validator process claims
old homework or `run.cs336.a1.starter.*` challenges first, production Nexus is
missing the current validator-priority fix. The fixed behavior is: validator
claims prioritize `run_kind: "homework_auto_dispatch"` windows first,
`run_kind: "homework_dispatch"` windows second, normal runs third, and hosted
starter backlog last.

If a `homework_dispatch` or `homework_auto_dispatch` run has one aggregate
challenge plus one contribution-sample challenge, production Nexus is missing
the homework validation-policy fix. The fixed behavior is: homework-dispatch
windows use the aggregate challenge only until the released npm Pylon
contribution-sample replay path stops producing artifact-manifest digest drift.

If the validator log reports an `artifact_digest_mismatch` where the target
`contribution_artifact_manifest` artifact id digest differs from the materialized
target bytes, the validator is running an older Pylon release. Upgrade to
`pylon-v0.1.15` or newer and retry the run. Current releases repair stale
retained target artifact ids and fall back away from mismatched local same-host
target files.

If the validator finalizes as verified but the hosted window stays
`replay_required` or `refused` instead of `rewarded`, production Nexus is
missing the aggregate-only closeout reward fix. The broken behavior is: Nexus
accepts the aggregate challenge, but reconciliation has no per-sample
contribution disposition to map back onto the contribution, so accepted work is
treated as refused. The fixed behavior is deployed from `fb60b9167` or newer:
for homework-dispatch aggregate-only validation, Nexus applies the aggregate
terminal disposition to the contribution outcome.

## Verify Accepted-Work Payment

After the run has accepted work, check treasury:

```bash
curl -fsS -H "Authorization: Bearer ${token}" \
  "${NEXUS_BASE_URL}/v1/treasury/status" |
  jq '{
    wallet_runtime_status,
    wallet_balance_sats,
    accepted_work: (.training_payout_ledger_summary.accepted_work // null),
    recent_training_payouts: (.recent_training_payouts // [])[:10],
    active_continuity_alerts
  }'
```

Verify the worker wallet directly:

```bash
PROOF_ROOT="$(cat /private/tmp/pylon-npm-e2e-latest-root)"
PYLON_BIN="${PROOF_ROOT}/install/versions/pylon-v0.1.15-darwin-arm64/pylon"

HOME="/Users/christopherdavid" \
OPENAGENTS_PYLON_HOME="${PROOF_ROOT}/pylon-home" \
OPENAGENTS_PYLON_CONFIG_PATH="${PROOF_ROOT}/pylon-home/config.json" \
"${PYLON_BIN}" wallet balance --json

HOME="/Users/christopherdavid" \
OPENAGENTS_PYLON_HOME="${PROOF_ROOT}/pylon-home" \
OPENAGENTS_PYLON_CONFIG_PATH="${PROOF_ROOT}/pylon-home/config.json" \
"${PYLON_BIN}" wallet history --limit 20 --json
```

The proof is complete when treasury shows a confirmed and settled
`accepted_work` payout for the triggered run, contribution, and worker payout
target, and the worker wallet balance reflects the paid sats. Current Spark
wallet history can return an empty `payments` list for this internal Spark
receive even when the balance increased, so do not fail an otherwise complete
proof only because wallet history omitted the receive.

## Evidence To Record

Record these fields in the issue comment or proof receipt:

- npm package and version
- GitHub release tag and whether the binary came from cache
- Pylon node pubkey
- Pylon payout destination
- triggered `training_run_id`
- triggered `window_id`
- triggered `assignment_id`
- contribution id
- accepted outcome id
- payout amount
- treasury payment id
- treasury status and reconciliation status
- worker wallet balance and wallet history observation
- any validator process used for proof
- any treasury snapshot or validator backlog warnings observed
- proof receipt path under `docs/reports/nexus/`

Do not record raw bearer tokens, wallet mnemonics, Spark API keys, or private
GCP credentials.

The first successful npm Pylon end-to-end proof for this path is recorded in
`docs/reports/nexus/20260422-035746-pylon-npm-e2e-fb60b91678ca.json`.

The first successful Autopilot-controlled production proof for this path is
recorded in
`docs/reports/nexus/2026-04-23-autopilot-pylon-production-earning-proof.md`.
That proof used the Autopilot-managed Pylon process on `main` at `96295609b`,
not the older `pylon-v0.1.9` release tag. It proved the worker was
online through Autopilot, received a bounded hosted homework run, completed and
sealed the work, passed validator closeout, received a confirmed and settled
25-sat accepted-work Treasury payout, and showed a worker wallet delta from
`0` to `25` sats. Public release assets for this exact source behavior must be
`pylon-v0.1.10` or newer; after publishing, rerun this runbook from a fresh
Pylon home before closing any release-dependent issue.

The first successful npm-installed `pylon-v0.1.10` release proof is recorded at
`docs/reports/nexus/20260423-050434-pylon-v0.1.10-release.json`. Treat that
receipt as the historical proof for the first npm release that settled
accepted-work sats into the worker wallet.

The current primary public-release proof is
`docs/reports/nexus/20260423-084113-pylon-v0.1.11-post-health-prod-e2e.json`.
It proves the published `pylon-v0.1.11` GitHub release asset and npm package,
the TUI-era minimum version floor, the Nexus online-version histogram, the
admin-triggered 25-sat CS336 homework run, treasury settlement, the worker
wallet balance increase, and the post-health-fix Nexus stats state
`payout_loop_health: idle` with no treasury degraded reason. The earlier
`docs/reports/nexus/20260423-080422-pylon-v0.1.11-prod-e2e.json` receipt
remains the first successful full `0.1.11` production earning proof.
