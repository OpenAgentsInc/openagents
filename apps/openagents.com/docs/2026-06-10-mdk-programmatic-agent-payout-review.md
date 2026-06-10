# MDK Programmatic Agent Payout Review

Date: 2026-06-10 (revised same day after owner correction; the first version
wrongly recommended standing up a new standalone `mdkd` campaign daemon)
Author: Fable (review of `projects/moneydevkit/repos/*` reference clones,
https://docs.moneydevkit.com/dashboard/payouts.md and llms.txt, and the live
OpenAgents production integration)

Question under review: how do we enable programmatic payouts so 1000 sats can
be distributed to new agents — using the balance we already hold with
MoneyDevKit and MDK's own payouts flow, not new wallet infrastructure?

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

## The one structural constraint, stated honestly

The payouts flow pays exactly **one env-pinned destination**. That is a
security feature, not a gap, and we should not fight it. It means the
dashboard Pay button can move the campaign budget *out of the app balance in
one hop*, but it cannot fan out to N different agents' wallets directly —
per-agent destinations would require rotating an env var per recipient.

So the agent-reward architecture is two hops, each using MDK surfaces we
already operate:

1. **Budget hop (MDK payouts flow).** `WITHDRAWAL_DESTINATION` = the BOLT12
   offer of the existing funded operator/campaign wallet (the same local MDK
   wallet family already used for the funded tip smokes — not Treasury, not
   the edge wallet). Operator clicks Pay for the campaign amount
   (N x 1000 sats + fee headroom). One click, one webhook, idempotent,
   self-custodial end to end.
2. **Distribution hop (per-agent sends from the campaign wallet).** The
   funded wallet pays each eligible agent's registered BOLT12 offer:
   1000 sats per reward row, driven by the `x_claim_reward_ledger` state
   machine (`approve_dispatch -> pay -> mark_dispatched -> mark_settled`)
   via the wallet's existing send surface — the local wallet daemon's
   `POST /pay` (`{ destination, amountSat, waitForPaymentSecs }`) or
   `npx @moneydevkit/agent-wallet send <offer> 1000`. No new daemon: this is
   the wallet that already exists and already holds spend history from the
   tip smokes.

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

## Execution-layer notes that still apply to the distribution hop

- **Idempotency lives in the ledger, not the wallet.** The wallet `/pay`
  call has no idempotency key; a reward row passes through
  `dispatch_requested` exactly once and is never paid twice. (The payouts
  flow's own `idempotencyKey` covers the budget hop.)
- **No `payment_sent` webhook from the local wallet daemon.** Outbound
  completion comes from the synchronous wait window (max 50s) or event
  polling; treat PENDING as poll-again.
- **Cold-channel latency.** First payments that open or splice a channel
  settle correctly but can exceed the wait window; warm channels settle in
  seconds. Keep the fail-then-pass tolerance the tip smokes encode.
- **Self-pay classification.** Shared MDK LSP introduction pubkeys are not
  wallet identity; only per-wallet JIT SCID path entries are (commit
  `14464c96e`).
- **Port discipline.** Set `MDK_WALLET_PORT` explicitly; the CLI
  restart-respawn-on-3456 cross-talk bug (#4609) misroutes sends when two
  wallets share a port.

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
