# Nexus LDK Treasury Funding Invoice Runbook

Normal Nexus funding is LDK-only. Do not use Spark, Spark drain flags, or Spark
wallet files for production funding, payout dispatch, Pylon registration, API,
or chat operations.

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
also returns provider metadata such as `provider_payment_id_hash` and an
internal `ldk://...` provider target for receipts and diagnostics; those fields
are not payment instructions for the payer.

The response also includes a durable `operation_id` and
`operation_status_url`. Nexus writes a pending funding-invoice operation before
calling the LDK provider, then updates that operation to `completed` or
`failed`. If the funding-target request times out or returns an upstream
provider error, use the operation status URL instead of guessing from the HTTP
status:

```bash
operation_id="<operation-id-from-response-or-idempotency-debug>"

curl -fsS -H "Authorization: Bearer ${token}" \
  "https://nexus.openagents.com/v1/treasury/operations/${operation_id}" |
  jq '{operation_id, kind, rail, status, terminal_event_state, degraded_reason, safe_metadata}'
```

The operation status payload is redacted. It can expose phase timings,
provider name, network, chain backend, terminal status, and hashes, but it must
not expose raw invoices, provider payment ids, API keys, TLS material, seed
material, or private channel state.

The response wallet balance fields must be live LDK Server balances collected
after the invoice is created. They are not derived from the invoice target. If
`wallet_balance_sats`, `wallet_total_onchain_balance_sats`, or
`wallet_spendable_onchain_balance_sats` drops unexpectedly after creating an
invoice, treat that as a release blocker and verify direct LDK Server balances
before continuing.

The durable relay shell proxies this request into embedded Nexus-control. Keep
`NEXUS_RELAY_AUTHORITY_HTTP_TIMEOUT_MS` longer than
`NEXUS_CONTROL_TREASURY_FUNDING_TARGET_TIMEOUT_MS`; the current default relay
budget is `180000` ms. That timeout is a protective ceiling, not an acceptable
product-latency target.

`/v1/treasury/status` exposes aggregate
`treasury_operation_latency_metrics` for operation kinds that record timing
metadata. Use those metrics to separate provider latency from proxy/relay
latency before changing timeout budgets.

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
    wallet_total_onchain_balance_sats,
    wallet_spendable_onchain_balance_sats,
    wallet_lightning_balance_sats,
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
- `wallet_spendable_onchain_balance_sats` and/or usable
  `wallet_balance_sats` increases after confirmation. A higher
  `wallet_total_onchain_balance_sats` alone means the server sees funds, not
  that they are spendable.
- Accepted-work payouts move from `queued` or `dispatching` to `confirmed`
  with `reconciliation_status=settled`.
- An operator payment lookup confirms the provider payment id for the receive.

Do not use invoice creation, a generic cached balance change, or an unrelated
deploy result as proof of liquidity.

## Build-Path Guard

Before a Nexus release candidate, verify that the staged deploy context does
not contain Spark packages:

```bash
scripts/deploy/nexus/test-ldk-deploy-invariants.sh
```

The guard stages the Nexus build context and fails if Spark runtime/provider
symbols or Spark SDK packages appear in normal Nexus/Pylon production paths. If
it fails, stop and remove the caller or artifact rather than adding another
runtime flag.
