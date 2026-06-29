# Buy Mode Dispatcher Operator Walkthrough

Date: 2026-06-10

Issue: `OpenAgentsInc/openagents#4639`

Promise lane: `pylon.five_bitcoin_revenue_streams.v1`

Registry version during implementation: `2026-06-10.4`

## Scope

This ships the disabled-by-default Worker buy-mode dispatcher shell for
operator-approved NIP-90 job issuance and capped settlement. It does not turn
on live spend by default, does not enable any campaign without an operator
admin token, and does not move sats unless the operator starts a campaign with
spend enabled and the Worker has a configured settlement bridge.

The dispatcher uses `@openagentsinc/nip90`, which wraps the shared
`nostr-effect` NIP-90 helpers. Do not reimplement the NIP-90 protocol locally
in this Worker path.

## Routes

All routes are under `apps/openagents.com/workers/api` and require the same
admin-token gate used by scoped operator grants.

- `GET /api/operator/buy-mode`
  - Returns the latest campaign, disabled-default authority facts, and whether
    spend mutation is currently allowed.
- `POST /api/operator/buy-mode/start`
  - Requires `Idempotency-Key`.
  - Body: `perJobCapMsats`, `dailyCapMsats`, optional `relayUrl`, optional
    `campaignId`, optional `spendEnabled`.
  - `spendEnabled` defaults to `false`.
  - Rejects impossible cap policy, including per-job cap above daily cap.
- `POST /api/operator/buy-mode/stop`
  - Disables the latest campaign.
- `POST /api/operator/buy-mode/dispatch`
  - Requires `Idempotency-Key`.
  - Body: `amountMsats`, `content`, optional `campaignId`, optional `jobId`,
    optional `providerPubkeys`.
  - Issues a NIP-90 job request only while a campaign is enabled and the
    request fits both caps.
  - Stores only digest refs for prompt/result content.
- `POST /api/operator/buy-mode/results/settle`
  - Requires `Idempotency-Key`.
  - Body: `amountMsats`, `bolt11`, `content`, `providerPubkey`,
    `requestEventId`, `resultEventId`.
  - Pays only a known request, only once per result event, only for matching
    amount, only with nonempty result content, and only through the configured
    payment bridge.

## Cap And Halt Policy

The dispatcher enforces both caps before relay publication:

- `amountMsats > perJobCapMsats` creates
  `alert.buy_mode.per_job_cap_breach`, halts the campaign, and does not
  publish to the relay.
- `spentTodayMsats + amountMsats > dailyCapMsats` creates
  `alert.buy_mode.daily_cap_breach`, halts the campaign, and does not publish
  or settle.
- A returned result that asks for a different amount than the issued job
  creates `alert.buy_mode.result_amount_cap_breach` and halts before payment.

Consistency-critical D1 writes use one `db.batch([...])` for halt+alert,
dispatch write, and settlement write.

## Settlement Boundary

No sats move in the default deployed state.

There are two separate gates:

1. The operator must start a campaign with `spendEnabled: true`.
2. The Worker must have a real payment bridge dependency configured.

If an operator enables spend but no settlement bridge is configured, settlement
returns:

```json
{
  "result": {
    "kind": "blocked",
    "reasonRef": "blocker.buy_mode.payment_bridge_unconfigured"
  }
}
```

That is intentional. The route must not synthesize payment receipts.

## Public Projection Rules

Route responses and stored job records do not include raw prompts, raw result
content, raw BOLT11 invoices, secrets, tokens, wallet material, payment
preimages, or private keys. Prompt and result content are represented as
`digest.buy_mode.*` refs. BOLT11 invoices are represented as
`bolt11.buy_mode.redacted.*` refs after settlement.

## Verification

Focused route coverage:

```bash
cd apps/openagents.com/workers/api
bunx vitest run src/operator-buy-mode-routes.test.ts
```

The route tests cover:

- Disabled by default and admin-token gated.
- Campaign start with spend disabled.
- NIP-90 job issuance through an injected relay publisher while enabled.
- Cap breach halt and operator alert before relay publication.
- Settlement blocked until operator spend approval.
- Settlement blocked when the live payment bridge is unconfigured.
- Valid-result settlement and duplicate-settlement replay guard.
