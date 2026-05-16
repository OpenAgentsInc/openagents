# Nexus LDK Treasury Funding Invoice Runbook

Normal Nexus funding is LDK-only. Do not use Spark, Spark final-drain flags,
or Spark wallet files for production funding, payout dispatch, Pylon
registration, API, or chat operations.

This runbook creates a Lightning invoice through the active Nexus treasury
provider boundary. The deployable Nexus image excludes the Spark crate and
does not compile or stage Spark SDK dependencies.

## Preconditions

- Work from the `openagents` repo.
- Read `/Users/christopherdavid/work/.secrets/nexus-admin.env` for the admin
  bearer token. Do not print the token.
- Use the hosted funding-target endpoint; do not inspect or mutate treasury
  wallet files to create an invoice.
- Treat generated Bolt11 invoices as live payment requests. Do not commit
  invoices or paste them into issue comments after use.

## Create Funding Material

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

Use the `bolt11_invoice` field as the payable invoice. The active provider
also returns provider metadata such as `provider_payment_id` and an internal
`ldk://...` provider target for receipts and diagnostics; those fields are not
payment instructions for the payer.

The durable relay shell proxies this request into embedded Nexus-control. Keep
`NEXUS_RELAY_AUTHORITY_HTTP_TIMEOUT_MS` longer than
`NEXUS_CONTROL_TREASURY_FUNDING_TARGET_TIMEOUT_MS`; the current default relay
budget is `180000` ms. That timeout is a protective ceiling, not an acceptable
product-latency target.

## Confirm Payment

Invoice creation is not payment. After the payer reports payment, verify with
treasury status:

```bash
curl -fsS -H "Authorization: Bearer ${token}" \
  https://nexus.openagents.com/v1/treasury/status | jq '{
    active_treasury_provider,
    active_treasury_rail,
    ldk_network,
    ldk_chain_backend,
    ldk_server_configured,
    wallet_balance_sats,
    wallet_runtime_status,
    wallet_balance_updated_at_unix_ms,
    accepted_pending: .training_payout_ledger_summary.accepted_work_pending_payout_count,
    accepted_attention: .training_payout_ledger_summary.accepted_work_attention_payout_count,
    payouts_dispatched_24h,
    payouts_confirmed_24h,
    last_dispatch_at_unix_ms,
    last_confirmed_payout_at_unix_ms,
    active_continuity_alerts
  }'
```

Acceptable funding proof is one of:

- The LDK provider reports a settled receive or updated spendable balance.
- Accepted-work payouts move from `queued` or `dispatching` to `confirmed`
  with `reconciliation_status=settled`.
- An operator payment lookup confirms the provider payment id for the receive.

Do not use invoice creation, a generic cached balance change, or an unrelated
deploy result as proof of liquidity.

## Build-Path Guard

Before a Nexus release candidate, verify that the staged deploy context does
not contain Spark packages:

```bash
tmp_context="$(mktemp -d /tmp/openagents-nexus-build-context.XXXXXX)"
scripts/deploy/nexus/stage-build-context.sh "$tmp_context" >/dev/null
rg -n 'openagents-spark|breez-sdk-spark|spark-wallet|name = "spark"|breez/spark-sdk' "$tmp_context" -S
```

The search should return no rows. The root workspace may still exclude a
non-deployed `crates/spark` directory while old non-Nexus packages exist, but
that directory must not be copied into the staged Nexus build plan or lockfile.
If package rows appear, stop and remove the caller or artifact rather than
adding another runtime flag.
