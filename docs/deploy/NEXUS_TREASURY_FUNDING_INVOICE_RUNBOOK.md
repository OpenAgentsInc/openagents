# Nexus Treasury Funding Invoice Runbook

2026-05-15 transition note: this runbook describes the legacy Spark-backed
funding path. New treasury work should prioritize the LDK transition documented
in
[`../2026-05-15-ldk-nexus-treasury-transition-audit.md`](../2026-05-15-ldk-nexus-treasury-transition-audit.md).
Use this Spark path only when operating existing production compatibility
surfaces or explicitly testing the legacy fallback.

Use this when Nexus needs more spendable sats for accepted-work payouts.
This runbook is for hosted production treasury funding material only. It is
not a Pylon user wallet invoice flow and it is not payout proof by itself.

## Preconditions

- Work from the `openagents` repo.
- Read the workspace `.secrets/nexus-admin.env` file for the admin bearer
  token. Do not print the token.
- Use the hosted funding-target endpoint; do not inspect or mutate treasury
  wallet files to create an invoice.
- Treat the generated Bolt11 invoice as a live payment request. It is safe to
  hand to the payer, but do not commit it, paste it into issue comments after
  use, or treat it as a secret-bearing receipt.

## Create The Funding Material

Preserve `PATH` before sourcing the secret file because the secret file is not
a complete shell profile:

```bash
old_path="$PATH"
set -a
source /Users/christopherdavid/work/.secrets/nexus-admin.env
set +a
PATH="$old_path"

token="${NEXUS_ADMIN_BEARER_TOKEN:-${NEXUS_CONTROL_ADMIN_BEARER_TOKEN:-}}"

curl -fsS -X POST https://nexus.openagents.com/v1/treasury/funding-target \
  -H "Authorization: Bearer ${token}" \
  -H "Content-Type: application/json" \
  -d '{
    "amount_sats": 50000,
    "description": "OpenAgents Nexus treasury funding",
    "expiry_seconds": 3600
  }' | jq .
```

The preferred response field to give a Spark-capable payer is `spark_invoice`
or `spark_address`. Hosted Nexus pays Pylons to Spark addresses, so Spark
funding is the direct payout-liquidity path. The `bolt11_invoice` field remains
available for normal Lightning payers, but a paid Bolt11 invoice is not by
itself proof that the wallet now has Spark leaves available for Spark-address
payouts.
Hosted Nexus should return the Spark invoice even if compatibility Bolt11
invoice creation fails.

The durable relay shell proxies this request into embedded Nexus-control. Keep
`NEXUS_RELAY_AUTHORITY_HTTP_TIMEOUT_MS` longer than
`NEXUS_CONTROL_TREASURY_FUNDING_TARGET_TIMEOUT_MS`; the default relay budget is
`180000` ms. A shorter relay budget can turn a real Nexus-control funding
timeout into an unhelpful relay `502`.

If public traffic is temporarily routed through the
`nexus-http-recovery-proxy` service, keep that proxy's upstream timeout in the
same budget class. The repo-owned public watchdog installer now defaults
`NEXUS_HTTP_RECOVERY_PROXY_UPSTREAM_TIMEOUT_SECONDS` to `180` seconds. Restoring
the older 12 second proxy timeout will make public funding-target requests fail
with generic recovery-proxy `502` responses while VM-local relay calls can still
succeed.

Do not treat the 180 second timeout as acceptable product latency. It only
prevents the public proxy from masking the real wallet result. Historical
Nexus receipts already show Spark wallet sync, funding-target, and
leaf-selection classes that can exceed normal chat/API budgets, including
20s, 180s, and 600s timeout classes. The long-term fix is an async
funding-target operation with an idempotency key, phase-level wallet timing,
and explicit degraded reasons such as `spark_wallet_sync_slow` or
`spark_leaf_selection_blocked`. Keep raw invoices out of logs either way.

If the payer needs a different amount, change only `amount_sats`,
`description`, and `expiry_seconds`. Keep `amount_sats` positive. A request
without a positive amount may return receive addresses without amount-specific
invoices.

## Confirm Payment

Invoice creation is not payment. A `504` from the endpoint is also not payment
or non-payment; it usually means the bounded wallet operation timed out during
restart, sync, or wallet load.

After the payer says it is paid, verify with treasury status:

```bash
curl -fsS -H "Authorization: Bearer ${token}" \
  https://nexus.openagents.com/v1/treasury/status | jq '{
    wallet_balance_sats,
    wallet_runtime_status,
    wallet_balance_updated_at_unix_ms,
    accepted_pending: .training_payout_ledger_summary.accepted_work_pending_payout_count,
    accepted_attention: .training_payout_ledger_summary.accepted_work_attention_payout_count,
    payouts_dispatched_24h,
    payouts_confirmed_24h,
    last_dispatch_at_unix_ms,
    last_confirmed_payout_at_unix_ms,
    active_continuity_alerts,
    recent_training_payouts: [.recent_training_payouts[]? | {
      status,
      reconciliation_status,
      amount_sats,
      payment_id,
      classification
    }] | .[0:8]
  }'
```

Acceptable payout-liquidity proof is one of:

- Accepted-work payouts move from `queued` or `dispatching` to `confirmed` and
  `reconciliation_status=settled`.
- A later wallet/payment scan shows the receive in wallet history.

Do not use invoice creation, a generic cached balance change, or a Lightning
receive record by itself as proof that Spark-address payouts can drain. Do not
redeploy or roll back images to solve an underfunded-wallet error; fund the
wallet with Spark-spendable liquidity, verify status, then rerun the same
deploy gate if the image was otherwise correct.

## Closeout Notes

When funding was used during the issue 4368 / 4413 closeout, the invoice
payment cleared the insufficient-funds blocker, but the deploy still failed
until the treasury state hot-write loop was fixed. Future agents should keep
these concerns separate:

- Funding solves wallet insufficiency.
- Deploy gates prove the image is healthy.
- Public Pylon proof proves a user can run `pylon`, complete hosted training
  work, and receive completed payments in the Pylon wallet.
