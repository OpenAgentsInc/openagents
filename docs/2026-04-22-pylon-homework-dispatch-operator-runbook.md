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
treasury pays -> Pylon wallet history shows the receive
```

## Preconditions

Use this runbook only after the relevant local proof path or focused regression
test is green. Production Nexus is a confirmation surface, not the debugger for
ordinary scheduler and payout bugs.

Minimum runtime requirements:

- public Pylon `0.1.7` or newer
- production Nexus running the lease-priority fix that tries existing
  schedulable runs before auto-launching fresh hosted starter work
- production Nexus running the validator-priority fix that validates
  admin-dispatched homework before draining hosted starter backlog
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

HOME="/Users/christopherdavid" \
OPENAGENTS_PSIONIC_REPO="/Users/christopherdavid/work/psionic" \
npx --yes @openagentsinc/pylon@0.1.7 \
  --version 0.1.7 \
  --pylon-home "${PROOF_ROOT}/pylon-home" \
  --install-root "${PROOF_ROOT}/install" \
  --skip-diagnostics \
  2>&1 | tee "${PROOF_ROOT}/logs/npm-pylon.log"
```

The bootstrap should report a fresh prebuilt release asset, checksum
verification, `runtime ready` or `fully online`, and then:

```text
Starting Pylon default earning loop pylon
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

Bootstrap without launching:

```bash
PROOF_ROOT="$(cat /private/tmp/pylon-npm-e2e-latest-root)"
VAL_ROOT="${PROOF_ROOT}/validator"
mkdir -p "${VAL_ROOT}/logs"

HOME="/Users/christopherdavid" \
OPENAGENTS_PSIONIC_REPO="/Users/christopherdavid/work/psionic" \
npx --yes @openagentsinc/pylon@0.1.7 \
  --version 0.1.7 \
  --pylon-home "${VAL_ROOT}/pylon-home" \
  --install-root "${VAL_ROOT}/install" \
  --skip-diagnostics \
  --no-launch \
  2>&1 | tee "${VAL_ROOT}/logs/npm-validator-bootstrap.log"
```

Configure validator-only role claims:

```bash
VAL_BIN="${VAL_ROOT}/install/versions/pylon-v0.1.7-darwin-arm64/pylon"

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
RUN_PREFIX="codex-npm-e2e-$(date -u +%Y%m%d%H%M%S)"
printf '%s\n' "${RUN_PREFIX}" > "${PROOF_ROOT}/run-prefix.txt"

payload="$(jq -nc \
  --arg prefix "${RUN_PREFIX}" \
  '{
    run_count: 1,
    max_contributors_per_run: 1,
    amount_sats: 25,
    total_budget_sats: 25,
    run_slug_prefix: $prefix,
    reuse_existing_run: false
  }')"
printf '%s\n' "${payload}" > "${PROOF_ROOT}/dispatch-request.json"

http_status="$(curl -sS \
  -o "${PROOF_ROOT}/dispatch-response.json" \
  -w '%{http_code}' \
  -X POST "${NEXUS_BASE_URL}/v1/admin/homework/cs336-a1/dispatch" \
  -H "Authorization: Bearer ${NEXUS_CONTROL_ADMIN_BEARER_TOKEN}" \
  -H "Content-Type: application/json" \
  -d "${payload}")"
echo "HTTP_STATUS=${http_status}"
cat "${PROOF_ROOT}/dispatch-response.json" | jq .
```

A successful response has:

- `launched_run_count: 1`
- `failed_run_count: 0`
- `amount_sats` equal to the requested amount
- `launches[0].launch_phase: "leaseable"`
- `launches[0].assigned_pylons[0].node_pubkey_hex` equal to the npm Pylon worker
- `launches[0].assigned_pylons[0].release_id: "openagents.pylon@0.1.7"` or newer

Save the triggered run id:

```bash
jq -r '.launches[0].training_run_id' \
  "${PROOF_ROOT}/dispatch-response.json" > "${PROOF_ROOT}/triggered-run-id.txt"
```

## Verify The Triggered Run Is Actually Consumed

This is the critical check. The triggered run must move beyond merely
`assigned_contributors: 1`; it must receive contribution and accepted-work
state.

```bash
triggered_run_id="$(cat "${PROOF_ROOT}/triggered-run-id.txt")"
curl -fsS "${NEXUS_BASE_URL}/api/training/runs/${triggered_run_id}" |
  tee "${PROOF_ROOT}/triggered-run-status.json" |
  jq '{
    run: .run,
    featured_window: .featured_window,
    accepted_work: (.accepted_work // .accepted_outcomes // []),
    payouts: (.payouts // .training_payouts // [])
  }'
```

Also watch the local Pylon training status:

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

Then stop the proof Pylons so they do not keep generating more work.

Verify the worker wallet directly:

```bash
PROOF_ROOT="$(cat /private/tmp/pylon-npm-e2e-latest-root)"
PYLON_BIN="${PROOF_ROOT}/install/versions/pylon-v0.1.7-darwin-arm64/pylon"

HOME="/Users/christopherdavid" \
OPENAGENTS_PYLON_HOME="${PROOF_ROOT}/pylon-home" \
OPENAGENTS_PYLON_CONFIG_PATH="${PROOF_ROOT}/pylon-home/config.json" \
"${PYLON_BIN}" wallet balance --json

HOME="/Users/christopherdavid" \
OPENAGENTS_PYLON_HOME="${PROOF_ROOT}/pylon-home" \
OPENAGENTS_PYLON_CONFIG_PATH="${PROOF_ROOT}/pylon-home/config.json" \
"${PYLON_BIN}" wallet history --limit 20 --json
```

The proof is complete only when wallet history shows a completed receive whose
amount and payment id match a confirmed accepted-work treasury record for the
triggered run.

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
- worker wallet balance and wallet history receive
- any validator process used for proof
- any treasury snapshot or validator backlog warnings observed

Do not record raw bearer tokens, wallet mnemonics, Spark API keys, or private
GCP credentials.
