# Pylon Homework Dispatch Operator Runbook

Published: 2026-04-22

This runbook is for operators who need to prove the public Pylon earning path
against hosted Nexus by running Pylon from npm, triggering a bounded homework
run from a separate admin process, and verifying accepted-work payout into the
Pylon wallet.

The target user story is:

```text
npx installs Pylon -> pylon stays online -> admin dispatch creates homework
work -> Pylon claims the work -> work closes out -> validation accepts it ->
treasury pays -> Pylon wallet balance increases and treasury records a settled
accepted-work payout
```

## Preconditions

Use this runbook only after the relevant local proof path or focused regression
test is green. Production Nexus is a confirmation surface, not the debugger for
ordinary scheduler and payout bugs.

Minimum runtime requirements:

- public Pylon release asset `pylon-v0.1.10` or newer. The npm bootstrap
  package may still be invoked as `npx @openagentsinc/pylon`; the important
  version for earning and validation is the resolved standalone Pylon binary.
- production Nexus running the lease-priority fix that tries existing
  schedulable runs before auto-launching fresh hosted starter work
- production Nexus running the validator-priority fix that validates
  admin-dispatched homework before draining hosted starter backlog
- production Nexus running the homework validation-policy fix that validates
  homework-dispatch windows with the aggregate challenge only
- production Nexus running the closeout fix that treats aggregate-only
  homework validation as defensible for payout
- a normal user `HOME` for the running Pylon process so Rust and Psionic
  discovery work
- an isolated `OPENAGENTS_PYLON_HOME` for the proof
- local Gemma runtime visible to Pylon
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
```

And run:

```bash
cargo test -p nexus-control validation_policy
```

The homework validation-policy test must show that `homework_dispatch` keeps the
aggregate validator challenge and skips per-contribution sample challenges. Use
Pylon `0.1.10` or newer for current npm proofs. `0.1.8` fixed the validator
replay case where a retained claim can point at stale same-host local target
bytes: Pylon falls back to the bridge-inline payload or rewrites the target
artifact id to match the materialized digest. `0.1.10` adds the
Autopilot-controlled earning proof fixes: default Spark payout destination
creation in the long-lived serve path, retained snapshot reuse for validator
replay retries, and stricter Autopilot paid-state projection. Do not re-enable
sample challenges for homework dispatch until the per-contribution sample
replay path is separately fixed and proven with npm Pylon.

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
  --version 0.1.10 \
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
PYLON_BIN="${PROOF_ROOT}/install/versions/pylon-v0.1.10-darwin-arm64/pylon"

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
  --version 0.1.10 \
  --pylon-home "${VAL_ROOT}/pylon-home" \
  --install-root "${VAL_ROOT}/install" \
  --skip-diagnostics \
  --no-launch \
  2>&1 | tee "${VAL_ROOT}/logs/npm-validator-bootstrap.log"
```

Configure validator-only role claims:

```bash
VAL_BIN="${VAL_ROOT}/install/versions/pylon-v0.1.10-darwin-arm64/pylon"
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

## Trigger Homework Work From A Separate Process

Use a simple slug prefix. Avoid relying on shell variables named `status` in
zsh because `status` is read-only.

```bash
PROOF_ROOT="$(cat /private/tmp/pylon-npm-e2e-latest-root)"
NETWORK_ID="$(cat "${PROOF_ROOT}/network-id.txt")"
RUN_PREFIX="codex-npm-e2e-$(date -u +%Y%m%d%H%M%S)"
printf '%s\n' "${RUN_PREFIX}" > "${PROOF_ROOT}/run-prefix.txt"

payload="$(jq -nc \
  --arg prefix "${RUN_PREFIX}" \
  --arg min_version "0.1.10" \
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

## Cron-Compatible Dispatch

The same endpoint is the operator pacing control for paid homework. Put the
dispatch call in cron or another scheduler, and control payout rate with
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
  --arg min_version "0.1.10" \
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
PYLON_BIN="${PROOF_ROOT}/install/versions/pylon-v0.1.10-darwin-arm64/pylon"

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
VAL_BIN="${VAL_ROOT}/install/versions/pylon-v0.1.10-darwin-arm64/pylon"

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
`run.cs336.a1.starter.*` challenges first, production Nexus is missing the
validator-priority fix. The fixed behavior is: validator claims prioritize
`run_kind: "homework_dispatch"` windows, then normal runs, then hosted starter
backlog.

If a `homework_dispatch` run has one aggregate challenge plus one
contribution-sample challenge, production Nexus is missing the homework
validation-policy fix. The fixed behavior is: homework-dispatch windows use the
aggregate challenge only until the released npm Pylon contribution-sample
replay path stops producing artifact-manifest digest drift.

If the validator log reports an `artifact_digest_mismatch` where the target
`contribution_artifact_manifest` artifact id digest differs from the materialized
target bytes, the validator is running an older Pylon release. Upgrade to
`pylon-v0.1.10` or newer and retry the run. Current releases repair stale
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
PYLON_BIN="${PROOF_ROOT}/install/versions/pylon-v0.1.10-darwin-arm64/pylon"

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
