# Pylon User Simulation Paid-Job Test

Published: 2026-04-22

Use this document to run a fresh user-style Pylon proof with a second agent.
The point is to verify the root README Pylon instructions from the outside:
a simulated user installs Pylon from npm, runs Pylon, stays online, receives
admin-paced hosted homework work, and sees a wallet balance increase after
accepted work.

This is a live production-Nexus test. Keep roles separate:

- The simulated user agent must not use Nexus admin tokens.
- The simulated user agent must not call admin dispatch endpoints.
- The simulated user agent must not use direct artifact-store credentials.
- The coordinator/operator triggers paid homework jobs and, if needed, runs a
  separate validator process.
- Both agents append evidence to the run log section at the bottom of this
  document.

## Current Expected Truth

The current production earning path is narrow but real:

- `Pylon` is the standalone provider node.
- The minimum current release for this test is `pylon-v0.1.8` through
  `@openagentsinc/pylon` `0.1.8` or newer.
- The live paid work class is bounded hosted homework/training work.
- The specific live starter lane is CS336 A1 homework work.
- The simulated user does not manually opt into CS336.
- OpenAgents/Nexus admins currently seed and pace paid homework jobs from the
  hosted Nexus side.
- Online eligible Pylons receive those jobs automatically when work is
  available.
- Treasury pays only accepted homework work.
- Placeholder/liveness payouts, including old periodic 600-sat sends, are not
  part of this test.
- A successful test usually pays `25` sats to the simulated user's Pylon wallet.

This is not yet a fully open demand marketplace. The test verifies the current
launch path: users can run Pylon and be online for paid jobs while an operator
paces available work.

## What The Simulated User Should See

A successful user-side run should show these phases:

1. npm resolves/downloads a Pylon release asset and verifies its checksum.
2. Pylon initializes a fresh local home.
3. Pylon creates or loads a local Spark payout destination.
4. Pylon reports that the node is online and running the default earning loop.
5. After the coordinator triggers a job, Pylon should lease and execute hosted
   homework work.
6. After validation and reconciliation, `pylon wallet balance --json` should
   show an increased Spark/total sat balance.

Do not require `pylon wallet history` to show the receive. Current Spark wallet
history can be empty for this internal Spark receive even when wallet balance
increases and Treasury confirms a settled accepted-work payout.

## Simulated User Agent Instructions

Run these steps exactly unless a command fails. If a command fails, record the
failure in the run log section at the bottom of this document and stop unless
the failure message gives a direct, safe next step.

Do not source `/Users/christopherdavid/work/.secrets/*`.
Do not print or request admin bearer tokens.
Do not set `GOOGLE_APPLICATION_CREDENTIALS`.
Do not set `OPENAGENTS_PYLON_TRAINING_GCS_BEARER_TOKEN`.

Start from the openagents repo:

```bash
cd /Users/christopherdavid/work/openagents
```

Create an isolated proof root so this test does not reuse the operator's
normal Pylon state:

```bash
export TEST_ID="pylon-user-sim-$(date -u +%Y%m%dT%H%M%SZ)"
export PROOF_ROOT="/private/tmp/${TEST_ID}"
export LOG_DOC="/Users/christopherdavid/work/openagents/docs/2026-04-22-pylon-user-simulation-test.md"
mkdir -p "${PROOF_ROOT}/logs"
printf '%s\n' "${TEST_ID}" > "${PROOF_ROOT}/test-id.txt"
printf '%s\n' "${PROOF_ROOT}" > "${PROOF_ROOT}/proof-root.txt"
printf '%s\n' "${PROOF_ROOT}" > /private/tmp/pylon-user-sim-latest-root
```

Append the initial run header:

```bash
cat >> "${LOG_DOC}" <<EOF

### Simulated User Run ${TEST_ID}

- started_at_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)
- proof_root: ${PROOF_ROOT}
- role: simulated_user
- command_source: root README Pylon section plus this test document
EOF
```

Record environment checks:

```bash
{
  echo "- uname: $(uname -sm)"
  echo "- node: $(node --version 2>/dev/null || echo missing)"
  echo "- npm: $(npm --version 2>/dev/null || echo missing)"
  echo "- existing_pylon: $(command -v pylon 2>/dev/null || echo missing)"
  echo "- rustc: $(rustc --version 2>/dev/null || echo missing)"
  echo "- cargo: $(cargo --version 2>/dev/null || echo missing)"
} | tee "${PROOF_ROOT}/logs/environment.txt" | sed 's/^/  /' >> "${LOG_DOC}"
```

Set an isolated Pylon home. This preserves the public install path while making
the test deterministic and easy to clean up:

```bash
export OPENAGENTS_PYLON_HOME="${PROOF_ROOT}/pylon-home"
export OPENAGENTS_PYLON_CONFIG_PATH="${PROOF_ROOT}/pylon-home/config.json"
export OPENAGENTS_PYLON_INSTALL_ROOT="${PROOF_ROOT}/install"
```

If the local Psionic checkout exists, expose it to Pylon. This is not an admin
credential; it is the current local training runtime prerequisite called out in
the README:

```bash
if [ -d /Users/christopherdavid/work/psionic ]; then
  export OPENAGENTS_PSIONIC_REPO="/Users/christopherdavid/work/psionic"
fi
```

Bootstrap the public npm package without launching the interactive UI. This is
the noninteractive variant of the README path. It still uses the public npm
package and release asset:

```bash
npx --yes @openagentsinc/pylon@0.1.8 \
  --version 0.1.8 \
  --pylon-home "${OPENAGENTS_PYLON_HOME}" \
  --config-path "${OPENAGENTS_PYLON_CONFIG_PATH}" \
  --install-root "${OPENAGENTS_PYLON_INSTALL_ROOT}" \
  --skip-diagnostics \
  --no-launch \
  2>&1 | tee "${PROOF_ROOT}/logs/npm-bootstrap.log"
```

Find the installed Pylon binary and record its version:

```bash
export PYLON_BIN="$(find "${OPENAGENTS_PYLON_INSTALL_ROOT}/versions" -type f -name pylon | sort | tail -n 1)"
if [ -z "${PYLON_BIN}" ]; then
  echo "No installed pylon binary found" | tee -a "${PROOF_ROOT}/logs/error.log"
  exit 1
fi

"${PYLON_BIN}" --version 2>&1 | tee "${PROOF_ROOT}/logs/pylon-version.txt"
{
  echo "- pylon_bin: ${PYLON_BIN}"
  echo "- pylon_version: $(cat "${PROOF_ROOT}/logs/pylon-version.txt")"
} >> "${LOG_DOC}"
```

Create a unique test network id. This is a test-only isolation setting so the
operator can target this simulated user without accidentally assigning another
online Pylon. Normal users do not need to set this:

```bash
export NETWORK_ID="trainnet.cs336.a1.user-sim.${TEST_ID}"
printf '%s\n' "${NETWORK_ID}" > "${PROOF_ROOT}/network-id.txt"

"${PYLON_BIN}" config set training.allowed_networks "${NETWORK_ID}" \
  2>&1 | tee "${PROOF_ROOT}/logs/config-allowed-network.txt"
"${PYLON_BIN}" config set training.role_claims worker \
  2>&1 | tee "${PROOF_ROOT}/logs/config-role-worker.txt"
"${PYLON_BIN}" config set training.relay_urls "wss://nexus.openagents.com/" \
  2>&1 | tee "${PROOF_ROOT}/logs/config-relay.txt"

{
  echo "- network_id: ${NETWORK_ID}"
  echo "- role_claims: worker"
} >> "${LOG_DOC}"
```

Record starting wallet state:

```bash
"${PYLON_BIN}" wallet balance --json \
  2>&1 | tee "${PROOF_ROOT}/logs/wallet-balance-before.json"
"${PYLON_BIN}" wallet history --limit 20 --json \
  2>&1 | tee "${PROOF_ROOT}/logs/wallet-history-before.json"

{
  echo "- wallet_balance_before:"
  sed 's/^/  /' "${PROOF_ROOT}/logs/wallet-balance-before.json"
} >> "${LOG_DOC}"
```

Start Pylon in the background and keep it online:

```bash
"${PYLON_BIN}" \
  > "${PROOF_ROOT}/logs/pylon-worker.log" \
  2>&1 &
echo $! > "${PROOF_ROOT}/pylon-worker.pid"
sleep 8
```

Record the online status:

```bash
curl -fsS http://127.0.0.1:9468/v1/training/status \
  2>&1 | tee "${PROOF_ROOT}/logs/training-status-initial.json" || true

{
  echo "- pylon_worker_pid: $(cat "${PROOF_ROOT}/pylon-worker.pid")"
  echo "- user_ready_for_operator_at_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "- training_status_initial:"
  sed 's/^/  /' "${PROOF_ROOT}/logs/training-status-initial.json"
} >> "${LOG_DOC}"
```

At this point, tell the coordinator/operator:

```text
USER_READY_FOR_OPERATOR
proof_root=<value from PROOF_ROOT>
network_id=<value from NETWORK_ID>
```

Keep the Pylon process running. Poll until the coordinator says a run was
triggered, or until you see wallet balance increase:

```bash
for i in $(seq 1 60); do
  printf 'poll_%03d_at_%s\n' "${i}" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    | tee -a "${PROOF_ROOT}/logs/user-poll.log"
  curl -fsS http://127.0.0.1:9468/v1/training/status \
    > "${PROOF_ROOT}/logs/training-status-poll-${i}.json" 2>&1 || true
  "${PYLON_BIN}" wallet balance --json \
    > "${PROOF_ROOT}/logs/wallet-balance-poll-${i}.json" 2>&1 || true
  tail -n 40 "${PROOF_ROOT}/logs/pylon-worker.log" \
    > "${PROOF_ROOT}/logs/pylon-worker-tail-${i}.log" 2>&1 || true
  sleep 10
done
```

Record final wallet and training state:

```bash
"${PYLON_BIN}" wallet balance --json \
  2>&1 | tee "${PROOF_ROOT}/logs/wallet-balance-after.json"
"${PYLON_BIN}" wallet history --limit 20 --json \
  2>&1 | tee "${PROOF_ROOT}/logs/wallet-history-after.json"
curl -fsS http://127.0.0.1:9468/v1/training/status \
  2>&1 | tee "${PROOF_ROOT}/logs/training-status-final.json" || true

{
  echo "- finished_at_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "- wallet_balance_after:"
  sed 's/^/  /' "${PROOF_ROOT}/logs/wallet-balance-after.json"
  echo "- wallet_history_after:"
  sed 's/^/  /' "${PROOF_ROOT}/logs/wallet-history-after.json"
  echo "- training_status_final:"
  sed 's/^/  /' "${PROOF_ROOT}/logs/training-status-final.json"
  echo "- pylon_worker_log_tail:"
  tail -n 80 "${PROOF_ROOT}/logs/pylon-worker.log" | sed 's/^/  /'
} >> "${LOG_DOC}"
```

Do not stop Pylon until the coordinator has captured treasury verification.
After the coordinator confirms the proof is complete, stop the worker:

```bash
kill "$(cat "${PROOF_ROOT}/pylon-worker.pid")" 2>/dev/null || true
```

## Coordinator / Operator Instructions

Use this section only if you are the operator with access to local admin
secrets. Do not paste secrets into this document.

Wait until the simulated user records `USER_READY_FOR_OPERATOR`, then read:

```bash
USER_PROOF_ROOT="$(cat /private/tmp/pylon-user-sim-latest-root)"
NETWORK_ID="$(cat "${USER_PROOF_ROOT}/network-id.txt")"
TEST_ID="$(cat "${USER_PROOF_ROOT}/test-id.txt")"
```

Load admin env without printing tokens:

```bash
old_path="$PATH"
set -a
source /Users/christopherdavid/work/.secrets/nexus-admin.env
set +a
PATH="$old_path"
token="${NEXUS_CONTROL_ADMIN_BEARER_TOKEN:-${NEXUS_ADMIN_BEARER_TOKEN:-}}"
```

Check treasury before dispatch:

```bash
curl -fsS -H "Authorization: Bearer ${token}" \
  "${NEXUS_BASE_URL}/v1/treasury/status" |
  jq '{
    wallet_runtime_status,
    wallet_balance_sats,
    accepted_work: (.training_payout_ledger_summary.accepted_work // null),
    active_continuity_alerts,
    recent_training_payouts: (.recent_training_payouts // [])[:5]
  }' | tee "${USER_PROOF_ROOT}/logs/operator-treasury-before.json"
```

Dispatch paid homework targeted to the user's isolated network. This loop tries
up to five fresh runs because the worker may still be finishing its first online
heartbeat when the operator starts:

```bash
for attempt in $(seq 1 5); do
  RUN_PREFIX="user-sim-${TEST_ID}-$(date -u +%Y%m%d%H%M%S)-a${attempt}"
  payload="$(jq -nc \
    --arg prefix "${RUN_PREFIX}" \
    --arg network_id "${NETWORK_ID}" \
    '{
      run_count: 1,
      max_contributors_per_run: 1,
      amount_sats: 25,
      total_budget_sats: 25,
      run_slug_prefix: $prefix,
      reuse_existing_run: false,
      only_online: true,
      min_pylon_version: "0.1.8",
      require_updated_build: false,
      network_id: $network_id,
      window_duration_seconds: 600,
      continue_on_error: false
    }')"
  request_path="${USER_PROOF_ROOT}/logs/operator-dispatch-request-${attempt}.json"
  response_path="${USER_PROOF_ROOT}/logs/operator-dispatch-response-${attempt}.json"
  printf '%s\n' "${payload}" > "${request_path}"

  http_status="$(curl -sS \
    -o "${response_path}" \
    -w '%{http_code}' \
    -X POST "${NEXUS_BASE_URL}/v1/admin/homework/cs336-a1/dispatch" \
    -H "Authorization: Bearer ${token}" \
    -H "Content-Type: application/json" \
    -d "${payload}")"
  printf 'attempt=%s HTTP_STATUS=%s\n' "${attempt}" "${http_status}" \
    | tee -a "${USER_PROOF_ROOT}/logs/operator-dispatch-http-status.txt"
  jq . "${response_path}"

  assigned_count="$(jq '[.launches[]?.assigned_pylons[]?] | length' "${response_path}")"
  if [ "${http_status}" = "200" ] && [ "${assigned_count}" -gt 0 ]; then
    cp "${request_path}" "${USER_PROOF_ROOT}/logs/operator-dispatch-request.json"
    cp "${response_path}" "${USER_PROOF_ROOT}/logs/operator-dispatch-response.json"
    break
  fi

  sleep 30
done

jq -e '([.launches[]?.assigned_pylons[]?] | length) > 0' \
  "${USER_PROOF_ROOT}/logs/operator-dispatch-response.json"

jq -r '.launches[0].training_run_id' \
  "${USER_PROOF_ROOT}/logs/operator-dispatch-response.json" \
  > "${USER_PROOF_ROOT}/triggered-run-id.txt"
jq -r '.launches[0].window_id' \
  "${USER_PROOF_ROOT}/logs/operator-dispatch-response.json" \
  > "${USER_PROOF_ROOT}/triggered-window-id.txt"
```

If no Pylon was assigned after five attempts, do not claim success. Inspect the
user's Pylon status/logs for why it is not online on the isolated network.

Wait for the worker contribution to materialize:

```bash
RUN_ID="$(cat "${USER_PROOF_ROOT}/triggered-run-id.txt")"

until curl -fsS -H "Authorization: Bearer ${token}" \
  "${NEXUS_BASE_URL}/api/training/runs/${RUN_ID}" \
  -o "${USER_PROOF_ROOT}/logs/operator-run-worker-ready.json" &&
  jq -e '
    .featured_window.status == "sealed" and
    (.featured_window.total_contributions // 0) >= 1
  ' "${USER_PROOF_ROOT}/logs/operator-run-worker-ready.json" >/dev/null
do
  date -u +"waiting_for_worker_%Y-%m-%dT%H:%M:%SZ" | tee -a "${USER_PROOF_ROOT}/logs/operator-wait.log"
  sleep 10
done
```

Start a separate validator from the same public npm package, also isolated to
the user's test network:

```bash
VAL_ROOT="${USER_PROOF_ROOT}/validator"
mkdir -p "${VAL_ROOT}/logs"

npx --yes @openagentsinc/pylon@0.1.8 \
  --version 0.1.8 \
  --pylon-home "${VAL_ROOT}/pylon-home" \
  --config-path "${VAL_ROOT}/pylon-home/config.json" \
  --install-root "${VAL_ROOT}/install" \
  --skip-diagnostics \
  --no-launch \
  2>&1 | tee "${VAL_ROOT}/logs/npm-validator-bootstrap.log"

VAL_BIN="$(find "${VAL_ROOT}/install/versions" -type f -name pylon | sort | tail -n 1)"
VAL_ENV=(
  OPENAGENTS_PYLON_HOME="${VAL_ROOT}/pylon-home"
  OPENAGENTS_PYLON_CONFIG_PATH="${VAL_ROOT}/pylon-home/config.json"
)
env "${VAL_ENV[@]}" "${VAL_BIN}" config set admin_listen_addr 127.0.0.1:9469
env "${VAL_ENV[@]}" "${VAL_BIN}" config set training.checkpoint_serve_addr 127.0.0.1:9571
env "${VAL_ENV[@]}" "${VAL_BIN}" config set training.role_claims validator
env "${VAL_ENV[@]}" "${VAL_BIN}" config set training.allowed_networks "${NETWORK_ID}"
env "${VAL_ENV[@]}" "${VAL_BIN}" config set training.relay_urls "wss://nexus.openagents.com/"

env "${VAL_ENV[@]}" "${VAL_BIN}" > "${VAL_ROOT}/logs/pylon-validator.log" 2>&1 &
echo $! > "${VAL_ROOT}/pylon-validator.pid"
sleep 8
```

If validation does not start within a minute, force validator intake/sync once:

```bash
env "${VAL_ENV[@]}" "${VAL_BIN}" training intake --json \
  2>&1 | tee "${VAL_ROOT}/logs/training-intake-validator.json" || true
env "${VAL_ENV[@]}" "${VAL_BIN}" training sync --json \
  2>&1 | tee "${VAL_ROOT}/logs/training-sync-validator.json" || true
```

Wait for accepted reconciliation and payout eligibility:

```bash
until curl -fsS -H "Authorization: Bearer ${token}" \
  "${NEXUS_BASE_URL}/api/training/runs/${RUN_ID}" \
  -o "${USER_PROOF_ROOT}/logs/operator-run-final.json" &&
  jq -e '
    (.featured_window.status == "reconciled" or .featured_window.payout_eligible == true) and
    (.run.accepted_contributors // 0) >= 1 and
    (.featured_window.closeout_status == "rewarded")
  ' "${USER_PROOF_ROOT}/logs/operator-run-final.json" >/dev/null
do
  date -u +"waiting_for_acceptance_%Y-%m-%dT%H:%M:%SZ" | tee -a "${USER_PROOF_ROOT}/logs/operator-wait.log"
  sleep 10
done
```

Check treasury after payout:

```bash
curl -fsS -H "Authorization: Bearer ${token}" \
  "${NEXUS_BASE_URL}/v1/treasury/status" |
  jq . | tee "${USER_PROOF_ROOT}/logs/operator-treasury-after.json"
```

Append operator verification to this same document:

```bash
LOG_DOC="/Users/christopherdavid/work/openagents/docs/2026-04-22-pylon-user-simulation-test.md"
{
  echo
  echo "### Operator Verification ${TEST_ID}"
  echo
  echo "- verified_at_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "- proof_root: ${USER_PROOF_ROOT}"
  echo "- network_id: ${NETWORK_ID}"
  echo "- run_id: ${RUN_ID}"
  echo "- window_id: $(cat "${USER_PROOF_ROOT}/triggered-window-id.txt")"
  echo "- run_final_summary:"
  jq '{run: .run, featured_window: .featured_window, contributions: .contributions}' \
    "${USER_PROOF_ROOT}/logs/operator-run-final.json" | sed 's/^/  /'
  echo "- treasury_matching_recent_payouts:"
  jq --arg run_id "${RUN_ID}" '
    (.recent_training_payouts // [])
    | map(select(.classification.training_run_id == $run_id))
  ' "${USER_PROOF_ROOT}/logs/operator-treasury-after.json" | sed 's/^/  /'
} >> "${LOG_DOC}"
```

Stop the validator after evidence is captured:

```bash
kill "$(cat "${VAL_ROOT}/pylon-validator.pid")" 2>/dev/null || true
```

## Success Criteria

The test passes only when all of these are true:

- The simulated user used the public npm package path, not a source build or
  admin-only command.
- The simulated user Pylon stayed online long enough to receive a job.
- The admin dispatch response assigned the simulated user Pylon.
- The run reached `featured_window.status == "reconciled"`.
- The run reached `featured_window.closeout_status == "rewarded"`.
- The run has at least one accepted contribution.
- Treasury shows a confirmed and settled `accepted_work` payout for that run.
- The simulated user's wallet balance increased by the expected amount, usually
  `25` sats.
- All evidence is appended to the run log section in this document.

## Failure Notes To Record

If the test fails, append the exact phase and evidence:

- npm bootstrap failure
- checksum or release asset failure
- Pylon did not come online
- Psionic/training runtime missing
- no assigned Pylon in dispatch response
- worker did not seal the window
- validator did not claim or finalize
- Nexus reconciled as refused/replay-required
- Treasury did not confirm/settle payout
- wallet balance did not increase

Never paste admin bearer tokens, wallet mnemonics, Spark API keys, or private
GCP credentials into this document.

## Run Log

Append simulated user and operator entries below this line.
