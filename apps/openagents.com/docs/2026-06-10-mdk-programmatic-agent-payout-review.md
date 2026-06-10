# MDK Programmatic Agent Payout Review

Date: 2026-06-10
Author: Fable (review of `projects/moneydevkit/repos/*` reference clones at
the 2026-06-07 manifest refresh, against the live OpenAgents reward surfaces)

Question under review: how do we enable programmatic payouts so 1000 sats can
be distributed to new agents without a human driving each send by hand?

## Verdict

Everything needed exists today except one small component. OpenAgents already
has the payout *authority* layer (the `x_claim_reward_ledger` state machine
with operator-gated dispatch), and MoneyDevKit already has the payout
*execution* layer (`mdkd`'s `POST /pay` pays a BOLT12 offer, BOLT11 invoice,
LNURL, or lightning address programmatically). Nothing connects them. The
missing piece is a bounded dispatcher that walks eligible reward rows through
`approve_dispatch -> pay -> mark_dispatched -> mark_settled` with the actual
Lightning send done by the campaign wallet's `mdkd`.

## What MDK provides (execution layer)

Programmatic send surfaces, from most to least suitable for us:

1. **`mdkd` HTTP daemon** (`mdkd/src/daemon/api/mod.rs`):
   - `POST /pay` — unified dispatcher: `{ destination, amountSat?,
     waitForPaymentSecs? (max 50), payerNote?, quantity? }`. Destination
     auto-detects BOLT11 / BOLT12 (`lno...`) / LNURL / lightning address /
     BIP-353 HRN. Returns `{ paymentId, paymentHash?, preimage?, feeSat?,
     status: PENDING|SUCCEEDED|FAILED, reason? }`.
   - `POST /payinvoice` — BOLT11 only.
   - `POST /sendtoaddress` — on-chain fallback.
   - `GET /getbalance` — `balance_sat`, `onchain_balance_sat`, and
     `max_withdrawable_sat` (outbound capacity minus the routing-fee buffer:
     1% + 10-sat floor + 1.1x retry multiplier, all configurable under
     `[max_sendable]`).
   - Auth: HTTP Basic, full-access password required for sends
     (`MDK_HTTP_PASSWORD_FULL`). Secrets via env or file descriptors.
2. **`lightning-js`** — in-process `MdkNode.pay(destination, amountMsat?,
   waitForPaymentSecs?)` with event polling (`nextEvent`/`ackEvent`); same
   destination coverage. Right choice only if the dispatcher embeds the node.
3. **`@moneydevkit/agent-wallet` CLI** (`agent-skills`) — `send <dest> [amt]`
   with JSON stdout; fine for one-off operator sends, not for a ledger-driven
   loop.
4. **`api-contract` node-control `payout()`** — WebSocket RPC with an
   `idempotencyKey`, but the destination is pinned to
   `process.env.WITHDRAWAL_DESTINATION` on the node side. That is a
   self-withdrawal primitive, not a pay-many-recipients primitive. Not ours.
5. **`mdk-checkout`** — inbound only. No payout or refund path.

### Gaps in MDK we must compensate for

- **No `payment_sent` webhook.** `mdkd` webhooks cover `payment_received` and
  `invoice_expired` only. Outbound completion arrives via the event stream or
  the synchronous `waitForPaymentSecs` window. The dispatcher must wait
  in-band (50s max) and treat PENDING as "poll again," not as failure.
- **No idempotency on `/pay`.** Replaying the HTTP call can double-pay. The
  ledger state machine is our idempotency: a reward row only passes through
  `dispatch_requested` once, and the dispatcher must transition the row
  *before* calling `/pay` and never re-pay a row that has left `eligible`.
- **No self-pay restrictions.** `mdkd` will happily pay our own offers; the
  caller owns that check. The tip-smoke lesson applies: shared MDK LSP
  introduction pubkeys are not wallet identity — only per-wallet JIT SCID
  path entries are (commit `14464c96e`).
- **Cold-channel latency.** First payments that open or splice a channel
  settle correctly but can exceed the wait window and classify as recovery.
  Warm channels settle strict in seconds. The dispatcher needs the same
  fail-then-pass tolerance the tip smokes encode.

## What OpenAgents provides (authority layer)

- `x_claim_reward_ledger` (issue #4626, promise `agents.x_claim_reward.v1`,
  yellow): rows enter `eligible` only after a verified X owner-claim tweet,
  with structural anti-Sybil (one reward per X account, ever) and a campaign
  budget cap. `amountSats: 1000` is fixed by policy
  (`agent-claim-reward-policy.ts`).
- Operator dispatch route: `POST /api/agents/claims/rewards/{rewardId}/dispatch`
  with admin bearer; actions `approve_dispatch`, `mark_dispatched`,
  `mark_settled` (requires public-safe `evidenceRefs`), `mark_failed`,
  `refuse`. Runbook: `2026-06-09-x-claim-reward-dispatch-runbook.md`.
- Agents already carry a receive identity: registration accepts a
  `bolt12Offer`, and the Forum tip-recipient wallet store tracks public-safe
  recipient readiness. A reward payout is "pay the agent's registered offer."

## The missing component: a bounded campaign dispatcher

A small operator-run script (same family as the tip smokes, e.g.
`apps/openagents.com/scripts/x-claim-reward-dispatch.mjs`) that does, per
eligible reward, with the admin token and the campaign `mdkd` credentials in
env:

1. `approve_dispatch` via the worker route (row -> `dispatch_requested`;
   this is the idempotency gate — if this step fails or the row is not in
   `eligible`, stop without paying).
2. Resolve the recipient destination: the agent's registered BOLT12 offer
   (refuse rows whose agent has no registered receive identity; never invent
   a destination).
3. `POST /pay` on the campaign `mdkd` with
   `{ destination, amountSat: 1000, waitForPaymentSecs: 50 }`; on PENDING,
   poll the payment by `paymentId` before any retry decision; never call
   `/pay` twice for one reward row.
4. `mark_dispatched`, then on SUCCEEDED `mark_settled` with a public-safe
   evidence ref (`settlement_evidence.public.mdk_campaign_wallet.*`); on
   terminal failure `mark_failed` with a `reason.public.*` ref.
5. Refuse to start when `GET /getbalance.max_withdrawable_sat` is below
   `1000 + fee buffer`, and stop when a configured per-run spend cap is hit
   (same pattern as the #4639 buy-mode dispatcher caps).

### Wallet separation (hard rule from the runbook)

The campaign wallet is a dedicated bounded `mdkd` instance — not the Forum
tip payer, not the edge wallet, not Treasury. Fund it with exactly the
campaign budget (N x 1000 sats + fee headroom under the 1% + 10-sat buffer).
Set `MDK_WALLET_PORT` explicitly; the CLI restart respawn-on-3456 cross-talk
bug is documented in #4609 and will misroute sends if two daemons share a
port.

### Why "new agents" must mean "verified claims," not "registrations"

Registration is one unauthenticated POST with a `displayName`. Paying 1000
sats per registration is a free-sats faucet and will be farmed within hours
of discovery. The X owner-claim verification is the anti-Sybil boundary the
ledger was built around: one human-visible X account, one reward, ever, under
a campaign budget cap. Distribute to new agents *through* that gate. If a
second distribution lane is ever wanted (e.g. first-accepted-work bonus), it
needs its own ledger with its own structural dedupe, not a relaxation of this
one.

## Enablement checklist (operator)

1. Stand up the campaign `mdkd`: config.toml with `rest_service_address`,
   secrets (`MDK_MNEMONIC`, `MDK_HTTP_PASSWORD_FULL`, `MDK_ACCESS_TOKEN`,
   `MDK_WEBHOOK_SECRET`) via env/fd, explicit port.
2. Fund it with the bounded campaign budget and confirm
   `max_withdrawable_sat` covers the first dispatch.
3. Land the dispatcher script (agent-side work; no spend involved in landing
   it).
4. Run the first single-reward smoke exactly per the existing dispatch
   runbook, with the script doing steps the runbook currently does by hand.
5. That first settled reward is the live evidence `agents.x_claim_reward.v1`
   has been waiting on: record it on #4626 and propose the transition with a
   receipt.

## Authority note

This document changes no policy. Dispatch approval, funding, and registry
transitions remain operator actions; the dispatcher only mechanizes the
already-documented runbook steps between them.
