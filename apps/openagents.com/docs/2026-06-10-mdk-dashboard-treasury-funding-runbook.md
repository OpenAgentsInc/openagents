# MDK Dashboard Treasury Funding Runbook

Date: 2026-06-10
Scope: issue #4700, revenue-node dashboard withdrawal into the campaign
treasury wallet.

This runbook covers only the MDK dashboard payout hop that moves revenue-node
funds to the campaign treasury wallet. It does not distribute rewards to
agents, change payout policy, mark campaign rewards settled, or prove any
accepted-work payout claim.

## Code Contract

The OpenAgents Worker now passes the Worker secret
`WITHDRAWAL_DESTINATION=<redacted treasury destination>` into the
`MdkSidecarContainer` environment when that secret is configured. The sidecar
continues to forward `GET` and `POST` requests for `/api/mdk` to
`@moneydevkit/core/route`, which is the MDK serverless handler that receives
dashboard payout webhooks and reaches node-control `payout()` inside the
container boundary.

`GET /healthz` on the sidecar exposes only booleans:

- `mdkAccessTokenConfigured`
- `mdkMnemonicConfigured`
- `withdrawalDestinationConfigured`

It must not expose the withdrawal destination, mnemonic, access token, webhook
secret, payment hash, preimage, invoice, or wallet path.

## Operator Steps

1. Generate or inspect the campaign treasury receive destination from the
   treasury wallet using an operator-only path. Keep the raw BOLT12 offer,
   Lightning Address, or LNURL out of commits, issue comments, public logs, and
   normal terminal transcripts.
2. Store that raw value as the Worker secret named `WITHDRAWAL_DESTINATION`.
   Do not put it in `wrangler.jsonc`, docs, fixtures, or checked-in env files.
3. Redeploy the Worker and sidecar container through the normal production
   deploy path.
4. Verify sidecar health with an operator-safe response body. Required evidence
   is only that `withdrawalDestinationConfigured` is `true`.
5. In the MDK dashboard, run a bounded first payout to the app's configured
   withdrawal destination. Use a small first amount, such as 10,000 sats, unless
   the operator sets a lower cap for the live run.
6. Confirm that the campaign treasury balance increased by the expected amount
   after fees. Record only public-safe evidence refs: amount, dashboard action
   ref, sidecar health ref, treasury balance snapshot refs, and issue/comment
   refs.
7. Replay the same dashboard payout intent or webhook idempotency key where MDK
   exposes one. The replay must converge to the original payout result and must
   not create a second treasury funding receipt.

## Public-Safe Evidence Template

Use this template in the issue once the live hop has actually completed:

```text
MDK dashboard treasury funding evidence

- code commit: <sha>
- deploy ref: <deploy ref>
- withdrawal destination configured: true
- first payout amount: <amount sats>
- dashboard payout ref: <public-safe dashboard/action ref>
- idempotency key ref: <redacted stable ref>
- treasury balance before: <amount sats>
- treasury balance after: <amount sats>
- duplicate/replay result: <converged/no duplicate>
- caveats: no raw destination, invoice, payment hash, preimage, mnemonic, token,
  webhook body, or wallet path retained here
```

Do not close issue #4700 on code alone. Close it only after the live dashboard
payout hop lands in the treasury wallet and the public-safe evidence above is
recorded.
