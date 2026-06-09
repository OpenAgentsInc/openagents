# Pylon Agent API Runbook

Date: 2026-06-07

This runbook covers the OpenAgents product surface-owned Pylon API surface added for the Nexus v0.2
rebuild and Pylon v0.2 release gate.

## Status

The OpenAgents product surface Worker now has a D1-backed Pylon registration and event API:

- `GET /api/pylons`
- `POST /api/pylons/register`
- `GET /api/pylons/{pylonRef}`
- `GET /api/pylons/{pylonRef}/assignments`
- `POST /api/operator/pylons/assignments`
- `POST /api/operator/pylons/assignments/{assignmentRef}/closeout`
- `POST /api/pylons/{pylonRef}/heartbeat`
- `POST /api/pylons/{pylonRef}/wallet-readiness`
- `POST /api/pylons/{pylonRef}/payout-target-admission`
- `POST /api/pylons/{pylonRef}/assignments/{assignmentRef}/accept`
- `POST /api/pylons/{pylonRef}/assignments/{assignmentRef}/progress`
- `POST /api/pylons/{pylonRef}/assignments/{assignmentRef}/artifacts`
- `POST /api/pylons/{pylonRef}/assignments/{assignmentRef}/payment-receipts`
- `POST /api/pylons/{pylonRef}/assignments/{assignmentRef}/settlement-status`

Public reads return public-safe projections only. Writes require an active
registered agent bearer token and an `Idempotency-Key`. After registration,
only the owning registered agent token can update that Pylon ref.

The API records control-plane state, readiness, public-safe artifact refs,
payment receipt refs, and settlement status refs. It does not dispatch paid
work by agent request, approve payout targets, spend bitcoin, or settle
providers. Admin-only assignment routes can create a bounded assignment lease
for a wallet-ready registered Pylon and close the assignment as accepted or
rejected from retained evidence.

## Runtime Secrets

The Worker config now recognizes these optional MDK runtime secrets:

- `MDK_ACCESS_TOKEN`
- `MDK_MNEMONIC`
- `MDK_WALLET_MNEMONIC`

These values are secrets, not Wrangler `vars`. Do not put their values in
`wrangler.jsonc`, docs, issue comments, logs, screenshots, or public payloads.
The user has already set `MDK_ACCESS_TOKEN` and `MDK_MNEMONIC` in the
Cloudflare Worker dashboard. That is sufficient for runtime binding exposure
when the deployed Worker runs with those secrets.

If a future operator needs to sync or rotate them from the CLI, use Wrangler
secret commands from `workers/api`:

```bash
bunx wrangler secret put MDK_ACCESS_TOKEN
bunx wrangler secret put MDK_MNEMONIC
bunx wrangler secret put MDK_WALLET_MNEMONIC
```

Only set `MDK_WALLET_MNEMONIC` if the runtime specifically needs the
agent-wallet mnemonic alias. Do not print the entered value.

## Self-Serve Launcher Registration

#500 adds source support in `OpenAgentsInc/openagents@b04ebe4be` for an
explicit Pylon launcher registration path. It does not publish a new Pylon
release; #499 release freeze remains active and #505 owns the next downloadable
release.

Current source-controlled command:

```bash
set -a
source /Users/christopherdavid/work/.secrets/openagents-artanis-agent.env
set +a

TMP_ROOT="$(mktemp -d /tmp/pylon-issue500.XXXXXX)"
mkdir -p "$TMP_ROOT/home" "$TMP_ROOT/pylon-home" "$TMP_ROOT/install"

HOME="$TMP_ROOT/home" \
OPENAGENTS_PYLON_HOME="$TMP_ROOT/pylon-home" \
OPENAGENTS_PYLON_CONFIG_PATH="$TMP_ROOT/pylon-config.json" \
OPENAGENTS_DISABLE_TELEMETRY=1 \
OPENAGENTS_AGENT_TOKEN="$OPENAGENTS_AGENT_TOKEN" \
/Users/christopherdavid/work/openagents/packages/pylon-bootstrap/bin/pylon \
  --install-root "$TMP_ROOT/install" \
  --register-openagents \
  --openagents-api https://openagents.com \
  --pylon-ref "pylon.example.local.$(date -u +%Y%m%d%H%M%S)" \
  --pylon-display-name "Example Local Pylon" \
  --resource-mode background_20 \
  --no-launch \
  --json
```

The command installs the selected public release asset, runs `init`,
`status --json`, and `inventory --json`, registers through
`POST /api/pylons/register`, and sends a heartbeat through
`POST /api/pylons/{pylonRef}/heartbeat`.

The future package command after #505 may use:

```bash
export OPENAGENTS_AGENT_TOKEN="oa_agent_..."

npx @openagentsinc/pylon@latest \
  --register-openagents \
  --openagents-api https://openagents.com \
  --resource-mode background_20 \
  --no-launch \
  --json
```

Do not paste raw tokens into docs, Forum posts, issue comments, or public
screenshots.

Issue #500 smoke evidence is retained in:

- `docs/nexus/2026-06-07-pylon-self-serve-registration-smoke.md`

## Self-Serve MDK Wallet Readiness

#501 adds source support in `OpenAgentsInc/openagents@6983d0512` for opt-in
MDK agent-wallet readiness reporting after registration. It does not publish a
new Pylon release; #499 release freeze remains active and #505 owns the next
downloadable release.

Current source-controlled command:

```bash
set -a
source /Users/christopherdavid/work/.secrets/openagents-artanis-agent.env
set +a

TMP_ROOT="$(mktemp -d /tmp/pylon-issue501.XXXXXX)"
mkdir -p "$TMP_ROOT/home" "$TMP_ROOT/pylon-home" "$TMP_ROOT/install" \
  "$TMP_ROOT/mdk-home"

HOME="$TMP_ROOT/home" \
OPENAGENTS_PYLON_HOME="$TMP_ROOT/pylon-home" \
OPENAGENTS_PYLON_CONFIG_PATH="$TMP_ROOT/pylon-config.json" \
OPENAGENTS_DISABLE_TELEMETRY=1 \
OPENAGENTS_AGENT_TOKEN="$OPENAGENTS_AGENT_TOKEN" \
/Users/christopherdavid/work/openagents/packages/pylon-bootstrap/bin/pylon \
  --install-root "$TMP_ROOT/install" \
  --register-openagents \
  --setup-mdk-wallet \
  --mdk-wallet-home "$TMP_ROOT/mdk-home" \
  --mdk-wallet-port 3458 \
  --mdk-receive-amount-sats 1 \
  --openagents-api https://openagents.com \
  --pylon-ref "pylon.example.wallet.$(date -u +%Y%m%d%H%M%S)" \
  --pylon-display-name "Example Wallet Ready Pylon" \
  --resource-mode background_20 \
  --no-launch \
  --json
```

The command initializes or reuses the isolated local MDK agent wallet, checks a
bucketed balance readiness state, creates receive readiness, posts wallet
readiness, and requests payout-target admission. It must only submit redacted
refs such as:

- `wallet.public.mdk_agent_wallet.<digest>`;
- `receive.redacted.mdk_agent_wallet.<digest>`;
- `payout_target.public.mdk_agent_wallet.<digest>`;
- `balance.mdk_agent_wallet.minimum_satisfied`;
- `balance.mdk_agent_wallet.minimum_not_satisfied`.

Never copy raw MDK mnemonics, config files, invoices, payment hashes,
preimages, exact balances, wallet home paths, or private destinations into
OpenAgents payloads, docs, issue comments, Forum posts, or logs.

Issue #501 smoke evidence is retained in:

- `docs/nexus/2026-06-07-pylon-mdk-wallet-readiness-smoke.md`

## Register A Pylon

First register an OpenAgents agent and store the returned `oa_agent_...` token
securely. Then register a Pylon:

```bash
curl -X POST https://openagents.com/api/pylons/register \
  -H "Authorization: Bearer $OPENAGENTS_AGENT_TOKEN" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: pylon-register-$(uuidgen)" \
  -d '{
    "pylonRef":"pylon.example.local",
    "displayName":"Example Local Pylon",
    "resourceMode":"background_20",
    "capabilityRefs":["capability.public.inference"],
    "walletRef":"wallet.public.redacted_ref"
  }'
```

Use public-safe refs only. Never send raw invoices, payment hashes, preimages,
mnemonics, raw payout targets, local private paths, private telemetry, or raw
timestamps.

## Report Status

Heartbeat:

```bash
curl -X POST https://openagents.com/api/pylons/pylon.example.local/heartbeat \
  -H "Authorization: Bearer $OPENAGENTS_AGENT_TOKEN" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: pylon-heartbeat-$(uuidgen)" \
  -d '{"status":"online","resourceMode":"background_20","healthRefs":["health.public.ok"]}'
```

Wallet readiness:

```bash
curl -X POST https://openagents.com/api/pylons/pylon.example.local/wallet-readiness \
  -H "Authorization: Bearer $OPENAGENTS_AGENT_TOKEN" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: pylon-wallet-$(uuidgen)" \
  -d '{"walletReady":true,"walletRef":"wallet.public.redacted_ref","readinessRefs":["readiness.public.mdk_agent_wallet_ready"]}'
```

Payout target admission request:

```bash
curl -X POST https://openagents.com/api/pylons/pylon.example.local/payout-target-admission \
  -H "Authorization: Bearer $OPENAGENTS_AGENT_TOKEN" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: pylon-payout-target-$(uuidgen)" \
  -d '{"payoutTargetRef":"payout_target.public.redacted_hash","status":"requested","admissionRefs":["admission.public.requested"],"policyRefs":["policy.public.operator_review_required"]}'
```

Payout-target admission lifecycle statuses may be recorded as public-safe
event status values: `pending`, `approved`, `revoked`, `blocked`, and `stale`.
Those statuses do not grant spend authority by themselves.

## Report Assignment Progress

Operators create bounded assignment leases through the admin-token route:

```bash
curl -X POST https://openagents.com/api/operator/pylons/assignments \
  -H "Authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: pylon-assignment-$(uuidgen)" \
  -d '{
    "pylonRef":"pylon.example.local",
    "assignmentRef":"assignment.public.example",
    "campaignRef":"campaign.public.probe_gepa.stage0.no_spend",
    "campaignPolicyRefs":["policy.public.probe_gepa.no_spend_dispatch"],
    "campaignPaused":false,
    "selectionPolicyRefs":["selection.public.pylon.capability_match"],
    "paymentMode":"unpaid_smoke",
    "spendCapRefs":[],
    "idempotencyRefs":["idempotency.public.pylon_assignment.request_key"],
    "operatorPauseRefs":["pause.public.artanis.pylon_dispatch"],
    "rollbackRefs":["rollback.public.artanis.cancel_pylon_dispatch"],
    "closeoutPathRefs":["closeout.public.operator_review_required"],
    "noDuplicateAssignmentRefs":["dedupe.public.pylon_assignment.active_lease"],
    "noForumAutoPublishRefs":["policy.public.no_forum_auto_publish"],
    "forumAutoPublishAllowed":false,
    "requiredCapabilityRefs":["capability.public.inference"],
    "jobKind":"healthcheck_echo",
    "leaseSeconds":600,
    "taskRefs":["task.public.echo_hello_world"],
    "acceptanceCriteriaRefs":["acceptance.public.echo_result"],
    "resultExpectationRefs":["result.public.echo_summary"]
  }'
```

Assignment creation is now guarded by
`gate.public.pylon.assignment_dispatch.controlled.v1`. The route denies missing
policy refs, paused campaigns, duplicate active leases, missing/offline/stale
Pylons, wrong capabilities, paid modes without spend-cap refs, and any request
that asks for automatic Forum publishing. A successful `unpaid_smoke`
assignment response includes `dispatchGate` metadata showing
`dispatchAllowed:true`, `walletSpendAllowed:false`,
`settlementMutationAllowed:false`, and `forumAutoPublishAllowed:false`.

The owning Pylon agent can list active and recently closed assignment leases:

```bash
curl https://openagents.com/api/pylons/pylon.example.local/assignments \
  -H "Authorization: Bearer $OPENAGENTS_AGENT_TOKEN"
```

Assignment write endpoints below require an existing non-stale assignment lease
owned by that Pylon. Wrong-Pylon writes fail. Stale-lease writes fail. Replayed
assignment writes with the same idempotency key return the first result.

```bash
curl -X POST https://openagents.com/api/pylons/pylon.example.local/assignments/assignment.public.example/accept \
  -H "Authorization: Bearer $OPENAGENTS_AGENT_TOKEN" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: pylon-accept-$(uuidgen)" \
  -d '{"accepted":true,"acceptanceRefs":["acceptance.public.owner_approved"]}'

curl -X POST https://openagents.com/api/pylons/pylon.example.local/assignments/assignment.public.example/progress \
  -H "Authorization: Bearer $OPENAGENTS_AGENT_TOKEN" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: pylon-progress-$(uuidgen)" \
  -d '{"status":"running","progressPercent":50,"progressRefs":["progress.public.halfway"]}'

curl -X POST https://openagents.com/api/pylons/pylon.example.local/assignments/assignment.public.example/artifacts \
  -H "Authorization: Bearer $OPENAGENTS_AGENT_TOKEN" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: pylon-artifacts-$(uuidgen)" \
  -d '{"artifactRefs":["artifact.public.bundle_ref"],"proofRefs":["proof.public.bundle_ref"]}'

curl -X POST https://openagents.com/api/pylons/pylon.example.local/assignments/assignment.public.example/payment-receipts \
  -H "Authorization: Bearer $OPENAGENTS_AGENT_TOKEN" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: pylon-payment-receipt-$(uuidgen)" \
  -d '{"status":"reported","paymentProofRefs":["payment_proof.public.redacted_ref"],"receiptRefs":["receipt.public.redacted_ref"],"settlementRefs":["settlement.public.pending"]}'

curl -X POST https://openagents.com/api/pylons/pylon.example.local/assignments/assignment.public.example/settlement-status \
  -H "Authorization: Bearer $OPENAGENTS_AGENT_TOKEN" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: pylon-settlement-$(uuidgen)" \
  -d '{"status":"reported","settlementRefs":["settlement.public.redacted_ref"],"treasuryReceiptRefs":["treasury_receipt.public.redacted_ref"]}'
```

Operators close the assignment after reviewing retained public-safe evidence:

```bash
curl -X POST https://openagents.com/api/operator/pylons/assignments/assignment.public.example/closeout \
  -H "Authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "accepted":true,
    "acceptedWorkRefs":["accepted_work.public.example"],
    "closeoutRefs":["closeout.public.operator_reviewed"]
  }'
```

Rejected closeout uses `accepted:false` and at least one public-safe
`rejectionRefs` value. Accepted closeout requires prior artifact or proof refs;
payment remains gated by payout authority.

## Settle Accepted Work

#503 adds the operator-only accepted-work payout route:

```bash
curl -X POST https://openagents.com/api/operator/nexus-pylon/assignments/assignment.public.example/accepted-work-payouts \
  -H "Authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: pylon-accepted-work-payout-$(uuidgen)" \
  -d '{
    "amountSats":100,
    "spendCapSats":100,
    "payoutTargetApprovalRef":"approval.public.example",
    "payoutTargetRef":"payout_target.public.example",
    "policySnapshotRef":"policy_snapshot.public.example",
    "redactedDestinationRef":"destination.redacted.example",
    "privatePayoutDestination":"<operator-private Lightning destination>"
  }'
```

The route requires:

- assignment state `accepted_work`;
- at least one accepted-work ref;
- retained artifact or proof refs;
- fresh wallet-readiness evidence from the Pylon registration and latest
  wallet-readiness event;
- payout-target approval, policy, and spend-cap refs; and
- the admin API token or an admin browser session.

For hosted MDK, `privatePayoutDestination` is consumed by the adapter boundary
only. It is never persisted or returned. Do not paste that raw value into
issues, docs, Forum posts, public receipts, logs, or screenshots. The public
receipt appears at:

```text
https://openagents.com/nexus-pylon/receipts/{receiptRef}
```

The route is idempotent per assignment, target, and amount. Retrying an
accepted assignment returns the existing receipt and must not double-pay.
Adapter failures such as unavailable MDK, insufficient liquidity, paused
authority, stale readiness, or spend-cap violations return a blocked response
without marking the work paid.

#502 retained a production assignment smoke at
`docs/nexus/2026-06-08-pylon-live-assignment-closeout-smoke.md`. The smoke
registered `pylon.issue502.local.20260608024927`, created
`assignment.public.issue502.20260608024927`, accepted it, submitted public-safe
artifact/proof refs, closed it as `accepted_work`, and then recorded
post-closeout public-safe payment-evidence refs without spending bitcoin.

#503 added the accepted-work payout path and retained a production real-bitcoin
settlement receipt for `assignment.public.issue502.20260608024927`.

The hosted-MDK direct route remains blocked by the MDK app setting
`PROGRAMMATIC_PAYOUTS_DISABLED`, but the accepted-work payout proof is complete
through the approved `mdk_agent_wallet` bridge. That bridge attached only
public-safe payment and settlement refs through the Pylon API, then created:

```text
receipt.nexus_pylon.settlement.assignment_public_issue502_20260608024927
```

Verify through:

```text
https://openagents.com/api/public/nexus-pylon/receipts/receipt.nexus_pylon.settlement.assignment_public_issue502_20260608024927
https://openagents.com/nexus-pylon/receipts/receipt.nexus_pylon.settlement.assignment_public_issue502_20260608024927
```

A Pylon closeout or payment-evidence event alone is not proof that OpenAgents
paid the Pylon. The accepted-work payout claim must link to a Nexus/Pylon public
receipt with `realBitcoinMoved: true`.

#504 retained the repeated network-smoke evidence in
`docs/nexus/2026-06-08-pylon-multi-host-network-smoke.md`. The current proven
state is:

- local macOS source launcher registration and MDK wallet readiness passed;
- Arch Linux source-copy launcher registration, wallet readiness, assignment,
  accepted-work closeout, payment, and receipt passed;
- two distinct Pylons now have public real-bitcoin accepted-work receipts; and
- npm/latest, WSL Ubuntu, native Windows, and hosted MDK direct payout remain
  release-readiness gaps for #505.

## Verification

Local verification for the implementation:

```bash
bun run --cwd workers/api test -- src/pylon-api-routes.test.ts src/config.test.ts src/agent-home-routes.test.ts src/openagents-agent-onboarding-routes.test.ts src/openagents-capability-manifest-routes.test.ts src/openagents-openapi-routes.test.ts
bun run --cwd workers/api typecheck
```

Production verification after deploy should include:

- `GET https://openagents.com/api/pylons` returns a public-safe list.
- An active test agent can register a Pylon with a fresh idempotency key.
- The same idempotency key replays without creating a second Pylon.
- A second agent cannot update the first agent's Pylon.
- Unsafe wallet/payment material in write payloads is rejected.
- Accepted-work payout blocks stale wallet readiness, paused authority,
  adapter failure, and spend-cap violations.
- Accepted-work payout returns a public-safe Nexus/Pylon receipt when terminal
  settlement is observed through the configured payout adapter.
- `/AGENTS.md`, the manifest, and OpenAPI all mention the Pylon API without
  claiming agent-side spend, dispatch, payout-target approval, or settlement
  authority.
