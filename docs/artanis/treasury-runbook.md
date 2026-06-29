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
4. `POST /api/operator/treasury/payout` — the operator/Artanis direct payout
   path. Body: `{ destination, amountSat }` where `destination` is the
   recipient's BOLT12 offer / BOLT11 invoice / LNURL / lightning address and
   `amountSat` is the intended payout. The route applies the owner payout
   policy below and returns `{ intendedAmountSat, paidAmountSat,
   policyApplied, paymentId, status }` on success. On failure it returns a JSON
   body with `error`, `policyApplied`, a public-safe `reasonRef`, and safe
   diagnostics when the treasury container can provide them (`destinationKind`,
   `failureStage`, `preflightMaxSendableSat`, `reasonClass`, `timeoutSecs`,
   `errorName`, `errorCode`, `messageFingerprint`);
   manual operator calls must use `curl --fail-with-body` or omit `-f` so that
   body is not suppressed. The container classifies known failures before the
   Worker sees them, and the Worker falls back to its own classifier for older
   or generic payloads. `messageFingerprint` is a SHA-256 fingerprint of the
   raw daemon message for correlation only; the route still refuses to store or
   echo raw daemon text, destinations, invoices, payment hashes, or preimages.
   A failure before a durable payment id means no sats have been proven to leave
   the treasury; confirm by re-reading `balanceSat` / `maxSendableSat`.
   Large Lightning Address sends may fail even while smaller sends to the same
   recipient settle. In that case, split an owner-approved obligation into
   explicit smaller payouts and record each settled row, rather than retrying a
   single large invoice from memory.
5. Scheduled X-claim dispatcher — the Worker-internal path for already
   operator-approved `x_claim_reward_ledger` rows in `dispatch_requested`.
   It is controlled by `TREASURY_DISPATCH_ENABLED`, which defaults off. When
   enabled, it resolves the agent's registered BOLT12 offer from the
   tip-recipient wallet store, claims one row by moving it to `dispatched`,
   calls the treasury container, stores the private treasury payment id for
   later polling, and records only public-safe dispatch/settlement refs.
   Pending payments are polled on later ticks; rows are never re-paid.

Do not call the container's raw `/pay` surface from arbitrary new code. The
operator payout route and the scheduled X-claim dispatcher are the two modeled
Worker-side policy boundaries; raw `/pay` stays reserved for those internal
paths.

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
- The fractional policy is not a budget: the #4699 dispatcher applies
  `TREASURY_DISPATCH_PER_RUN_REWARD_CAP` (default 1) and
  `TREASURY_DISPATCH_DAILY_SATS_CAP` (default 5000) on top of treasury
  spendability. It also skips new sends unless `maxSendableSat` covers the
  1000-sat reward plus the configured liquidity buffer.

## Public treasury page and donations

`/treasury` is a public worker-served page (styled like the homepage: black,
white, Berkeley Mono) showing the live balance, spendable amount, and the
last 20 transactions — time, direction, amount, and state only. Recipients,
destinations, payment hashes, and invoice material are never shown for other
people's rows. JSON projection: `GET /api/public/treasury`.

Donations: `/treasury/donate` mints a fresh variable-amount JIT BOLT11
invoice from the treasury node and redirects to
`/treasury/donations/{id}` — a checkout-style page with QR code and
`lightning:` link that auto-refreshes until the payment arrives, then thanks
the donor with the received amount. Invoices expire after one hour.

Ledger: every successful payout through `POST /api/operator/treasury/payout`,
every pre-dispatch payout failure, and every donation is recorded in the
`treasury_transactions` D1 table (migrations 0159, 0197, 0198). Failed payout
attempts without a durable MDK payment id store `payment_ref:null` and a
public-safe `failure_reason_ref`; they never store raw destinations, BOLT11
invoices, hashes, preimages, or daemon error text. Operator failure responses
may include the public-safe diagnostics listed above; the public page reads
from the ledger but shows only time, direction, amount, and state.

Outbound MDK payment outcomes are now also journaled in the Worker-side
`MdkTreasuryContainer` / `MdkTipsBufferContainer` Durable Object storage. When
`POST /pay` or `GET /payments/{paymentId}` returns a terminal `succeeded` or
`failed` state, the Container wrapper stores only `{ status, reasonRef }` under
the private payment id. Later reconciliation can recover that terminal state
even if the Bun service process lost its in-memory event map. This journal is
not a public ledger and does not store raw daemon text, payment destinations,
invoices, hashes, preimages, mnemonics, or tokens. If no Worker request ever
observed the terminal event before a hard container loss, the row remains
pending rather than guessed.

The container's in-memory receive tracking means a donation confirmed across a
container restart may sit pending until expiry even though the sats arrived —
balance is always authoritative.

Artanis should link `/treasury` (never raw balances pasted into posts) when
reporting treasury state publicly, and may cite `GET /api/public/treasury`
as the machine-readable source.

## Current live state (2026-06-10)

- Container instance `openagents-mdk-treasury-20260610-2`, state
  `configured`.
- First funding: 500 sats from the local edge payer wallet over BOLT11
  (10 sats LSP JIT fee) -> 490 sats balance, 480 spendable.
- Worker-side X-claim dispatch code is landed and covered by no-spend tests,
  but `TREASURY_DISPATCH_ENABLED` defaults off. First real reward dispatch
  still waits on operator enablement plus enough treasury spendability.
