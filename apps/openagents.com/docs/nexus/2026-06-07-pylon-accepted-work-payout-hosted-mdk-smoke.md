# Pylon Accepted-Work MDK Payout Smoke

Date: 2026-06-07
Issue: #503

## Summary

OpenAgents product surface deployed the accepted-work payout route for Pylon assignments and ran a
production smoke against the accepted assignment produced by #502.

The route, policy gates, idempotency discipline, public-safe receipt projection,
and hosted MDK adapter are live. The hosted-MDK direct payout smoke did not
settle because MoneyDevKit rejected the hosted payout request with the
app-level programmatic payout toggle disabled.

This is a real production blocker, not a Cloudflare, D1, Tailnet, GCP, invoice,
or route-dispatch blocker. It blocks only the hosted programmatic payout lane,
not the already-approved `mdk_agent_wallet` settlement bridge.

The issue was unblocked by running a fresh local MDK agent-wallet payment for
the accepted #502 Pylon assignment, recording only public-safe payment and
settlement refs through the Pylon API, and then invoking the operator
settlement bridge. That created a public real-bitcoin receipt:

```text
receipt.nexus_pylon.settlement.assignment_public_issue502_20260608024927
```

Public API:

```text
https://openagents.com/api/public/nexus-pylon/receipts/receipt.nexus_pylon.settlement.assignment_public_issue502_20260608024927
```

Public page:

```text
https://openagents.com/nexus-pylon/receipts/receipt.nexus_pylon.settlement.assignment_public_issue502_20260608024927
```

## Live Route

Operator route:

```text
POST /api/operator/nexus-pylon/assignments/{assignmentRef}/accepted-work-payouts
```

The deployed route was verified through `openagents.com` after Worker version:

```text
7f0643e8-e0bd-408b-a470-f1b8e1f28804
```

Live public assets were also verified:

- `https://openagents.com/AGENTS.md`
- `https://openagents.com/.well-known/openagents.json`
- `https://openagents.com/api/openapi.json`

## Assignment Used

Accepted-work assignment:

```text
assignment.public.issue502.20260608024927
```

Pylon:

```text
pylon.issue502.local.20260608024927
```

D1 confirmed the assignment is in `accepted_work` state and has retained:

- accepted work refs;
- artifact refs;
- proof refs;
- closeout refs; and
- fresh wallet-readiness evidence for the current smoke window.

## Result

### Hosted MDK Direct Payout

The production route returned:

```json
{
  "error": "nexus_pylon_bridge_blocked",
  "reason": "adapter_unavailable: Hosted MDK dispatch failed: hosted_mdk_programmatic_payouts_disabled"
}
```

The MoneyDevKit source and package docs classify this condition as:

```text
PROGRAMMATIC_PAYOUTS_DISABLED
```

The MDK source describes it as a non-retryable app setting until the dashboard
toggle is enabled for the app whose `MDK_ACCESS_TOKEN` is bound to the Worker.
Issue #556 codifies this as the public-safe mode blocker
`blocker.mdk.hosted_programmatic_payouts_disabled`; hosted direct payout
claims stay disabled until the hosted app setting and funded key are both
verified.

### MDK Agent-Wallet Settlement Bridge

The local `mdk_agent_wallet` bridge smoke used isolated ignored wallet homes.
The payer send completed and the receiver wallet recorded inbound settlement.
The retained private files stay under `.secrets` and are not committed.
Current live accepted-work settlement claims are scoped to
`local_mdk_agent_wallet_bridge`, not hosted direct payout.

Only these public-safe refs were attached to the Pylon assignment:

```text
payment_proof.public.mdk_agent_wallet.issue503_3fdf88a715773db16f5ddf45
receipt.public.mdk_agent_wallet.issue503_3fdf88a715773db16f5ddf45
settlement.public.mdk_agent_wallet.issue503_3fdf88a715773db16f5ddf45
treasury_receipt.public.mdk_agent_wallet.issue503_3fdf88a715773db16f5ddf45
```

The Pylon assignment then recorded:

- `payment_receipt` with status `settled_real_bitcoin`;
- `settlement_status` with status `settled_real_bitcoin`; and
- operator settlement bridge receipt
  `receipt.nexus_pylon.settlement.assignment_public_issue502_20260608024927`.

The public receipt API verifies:

```json
{
  "receiptKind": "settlement_recorded",
  "realBitcoinMoved": true,
  "movementMode": "real_bitcoin",
  "assignmentRef": "assignment.public.issue502.20260608024927"
}
```

## What Passed

- The route exists in production.
- The route requires operator/admin authority.
- The route requires an `Idempotency-Key`.
- The route rejects missing or invalid private payout destinations for hosted
  MDK without storing raw destinations.
- The route requires `accepted_work` assignment state.
- The route requires retained accepted-work, artifact, proof, and closeout refs.
- The route requires fresh Pylon wallet-readiness evidence.
- The route enforces the spend cap before adapter dispatch.
- The route creates the payout authority intent before adapter dispatch.
- The hosted MDK adapter uses the Worker secret `MDK_ACCESS_TOKEN` and never
  imports native MDK Lightning runtime into the Cloudflare Worker.
- The hosted MDK adapter maps MDK payout failures into bounded public-safe
  reasons.
- Tests cover success, duplicate retry, stale wallet readiness, pause policy,
  insufficient liquidity, and raw destination redaction.

## What Did Not Pass

No hosted-MDK direct payout receipt was created because the hosted MDK app
rejected programmatic payout dispatch before money movement in that lane.

The accepted-work payout gate itself is complete because another approved
adapter, `mdk_agent_wallet`, settled accepted work and retained a public-safe
receipt without exposing raw wallet material. Hosted programmatic payouts still
need the dashboard toggle or a funded app key before that direct route can be
used for future payouts.

## Next Action

For #504, run repeated multi-Pylon and multi-host smokes against the live
Pylon API and settlement bridge. Keep hosted MDK direct payout marked as
configuration-blocked until programmatic payouts are enabled in the
MoneyDevKit dashboard for the deployed app or an app-scoped key with that
toggle and enough bitcoin liquidity is deployed.

## Redaction Boundary

This smoke intentionally does not record or publish:

- raw invoice;
- raw payment hash;
- preimage;
- wallet mnemonic;
- wallet config;
- exact wallet balance;
- raw payout destination;
- MDK access token;
- MDK mnemonic;
- webhook secret;
- raw command stdout or stderr; or
- private customer/operator data.
