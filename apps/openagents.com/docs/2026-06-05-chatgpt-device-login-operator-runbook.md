# ChatGPT Device Login Operator Runbook

Issue: `OPENAGENTS-P0-002`

This runbook is the command-line path for connecting multiple ChatGPT/Codex
provider accounts to an OpenAgents user before an overnight Adjutant/Sites run.

Live-provider steps are manual because the operator must complete each
ChatGPT device-code page in a browser. Route and CLI smoke tests use mocked
operator API responses and do not require real provider tokens.

## Requirements

From this Mac, use the existing ignored local secret file rather than asking
the operator to find or paste the admin token:

```bash
cd /Users/christopherdavid/work/openagents
set -a
. /Users/christopherdavid/work/.secrets/vortex-admin.env
set +a
export OPENAGENTS_BASE_URL="https://openagents.com"
```

The secret file provides `OPENAGENTS_ADMIN_API_TOKEN`. Do not print the token
value into terminal transcripts, docs, issue comments, or commit messages.

Future agents should source this file themselves when the user asks to connect
ChatGPT/Codex accounts from this workstation. Do not tell the user to export an
unknown token. If the file is missing or does not provide the token, inspect the
workspace root `.secrets/` runbooks before asking the user for help.

Do not paste or commit access tokens, refresh tokens, raw `auth.json`, device
auth secrets, grant secrets, or raw provider response bodies. The command prints
only verification URL, user code, expiry, attempt ID, public provider account
ref, label, and status.

## Live Agent-Assisted Connection Loop

Use this loop when an operator says they need to connect several accounts now.
The agent drives the CLI; the operator only completes each browser device-code
ceremony and replies `done`.

1. In `/Users/christopherdavid/work/openagents`, source
   `/Users/christopherdavid/work/.secrets/vortex-admin.env` and set
   `OPENAGENTS_BASE_URL=https://openagents.com`.
2. Run one `start` command with the next unique label, for example
   `account 1`, `account 2`, or `account 6`.
3. Give the operator only the verification URL, user code, expiry, attempt ID,
   and provider account ref.
4. Wait for the operator to say `done`.
5. Poll the attempt once. If it is still `pending`, wait briefly and poll again.
6. When it is `connected`, run a single-account sanity check using the printed
   provider account ref.
7. If sanity is healthy, start the next account. If sanity is not healthy,
   keep the account connected for audit, mark it as not ready in the handoff,
   and continue only if another account is available.
8. After the final account, run the fleet dashboard, all-account sanity, and
   parallel sanity probe.

The observed June 5, 2026 before-bed flow connected six ChatGPT/Codex accounts
for `chris@openagents.com` this way. The operator completed each page, replied
`done`, and the agent immediately polled, sanity checked, and started the next
labeled account. This is the preferred recovery pattern for future overnight
prep.

The agent should preserve four local facts for the active ceremony until that
account has passed sanity:

- account label;
- attempt ID;
- provider account ref;
- expiry timestamp.

When the operator says `done`, do not start a new account first. Poll the
stored attempt ID, confirm `Device login status: connected`, then run
`sanity <provider-account-ref>`. Only after that single-account sanity check
reports healthy should the agent start the next `--create-new` ceremony.

Never start more than one device-code ceremony at a time unless the operator
explicitly asks for parallel ceremonies. Sequential ceremonies avoid browser
session and user-code mixups.

### Interactive Reply Handling

During a live before-bed setup, treat short operator replies as commands to keep
the loop moving:

- `done`: poll the most recent attempt, confirm it reached `connected`, then
  sanity check the printed provider account ref.
- `done, next` or `done next`: complete the same poll and sanity check, then
  immediately start the next uniquely labeled account.
- `do another`, `one more`, or `I've got one more`: start exactly one more
  `--create-new` device-login attempt after the previous account has passed
  sanity.
- final `done` after the last planned account: poll and sanity check the final
  attempt, then run the fleet dashboard, all-account sanity, parallel sanity,
  selector explanation, active leases, and failover history checks.

The agent should keep the current attempt ID and provider account ref in its
working notes while the browser ceremony is pending. If context was compacted
or the attempt ID is lost, recover with the fleet dashboard and recent terminal
history before asking the operator to restart a ceremony.

If the CLI exits with `Missing OPENAGENTS_ADMIN_API_TOKEN`, the agent should
source `/Users/christopherdavid/work/.secrets/vortex-admin.env` itself. Do not
ask the operator where the token is, and do not print the token.

### Known Good Live Transcript Pattern

This is the intended command/reply cadence for future fleet setup:

```text
agent: start account N
agent: report verification URL, user code, expiry, attempt ID, provider account ref
operator: done
agent: poll attempt ID
agent: sanity provider account ref
agent: start account N+1 only after sanity passes
```

If the operator says `done, next`, treat it as the same cadence with an
automatic next start after sanity passes. If the operator says `I've got one
more`, finish the current poll and sanity check first, then start exactly one
additional labeled account. If they say final `done`, finish the current poll
and sanity check, then run the closeout commands in the next section.

Do not ask the operator to paste credentials, find `OPENAGENTS_ADMIN_API_TOKEN`,
or identify where the admin token lives. On this workstation, source the ignored
workspace secret file yourself and continue.

## After The Final `done`

When the operator finishes the last browser page and replies `done`, do not ask
them for another token or tell them to find an admin token. Source the ignored
workspace secret file yourself, poll the final attempt, run one sanity check for
that final provider account ref, then run the fleet-level checks:

```bash
cd /Users/christopherdavid/work/openagents
set -a
. /Users/christopherdavid/work/.secrets/vortex-admin.env
set +a
export OPENAGENTS_BASE_URL="https://openagents.com"

node scripts/provider-chatgpt-device-login.mjs poll <attempt-id>
node scripts/provider-chatgpt-device-login.mjs sanity <provider-account-ref>
node scripts/provider-chatgpt-device-login.mjs sanity --all --parallel 5 --email chris@openagents.com
node scripts/provider-chatgpt-device-login.mjs dashboard --email chris@openagents.com
node scripts/provider-chatgpt-device-login.mjs explain-lease --email chris@openagents.com
```

Expected overnight-ready result:

- every intended account is `connected/healthy`;
- the all-account sanity summary reports all accounts healthy;
- the parallel probe reports zero collisions;
- the dashboard summary reports every intended account eligible;
- `explain-lease` returns `Lease selector status: selected`.

On June 5, 2026, the final closeout for the before-bed fleet reported six of
six accounts healthy, zero collisions at parallelism five, six of six accounts
eligible, zero active leases, zero low-credit flags, zero reauth blockers, and a
selected lease candidate. Keep exact provider account refs and account labels
in operator-only terminal output or issue comments; they are not needed in this
tracked runbook.

### Replay The June 5 Before-Bed Flow

Future agents should treat this as the known-good command-line operating
procedure when the operator says they need to connect a ChatGPT/Codex fleet
before sleeping:

1. Source `/Users/christopherdavid/work/.secrets/vortex-admin.env` yourself and
   set `OPENAGENTS_BASE_URL=https://openagents.com`.
2. Start exactly one `--create-new` ceremony with the next unused label.
3. Show the operator the verification URL, user code, expiry, attempt ID, and
   public provider account ref. Do not show tokens or secret refs.
4. Wait for `done`.
5. Poll that exact attempt ID until it is `connected` or reaches a terminal
   failure.
6. Run `sanity <provider-account-ref>` for that exact account.
7. If sanity is healthy and the operator wants another account, start the next
   labeled ceremony. If sanity is not healthy, record the public status in the
   handoff and continue with another available account rather than blocking the
   whole fleet.
8. After the last `done`, run `sanity --all`, `sanity --all --parallel 5`,
   `dashboard`, `explain-lease`, `leases`, and `failover-history`.

If context compaction happens mid-flow, recover from the current terminal
transcript first: identify the most recent `attemptId`, public provider account
ref, and label that have not yet passed sanity. If the terminal transcript is
not enough, run the dashboard and inspect recent connection attempts before
asking the operator to restart a ceremony.

The final operator-facing handoff should include only counts and statuses:
number connected, number healthy, collision count, eligible count, active lease
count, low-credit count, reauth blocker count, selector status, and whether any
account needs operator action. Do not put raw provider refs, admin bearer token,
grant refs, secret refs, auth JSON, or provider responses in tracked docs.

## Start One Account

```bash
node scripts/provider-chatgpt-device-login.mjs start \
  --email chris@openagents.com \
  --label "account 1" \
  --create-new
```

The command prints:

- verification URL;
- user code;
- expiry;
- attempt ID;
- public provider account ref;
- next poll command.

Open the verification URL locally, enter the user code, and complete ChatGPT
sign-in for that account.

When the operator says the browser step is done, poll the attempt:

```bash
node scripts/provider-chatgpt-device-login.mjs poll <attempt-id>
```

If the poll returns `connected`, immediately start the next labeled account.
If it returns `pending`, wait briefly and poll again. If it returns `expired`,
`denied`, or `failed`, start a new attempt for that label and keep the failed
attempt ID only as an audit reference.

## Poll Until Connected

```bash
node scripts/provider-chatgpt-device-login.mjs poll <attempt-id>
```

Safe statuses:

- `pending`: the code has not been completed yet.
- `connected`: OpenAgents product surface stored a durable provider account row, health is healthy,
  and auth material is stored behind the server-side secret boundary.
- `expired`: start a fresh attempt.
- `denied`: the login was denied.
- `failed`: a typed redacted failure occurred.

Use `--json` on either command for machine-readable output.

## Sanity Check Accounts

After an account connects, run a sanity check before using it for overnight
work:

```bash
node scripts/provider-chatgpt-device-login.mjs sanity <provider-account-ref>
```

To check every connected account for the target user:

```bash
node scripts/provider-chatgpt-device-login.mjs sanity --all --email chris@openagents.com
```

To prove multiple accounts can be probed at the same time before an overnight
fleet run:

```bash
node scripts/provider-chatgpt-device-login.mjs sanity --all --parallel 5 --email chris@openagents.com
```

The sanity check issues and resolves a short-lived provider-account grant
through the server/runner boundary, validates that server-side auth material is
available, performs a minimal OAuth refresh probe equivalent to the runner's
Codex provider-auth path, records a durable sanity-check row, updates the
account's latest sanity timestamp/result, and records a provider account health
event. A successful refresh probe stores the rotated OAuth material back into
private KV before returning healthy. An invalidated, reused, expired, or
otherwise rejected refresh token is normalized to `token_invalidated`, marks the
account `requires_reauth`, and keeps it out of automatic leases until it is
reconnected.

When `--parallel` is greater than `1`, OpenAgents product surface also creates a redacted
simultaneous-probe receipt for each account with:

- probe run ID;
- per-account probe ID;
- per-account lease ID;
- public provider account ref;
- start and finish timestamp;
- terminal status;
- health classification;
- collision class.
- redacted failure class, when the probe identified one.

Safe collision classes:

- `none`
- `wrong_account_identity`
- `auth_material_overwrite`
- `grant_account_mismatch`
- `lease_isolation_failed`
- `hidden_global_lock_detected`

Safe classifications:

- `healthy`
- `requires_reauth`
- `low_credit`
- `rate_limited`
- `quota_exhausted`
- `provider_outage`
- `grant_resolution_failed`
- `launch_probe_failed`
- `unknown_failure`

Safe failure classes may include `token_invalidated`, `rate_limited`,
`provider_outage`, and other existing failover classes. They are redacted
classes only; raw provider response bodies are not safe output.

Sanity output is safe for issue comments. It must not include access tokens,
refresh tokens, raw auth JSON, grant secrets, provider raw responses, or secret
refs that grant retrieval authority.

## Acquire A Fleet Lease

Before launching customer work, inspect the fleet dashboard and acquire a
short-lived provider account lease instead of manually choosing a ChatGPT/Codex
account:

```bash
node scripts/provider-chatgpt-device-login.mjs dashboard --email chris@openagents.com
```

The dashboard shows each ChatGPT/Codex account's public account ref, operator
label, status, health, eligibility, eligibility blockers, operator priority,
active leases, lease limit, last sanity check, last simultaneous probe, last
selected timestamp, last successful/failed launch timestamp, recent failure
class, cooldown, low-credit flag, reauth/refill/operator notes, and safe
sanity/reconnect commands.

Dashboard output is an operator-only projection. It may include operator notes,
but it must not include provider tokens, auth JSON, secret refs, grant refs,
raw provider responses, or private runner payloads.

```bash
node scripts/provider-chatgpt-device-login.mjs explain-lease --email chris@openagents.com
```

```bash
node scripts/provider-chatgpt-device-login.mjs lease \
  --action customer_order_fulfillment \
  --assignmentId order_triage_ref_... \
  --email chris@openagents.com
```

The lease selector:

1. Expires stale active leases.
2. Excludes disconnected, denied, expired, unhealthy, reauth-required,
   stale reconnect marker, cooldown, low-credit, secretless, or deleted
   accounts.
3. Selects the connected healthy account with the fewest active leases.
4. Breaks ties by operator priority.
5. Falls back to oldest successful use / oldest connection and then public
   provider account ref for deterministic ordering.

Fleet metadata available to selection and failover includes operator label,
operator priority, lease limit, active leases, low-credit flag, cooldown until,
recent failure class, last sanity check, last simultaneous probe, last selected
time, last successful/failed launch time, reauth-required reason, operator
note, and refill note. Customer/public projections must continue to avoid
private operator notes, secret refs, auth JSON, grant refs, and raw provider
responses.

Safe lease output includes lease ref, public provider account ref, account
label, requested action, policy version, redacted selection reason, start time,
and expiry. It must not include provider tokens, auth JSON, secret refs, grant
refs, or raw provider responses.

Active leases can be inspected safely:

```bash
node scripts/provider-chatgpt-device-login.mjs leases --email chris@openagents.com
```

Long-running launches can touch a lease heartbeat:

```bash
node scripts/provider-chatgpt-device-login.mjs touch-lease \
  --leaseRef provider-account-lease_ref_... \
  --ttlSeconds 900
```

Completed or abandoned launches should release the lease:

```bash
node scripts/provider-chatgpt-device-login.mjs release-lease \
  --leaseRef provider-account-lease_ref_... \
  --status succeeded
```

## Fail Over From A Failed Lease

If a selected account fails launch or continuation, classify the failure and
ask OpenAgents product surface to retry the next eligible account:

```bash
node scripts/provider-chatgpt-device-login.mjs failover \
  --previousLeaseRef provider-account-lease_ref_... \
  --failureClass rate_limited \
  --action customer_order_fulfillment \
  --assignmentId order_triage_ref_... \
  --attemptNumber 1 \
  --maxAttempts 3 \
  --email chris@openagents.com
```

Supported failure classes:

- `token_invalidated`
- `low_credits`
- `rate_limited`
- `quota_exhausted`
- `provider_outage`
- `launch_timeout`
- `grant_resolution_failed`
- `runner_failure`
- `unknown_provider_failure`

Failover effects:

- `token_invalidated` marks the failed account `requires_reauth`.
- `low_credits` and `quota_exhausted` set the low-credit flag and a long
  cooldown.
- `rate_limited`, `provider_outage`, `launch_timeout`, and
  `unknown_provider_failure` set temporary cooldowns.
- `grant_resolution_failed` records a grant-path failure without printing
  grant refs, secret refs, or provider material.
- `runner_failure` records the failure but does not poison the account.

Failover writes a redacted receipt with previous public account ref, next
public account ref when available, failure class, account action, attempt
number, max attempts, outcome, cooldown/quarantine action, policy version,
operator summary, customer-safe summary, and customer-safe status. If attempts
are exhausted or no eligible account remains, the endpoint returns a blocked
outcome suitable for an operator-visible blocker.

Failover history can be inspected without exposing account secrets:

```bash
node scripts/provider-chatgpt-device-login.mjs failover-history \
  --email chris@openagents.com
```

Useful filters:

```bash
node scripts/provider-chatgpt-device-login.mjs failover-history \
  --assignmentId order_triage_ref_... \
  --email chris@openagents.com

node scripts/provider-chatgpt-device-login.mjs failover-history \
  --runId run_... \
  --email chris@openagents.com
```

History output is safe for issue comments when it is limited to receipt ID,
timestamps, public provider account refs, failure class, outcome, policy
version, and redacted summaries. It must not include provider tokens, auth
JSON, secret refs, grant refs, raw provider responses, or private operator
notes.

## Connect A Fleet Of Accounts

Repeat the start/poll ceremony with `--create-new` and a unique label. The
labels do not need to match the ChatGPT account email; they are operator-facing
fleet labels used in lease, sanity, and failover output.

```bash
node scripts/provider-chatgpt-device-login.mjs start --email chris@openagents.com --label "account 1" --create-new
node scripts/provider-chatgpt-device-login.mjs start --email chris@openagents.com --label "account 2" --create-new
node scripts/provider-chatgpt-device-login.mjs start --email chris@openagents.com --label "account 3" --create-new
node scripts/provider-chatgpt-device-login.mjs start --email chris@openagents.com --label "account 4" --create-new
node scripts/provider-chatgpt-device-login.mjs start --email chris@openagents.com --label "account 5" --create-new
```

Continue with `account 6`, `account 7`, and so on when more provider accounts
are available. Poll each attempt after completing the device-code page for that
account.

If the CLI reports `Missing OPENAGENTS_ADMIN_API_TOKEN`, do not ask the
operator to find a token. Source the ignored local operator secret file from the
workspace root:

```bash
set -a
. /Users/christopherdavid/work/.secrets/vortex-admin.env
set +a
```

Then rerun the same CLI command. The environment variable must stay local to
the shell and must not be printed.

The observed successful overnight-prep pattern was:

1. Start `account 1`.
2. Give the operator the verification URL, user code, and attempt ID.
3. Wait for "done".
4. Poll that attempt.
5. Confirm `Device login status: connected`, `Account status: connected`, and
   `Account health: healthy`.
6. Run `node scripts/provider-chatgpt-device-login.mjs sanity <provider-account-ref>`.
7. Start the next account.
8. Repeat until every intended fleet account is connected and sanity checked.

Do not launch multiple device-code ceremonies in parallel unless the operator
explicitly asks. Sequential ceremonies avoid mixing browser sessions and codes.

## Copy-Paste Five-Account Flow

Use this as the before-bed sequence. Replace attempt IDs with the values
printed by each `start` command.

```bash
node scripts/provider-chatgpt-device-login.mjs start --email chris@openagents.com --label "account 1" --create-new
node scripts/provider-chatgpt-device-login.mjs poll provider_attempt_...
node scripts/provider-chatgpt-device-login.mjs sanity provider-account_ref_...

node scripts/provider-chatgpt-device-login.mjs start --email chris@openagents.com --label "account 2" --create-new
node scripts/provider-chatgpt-device-login.mjs poll provider_attempt_...
node scripts/provider-chatgpt-device-login.mjs sanity provider-account_ref_...

node scripts/provider-chatgpt-device-login.mjs start --email chris@openagents.com --label "account 3" --create-new
node scripts/provider-chatgpt-device-login.mjs poll provider_attempt_...
node scripts/provider-chatgpt-device-login.mjs sanity provider-account_ref_...

node scripts/provider-chatgpt-device-login.mjs start --email chris@openagents.com --label "account 4" --create-new
node scripts/provider-chatgpt-device-login.mjs poll provider_attempt_...
node scripts/provider-chatgpt-device-login.mjs sanity provider-account_ref_...

node scripts/provider-chatgpt-device-login.mjs start --email chris@openagents.com --label "account 5" --create-new
node scripts/provider-chatgpt-device-login.mjs poll provider_attempt_...
node scripts/provider-chatgpt-device-login.mjs sanity provider-account_ref_...
```

List and validate the fleet after the fifth account:

```bash
node scripts/provider-chatgpt-device-login.mjs dashboard --email chris@openagents.com
node scripts/provider-chatgpt-device-login.mjs sanity --all --email chris@openagents.com
node scripts/provider-chatgpt-device-login.mjs sanity --all --parallel 5 --email chris@openagents.com
node scripts/provider-chatgpt-device-login.mjs explain-lease --email chris@openagents.com
node scripts/provider-chatgpt-device-login.mjs leases --email chris@openagents.com
node scripts/provider-chatgpt-device-login.mjs failover-history --email chris@openagents.com
```

The dashboard command is the connected-account list. It shows connected,
healthy, low-credit, rate-limited/cooldown, reauth-required, busy-at-lease-limit,
and stale states from one surface.

## Final Overnight Fleet Check

After the last account is connected, run the all-account sanity check and a
parallel probe:

```bash
node scripts/provider-chatgpt-device-login.mjs sanity --all --email chris@openagents.com
node scripts/provider-chatgpt-device-login.mjs sanity --all --parallel 5 --email chris@openagents.com
```

Expected result for a ready fleet:

- every connected account reports `healthy`;
- `collisionCount` is `0`;
- each parallel probe has collision class `none`;
- no account is marked low-credit, reauth-required, denied, or disconnected.

Then inspect the selector without consuming a lease:

```bash
node scripts/provider-chatgpt-device-login.mjs explain-lease --email chris@openagents.com
```

Expected selector result:

- `Eligible: yes`;
- the selected account has the lowest active lease count among eligible
  accounts;
- selection reason is redacted and does not expose secret refs, grant refs,
  auth JSON, tokens, or raw provider responses.

If a real overnight launch is about to begin, acquire a short lease per
assignment with a stable action and assignment ID:

```bash
node scripts/provider-chatgpt-device-login.mjs lease \
  --action customer_order_fulfillment \
  --assignmentId order_triage_ref_... \
  --email chris@openagents.com
```

If the launch succeeds, release the lease as `succeeded`. If the selected
account fails for account-specific reasons, call `failover` with the normalized
failure class so OpenAgents product surface can retry the next eligible account and record a
redacted receipt.

## Fake Smoke Test

Run this no-provider smoke before editing the CLI or relying on the runbook:

```bash
bunx vitest run scripts/provider-chatgpt-device-login.test.ts
```

The smoke starts a local mocked operator API and exercises:

- device-login start;
- poll pending, connected, and expired states;
- all-account sanity with `--parallel 5`;
- selector explanation;
- fleet dashboard;
- lease acquire, active lease list, and release;
- failover receipt creation;
- failover history; and
- redaction of token-like fields.

This test does not prove live ChatGPT auth. Live auth is proven only by the
manual device-code flow and the production sanity/parallel-probe commands.

## Go/No-Go Checklist

Go for overnight customer-order fulfillment only when all are true:

- `dashboard --email chris@openagents.com` shows at least five connected
  accounts and the intended minimum number eligible.
- No intended account is `requires_reauth`, `low_credit`, denied,
  disconnected, secretless, deleted, or in cooldown.
- `sanity --all --email chris@openagents.com` reports every intended account
  healthy.
- `sanity --all --parallel 5 --email chris@openagents.com` reports
  `collisionCount` 0 and collision class `none` for every account.
- `explain-lease --email chris@openagents.com` returns `selected` with a
  redacted reason.
- `leases --email chris@openagents.com` shows no stale active leases unrelated
  to current work.
- `failover-history --email chris@openagents.com` contains only redacted
  receipt fields.
- Safe-to-paste outputs include public provider account refs, public lease
  refs, receipt IDs, statuses, classifications, timestamps, policy versions,
  redacted selector/failover summaries, and issue/order/assignment/run IDs.
- Never paste provider tokens, refresh tokens, raw `auth.json`, secret refs,
  grant refs/secrets, raw provider responses, private runner payloads, or the
  admin API token.

No-go if any intended account requires reconnect/refill, if parallel probing
shows a collision, if no account is eligible for lease, or if the operator
cannot inspect the fleet dashboard.

## Operator API

Start:

```text
POST /api/operator/provider-accounts/chatgpt-codex/device-login/start
Authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN
```

Body:

```json
{
  "email": "chris@openagents.com",
  "accountLabel": "account 1",
  "createNew": true
}
```

Poll:

```text
GET /api/operator/provider-accounts/chatgpt-codex/device-login/{attemptId}
Authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN
```

Sanity check:

```text
POST /api/operator/provider-accounts/chatgpt-codex/sanity
Authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN
```

Single-account body:

```json
{
  "providerAccountRef": "provider-account_ref_..."
}
```

All connected accounts for a target user:

```json
{
  "email": "chris@openagents.com",
  "all": true
}
```

The operator API uses the same provider-account service as the browser settings
flow, so completed attempts create/update provider account rows, login events,
health, and server-side secret refs without exposing secret material in the
response.
