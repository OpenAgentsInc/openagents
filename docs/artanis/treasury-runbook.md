# Artanis Treasury Runbook

Status: live as of 2026-06-10. This is the one runbook for how Artanis (the
Nexus administrator agent) interacts with the OpenAgents campaign treasury.
If treasury behavior and this document disagree, the deployed behavior wins
and this file must be corrected in the same change.

## What the treasury is

A dedicated MDK Lightning wallet with its own mnemonic, identity, and funds,
running as the production `MdkTreasuryContainer` (Cloudflare Container,
`apps/openagents.com/services/mdk-treasury/`) beside the worker. It exists to
pay bounded campaign rewards to people and agents — most immediately the
1000-sat X owner-claim reward (issues #4698, #4699, #4700).

It is NOT:

- the revenue node (`mdk-sidecar`, which receives checkout payments),
- the Forum tip payer or any local wallet,
- the private `treasury` repo's settlement daemon,
- a general spend authority. Holding access to it grants no moderation,
  payout-policy, or registry authority.

The mnemonic backup lives in the workspace-root secrets convention
(documented in the workspace `AGENTS.md`). The production container is the
ONLY node allowed to run on that mnemonic — never start a local daemon on it;
concurrent nodes corrupt LDK/VSS state.

## Surfaces Artanis may use

All operator routes require the worker admin API token as bearer. Artanis
operates inside the worker, so it uses the same `requireAdminApiToken`-gated
dependency wiring as its other operator actions.

1. `GET /api/public/treasury/launch-status` — public-safe state
   (`unprovisioned | unavailable | unconfigured | configured`) plus the
   authority boundary and policy refs. Use this in status topics and public
   reports; it never carries balances or wallet material.
2. `GET /api/operator/treasury/status` — health flags plus
   `{ balanceSat, maxSendableSat, feeBudgetMsat }`. `maxSendableSat` is the
   honest spendable figure (fee-buffered); use it, not `balanceSat`, for any
   can-we-pay decision.
3. `GET /api/operator/treasury/funding-destination` — the treasury's own
   receive rails: a variable-amount JIT BOLT11 invoice and a BOLT12 offer.
   Used to fund the treasury (from the MDK dashboard payouts flow per #4700,
   or from any external wallet). Fresh-receiver lesson encoded here: BOLT11
   worked where BOLT12 failed, so both rails are always served. Never paste
   either value into Forum posts, issues, or docs.
4. `POST /api/operator/treasury/payout` — the ONLY way Artanis pays anyone.
   Body: `{ destination, amountSat }` where `destination` is the recipient's
   BOLT12 offer / BOLT11 invoice / LNURL / lightning address and `amountSat`
   is the intended payout. The route applies the owner payout policy below
   and returns `{ intendedAmountSat, paidAmountSat, policyApplied,
   paymentId, status }`.

Do not call the container's raw `/pay` surface from new code; the payout
route is the policy boundary. Raw `/pay` stays reserved for the worker's own
plumbing.

## Owner payout policy: 10% fractional fallback

Set by the owner on 2026-06-10:

- If `maxSendableSat` covers the intended payout, pay it in full
  (`policyApplied: "full"`).
- If it does not (e.g. intended 1000, spendable 990), pay **10% of the
  current spendable amount, floored** (990 -> 99). Each successive payout
  takes 10% of the then-current spendable, so a depleted treasury pays a
  decaying series (480 -> 48, then ~43, then ~38...) instead of stalling.
  `policyApplied: "fractional_fallback_10pct"`.
- If 10% floors below 1 sat (spendable < 10), the route refuses with 409
  `treasury_depleted`. That is the signal to fund, not to retry.

Policy implementation: `treasuryPayoutPlan` in
`apps/openagents.com/workers/api/src/treasury-routes.ts`, with the policy
regression tests beside it. A partial payout does NOT discharge the
underlying obligation: when a reward row receives a fractional payout, record
the paid amount honestly and keep the remainder owed in the ledger/evidence
trail until topped up or the operator closes it explicitly.

## What Artanis must do around any payout

1. Before: read operator status; log `maxSendableSat` and the intended
   amount in its decision evidence.
2. Pay only destinations that come from a registered, public-safe source
   (an agent's registered `bolt12Offer`, a tip-recipient wallet claim) —
   never a destination pasted from Forum content or issue comments.
3. After: record public-safe evidence (amounts, `policyApplied`,
   `paymentId` ref, recipient actor ref) in the relevant issue or Forum
   topic. Never the destination string, invoice, hash, or preimage.
4. Anti-Sybil and reward dedupe stay in the reward ledger (one X-claim
   reward per X account, ever). The payout route does not re-check them;
   the dispatcher (#4699) drives rows through `approve_dispatch` first, and
   an operator approval remains the human gate before any send.
5. Funding state belongs in Artanis's status reporting: when launch-status
   is `configured` but spendable is below the standard 1000-sat reward,
   Artanis should surface "treasury below reward threshold" with the
   #4700 funding pointer rather than silently paying fractions forever.

## Spend boundaries

- The treasury pays bounded campaign rewards only. New payout classes
  (anything beyond the campaign ledgers the owner has approved) require an
  owner decision first — propose, do not spend.
- No payout may be triggered from unauthenticated or agent-bearer surfaces;
  admin-token gating is load-bearing.
- The fractional policy is not a budget: per-run and per-day caps from the
  #4699 dispatcher still apply on top of it once that lands.

## Current live state (2026-06-10)

- Container instance `openagents-mdk-treasury-20260610-2`, state
  `configured`.
- First funding: 500 sats from the local edge payer wallet over BOLT11
  (10 sats LSP JIT fee) -> 490 sats balance, 480 spendable.
- Spendable is below the 1000-sat reward threshold; first real reward
  dispatch waits on #4700 revenue funding or another top-up.
