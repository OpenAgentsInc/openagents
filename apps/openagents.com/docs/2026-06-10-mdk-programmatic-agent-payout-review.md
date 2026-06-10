# MDK Programmatic Agent Payout Review

Date: 2026-06-10 (revised twice same day on owner direction; see Decision)
Author: Fable (review of `projects/moneydevkit/repos/*` reference clones,
https://docs.moneydevkit.com/dashboard/payouts.md and llms.txt, and the live
OpenAgents production integration)

Question under review: how do we enable programmatic payouts so 1000 sats can
be distributed to new agents.

## Decision (owner, 2026-06-10)

Payouts to people come from a **dedicated campaign treasury wallet**: a
regular MDK wallet with its own identity (own mnemonic) and its own funds,
deployed in our cloud infrastructure — not on a local machine, and not
entangled with the revenue node or the MDK dashboard payouts flow. The
dashboard payouts flow stays what MDK built it to be — a way to withdraw the
app's revenue balance to one configured destination — and is at most an
optional funding source for the treasury wallet, not part of the per-agent
payout path.

Revision history: v1 of this doc proposed a campaign `mdkd` but assumed a
local operator machine; v2 overcorrected into routing distribution through
the existing local funded wallets. Both are superseded by the
cloud-deployed treasury wallet above.

## Where our MDK balance actually lives

Production already runs the MDK node. The Worker config sets
`MDK_CHECKOUT_ROUTE_KIND: "self_hosted_mdkd_sidecar"` and deploys the
`openagents-autopilot-mdksidecarcontainer` from
`apps/openagents.com/services/mdk-sidecar/` — a thin proxy that forwards
`/api/mdk` to `@moneydevkit/core/route` (the standard MDK serverless app
handler) with `MDK_MNEMONIC` and `MDK_ACCESS_TOKEN` in its environment.

That self-custodial node is "our balance with Money Dev Kit": revenue from
MDK checkout products (orange checks, and anything else sold through the
dashboard products) accrues to it. Per MDK's own architecture docs
(howitworks.md), MDK opens channels and manages inbound liquidity to that
node; the keys and funds are ours.

Consequence: **no new `mdkd` daemon is needed.** The node that holds the
money is already deployed beside the worker.

## The MDK payouts flow (the platform way to move that balance)

From https://docs.moneydevkit.com/dashboard/payouts.md:

1. Set `WITHDRAWAL_DESTINATION` in the app's environment to a Lightning
   Address (`name@domain`), LNURL, or BOLT12 offer (`lno...`).
2. In the MDK dashboard, choose an amount and click **Pay**.
3. MDK sends a webhook to the app; the app's node spins up and pays the
   configured destination from its own balance.

Under the hood this is the `api-contract` node-control `payout()` RPC:
`{ amountMsat, idempotencyKey }` in, `{ accepted, paymentId, paymentHash }`
out, with completion arriving as `paymentSent` / `paymentFailed` node events.
The destination is read from `process.env.WITHDRAWAL_DESTINATION` on the node
side **by design** — mdk.com cannot redirect funds even if the dashboard or
platform key is compromised.

Current enablement state in this repo: `WITHDRAWAL_DESTINATION` is not set
anywhere (worker vars, sidecar env, or secrets). Setting it on the sidecar
container env and redeploying is the entire platform-side enablement step.

## Why the payouts flow is not the per-agent path

The payouts flow pays exactly **one env-pinned destination**. That is a
security feature, not a gap: mdk.com cannot redirect funds even if the
dashboard or platform key is compromised. It makes the flow right for
withdrawing revenue to the owner's wallet and wrong for fanning out to N
different agents — per-agent destinations would mean rotating an env var per
recipient. Payouts-to-people is a different job with a different owner: a
treasury, not a withdrawal.

## The campaign treasury wallet (target architecture)

A second MDK wallet with its **own mnemonic, own identity, own funds**,
deployed the same way the revenue node already is: as a Cloudflare Container
beside the worker. The existing `services/mdk-sidecar/` container proves the
pattern; the treasury container differs in one way — it exposes a *send*
surface instead of the checkout route. `mdkd` is MDK's stock daemon for
exactly this (`POST /pay` with `{ destination, amountSat,
waitForPaymentSecs }`, `GET /getbalance`, HTTP Basic full-access auth,
secrets via env), so the treasury container is "a regular MDK wallet," not a
custom payment engine. Nothing runs on a local machine.

The pieces and their jobs:

- **Treasury wallet container** (`services/mdk-treasury/`, new): holds the
  campaign funds, executes sends. Its own `MDK_MNEMONIC` (backed up like the
  others), explicit port, full-access password as a deploy secret. Reachable
  only from the worker via its container binding — never publicly routed.
- **Worker-side dispatcher** (new, small): runs in the worker we already
  deploy (scheduled handler or queue consumer — the worker already has a
  `* * * * *` cron trigger and queue infrastructure). It walks
  `x_claim_reward_ledger` rows that an operator has moved to
  `dispatch_requested`, resolves the agent's registered BOLT12 offer, calls
  the treasury container's `/pay`, then `mark_dispatched` /
  `mark_settled`-with-evidence or `mark_failed`. Per-run and per-day spend
  caps enforced in code, same pattern as the #4639 buy-mode dispatcher.
- **Funding** (decoupled): the treasury wallet is funded like any wallet —
  from an external wallet, or optionally via the MDK dashboard payouts flow
  by setting `WITHDRAWAL_DESTINATION` to the treasury wallet's offer and
  clicking Pay to move revenue-balance into it. Either way, funding is an
  operator action separate from distribution.

Boundary note: this is the bounded *campaign* treasury inside the
`openagents.com` surface (marketing rewards with their own ledger and caps).
It is not the private `treasury` repo's settlement daemon; if payout classes
beyond bounded campaigns ever route through it, that work migrates to the
repo that owns payout-execution truth.

## What OpenAgents provides (authority layer, unchanged)

- `x_claim_reward_ledger` (issue #4626, promise `agents.x_claim_reward.v1`,
  yellow): rows enter `eligible` only after a verified X owner-claim tweet,
  with structural anti-Sybil (one reward per X account, ever) and a campaign
  budget cap. `amountSats: 1000` fixed by `agent-claim-reward-policy.ts`.
- Operator dispatch route:
  `POST /api/agents/claims/rewards/{rewardId}/dispatch` with admin bearer;
  actions `approve_dispatch`, `mark_dispatched`, `mark_settled` (requires
  public-safe `evidenceRefs`), `mark_failed`, `refuse`. Runbook:
  `2026-06-09-x-claim-reward-dispatch-runbook.md`.
- Agents carry a receive identity: registration accepts a `bolt12Offer`, and
  the Forum tip-recipient wallet store tracks public-safe recipient
  readiness. A reward payout is "pay the agent's registered offer."

## Execution-layer notes that apply to the dispatcher

- **Idempotency lives in the ledger, not the wallet.** `mdkd /pay` has no
  idempotency key; a reward row passes through `dispatch_requested` exactly
  once and is never paid twice. The dispatcher transitions the row before
  paying and never re-pays a row that has left that state.
- **No `payment_sent` webhook from `mdkd`.** Outbound completion comes from
  the synchronous wait window (`waitForPaymentSecs`, max 50s) or event
  polling; treat PENDING as poll-again with the returned `paymentId`, never
  as a reason to re-send.
- **Liquidity preflight.** Refuse a run when `GET /getbalance`'s
  `max_withdrawable_sat` is below `1000 + fee buffer` (default buffer: 1% +
  10-sat floor + 1.1x retry multiplier, configurable under
  `[max_sendable]`).
- **Cold-channel latency.** First payments that open or splice a channel
  settle correctly but can exceed the wait window; warm channels settle in
  seconds. Keep the fail-then-pass tolerance the tip smokes encode.
- **Self-pay classification.** Shared MDK LSP introduction pubkeys are not
  wallet identity; only per-wallet JIT SCID path entries are (commit
  `14464c96e`). The treasury wallet must refuse destinations that resolve to
  itself.

## Why "new agents" must mean "verified claims," not "registrations"

Registration is one unauthenticated POST with a `displayName`. Paying 1000
sats per registration is a free-sats faucet and will be farmed within hours
of discovery. The X owner-claim verification is the anti-Sybil boundary the
ledger was built around: one human-visible X account, one reward, ever, under
a campaign budget cap. Distribute to new agents *through* that gate. A
second distribution lane (e.g. first-accepted-work bonus) needs its own
ledger with its own structural dedupe, not a relaxation of this one.

## Enablement checklist (operator)

1. Pick the campaign wallet (existing funded local MDK wallet; bounded; not
   the tip payer, edge wallet, or Treasury) and get its BOLT12 offer.
2. Set `WITHDRAWAL_DESTINATION` to that offer in the mdk-sidecar container
   environment (as a deploy-time env/secret, never in tracked config with
   the raw `lno...` value) and redeploy the worker.
3. In the MDK dashboard, Pay the campaign budget (N x 1000 sats + fee
   headroom). Confirm arrival in the campaign wallet.
4. Land the dispatcher script for the distribution hop (agent-side work, no
   spend involved in landing it): walk `eligible` rewards through
   `approve_dispatch -> pay agent offer -> mark_dispatched -> mark_settled`
   with public-safe evidence refs and a per-run spend cap.
5. Run the first single-reward smoke per the existing dispatch runbook. That
   first settled reward is the live evidence `agents.x_claim_reward.v1` has
   been waiting on: record it on #4626 and propose the transition with a
   receipt.

## Authority note

This document changes no policy. Dispatch approval, dashboard payout clicks,
funding decisions, and registry transitions remain operator actions; the
dispatcher only mechanizes the already-documented runbook steps between them.
