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

Consequence: the revenue balance and the container deployment pattern both
already exist. What does not exist is a wallet whose job is paying people —
that is the treasury wallet this doc specifies, kept deliberately separate
from the revenue node.

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

Current state in this repo: the Worker passes the optional
`WITHDRAWAL_DESTINATION` secret through to the MDK sidecar container and the
sidecar reports only `withdrawalDestinationConfigured` on `/healthz`. The raw
destination is still operator-secret material and is not tracked in worker
vars, docs, fixtures, issue comments, or checked-in env files. Under the
decision above it is an optional revenue-withdrawal convenience — set it to
the treasury wallet's offer if revenue should fund the campaign, or to the
owner's wallet for ordinary withdrawals — and it is not on the per-agent payout
path. The live dashboard funding hop is tracked by
`2026-06-10-mdk-dashboard-treasury-funding-runbook.md`.

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

## Enablement checklist

Agent-side (no spend involved in landing any of it):

1. Add the `services/mdk-treasury/` container (mdkd image, own config,
   secrets via deploy-time env: fresh `MDK_MNEMONIC`, full/read-only
   passwords, explicit port) and its wrangler container binding, reachable
   from the worker only.
2. Land the worker-side dispatcher behind the existing operator gates:
   consumes `dispatch_requested` reward rows, pays the agent's registered
   BOLT12 offer via the treasury binding, marks
   `dispatched`/`settled`-with-evidence/`failed`, enforces per-run and
   per-day spend caps, exposes a public-safe status projection.
3. Tests + a no-spend smoke (mock treasury binding) before any live run.

Operator-side (bounded actions):

4. Generate and back up the treasury mnemonic; set the deploy secrets.
5. Fund the treasury wallet with the campaign budget (N x 1000 sats + fee
   headroom) — externally, or via the dashboard payouts flow by pointing
   `WITHDRAWAL_DESTINATION` at the treasury offer and clicking Pay.
6. Approve the first single reward (`approve_dispatch`) and let the
   dispatcher run it end to end. That first settled reward is the live
   evidence `agents.x_claim_reward.v1` has been waiting on: record it on
   #4626 and propose the transition with a receipt.

## Live state and payout policy (updated 2026-06-10, post-launch)

The treasury shipped and funded the same day:

- Container live as `openagents-mdk-treasury-20260610-2`, state `configured`
  (issue #4698 closed; #4699 now has a Worker-side dispatcher behind
  `TREASURY_DISPATCH_ENABLED=false` by default; #4700 remains the revenue
  funding hop).
- Secrets generated and backed up per the workspace secrets convention; no
  MDK dashboard key was needed (the platform accepts a self-generated wallet
  id as `mdkApiKey`, the agent-wallet pattern).
- First funding: 500 sats from the local edge payer wallet over the BOLT11
  JIT rail (10 sats LSP fee) -> 490 sats balance, 480 spendable. Revenue-scale
  funding remains #4700's dashboard payout hop.
- Owner payout policy (2026-06-10): payouts go through
  `POST /api/operator/treasury/payout`, which pays in full when
  `maxSendableSat` covers the intended amount and otherwise falls back to
  **10% of the current spendable, floored** (intended 1000 against 990
  spendable pays 99; successive payouts take 10% of the then-current
  spendable). Below 10 sats spendable the route refuses with
  `treasury_depleted`. Policy code and tests: `treasuryPayoutPlan` in
  `workers/api/src/treasury-routes.ts`.
- The single Artanis-facing operating contract for all of this is
  `docs/artanis/treasury-runbook.md`.
- #4699 dispatcher shape: scheduled Worker tick, one approved reward row per
  run by default, 5000 sats per UTC day by default, liquidity preflight before
  new sends, private treasury payment id stored for pending-poll idempotency,
  and public-safe operator status stats only. This is still no-spend code
  until the operator explicitly enables the flag and records a live dispatch
  smoke.

## Authority note

This document changes no policy. Dispatch approval, dashboard payout clicks,
funding decisions, and registry transitions remain operator actions; the
dispatcher only mechanizes the already-documented runbook steps between them.
