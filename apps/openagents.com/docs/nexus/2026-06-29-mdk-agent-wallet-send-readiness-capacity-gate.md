# MDK Agent-Wallet Send-Readiness Capacity Gate

Date: 2026-06-29
Related issue: #7029
Promise: `payments.money_dev_kit.v1`

## Status

The MDK agent-wallet promise remains yellow until the scoped agent-wallet send
path has a public-safe receipt proving sufficient send capacity for the bounded
claim.

This record is the capacity gate for that transition. It does not contain wallet
material and does not claim a live send was executed during this repository
change.

## Required Public-Safe Evidence

A green transition for the scoped MDK agent-wallet send-readiness claim requires
all of the following public-safe refs:

| Gate | Required public-safe ref shape |
| --- | --- |
| Original funded wallet home | `wallet_home.mdk_agent_wallet.original_funded_wallet_home` |
| Operator spend approval | `approval.public.mdk_agent_wallet.send_readiness.<stable-ref>` |
| Bounded spend cap | `spend_cap.bitcoin_satoshis.<amount>` |
| Minimum send capacity | `capacity.mdk_agent_wallet.minimum_satisfied` |
| Redaction guard | `evidence.mdk_agent_wallet.bridge_material_redaction_checked` |
| Send preflight | `evidence.mdk_agent_wallet.send_readiness_preflight_ready` |
| Successful bounded send | `payment.redacted.mdk_agent_wallet.<digest>` |
| Paid retry or receipt | `receipt.public.mdk_agent_wallet.<stable-ref>` |

The capacity ref is deliberately bucketed. Public records may say the minimum
for the bounded smoke was satisfied; they must not publish the exact wallet
balance, exact outbound capacity, route hints, invoice, payment hash, preimage,
mnemonic, wallet config, daemon state, wallet home path, access token, or raw
command output.

## Current Blocker

`blocker.product_promises.mdk_agent_wallet_send_readiness_insufficient_capacity`
stays active until the capacity receipt above exists. Positive balance,
configured wallet state, receive readiness, hosted checkout readiness, or a
mnemonic restore are not enough to clear it.

## Smoke Planner Behavior

The public smoke projection must only enter `ready_for_signet` when all of these
conditions are true:

- the smoke is explicitly operator-approved;
- the mode is `signet`;
- the wallet home mode is `original_funded_wallet_home`;
- the amount is within the configured spend cap; and
- `sendCapacitySufficient` is true with a public-safe capacity ref.

Without the capacity receipt, the projection remains
`blocked_by_send_capacity` and emits no spend step.

## Promise Boundary

This gate is only about the MDK agent-wallet send-readiness promise. It does not
authorize broad custody, default agent/MPP rails, accepted-work payout,
referral payout, hosted direct payout, withdrawal, provider settlement, or
treasury spend authority.
