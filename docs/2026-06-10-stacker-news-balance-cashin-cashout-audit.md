# Stacker News Balance, Cash-In, and Cash-Out Audit

Date: 2026-06-10

Source: read-only reference clone `projects/repos/stacker.news` (upstream
`stackernews/stacker.news`, clone at `3e282355`, 2026-06-02), primarily
`api/payIn/README.md`, `api/payIn/types/zap.js`, `worker/autowithdraw.js`,
`prisma/schema.prisma`, and `wallets/`.

Why this audit: today's live BOLT 12 tip testing showed the pure
peer-to-peer model is brittle in exactly the way the owner suspected. A
BOLT 12 offer can only be paid while the recipient's self-custodial node
is up to sign the invoice (per the BOLT 12 spec, the `invoice_request`
onion message must reach the offer's node; MDK's infrastructure provides
channels and liquidity but cannot sign for a self-custodial wallet). Tips
to a responsive-but-slow recipient (Kenobi) timed out client-side and
completed asynchronously; tips to an unreachable recipient (Comunero)
failed outright from two independent payer wallets including the
treasury. Stacker News solved this exact problem class. This audit
records how, and what we should port.

## 1. The arc: custodial → non-custodial with a custodial buffer

SN began fully custodial: `users.msats` was the ledger, and "any table
with an msats column could represent a debit or credit." They then moved
to non-custodial-by-default — but **did not** move to pure p2p. The
landing point is a three-asset model:

- **`users.msats` ("reward sats")** — withdrawable custodial sats,
  earned mainly from the rewards system. Real money; can leave.
- **`users.mcredits` ("cowboy credits", CCs)** — a non-withdrawable
  internal credit, 1 CC ≙ 1 sat for spending inside the platform.
  Bought with lightning (`buyCredits` pay-in type), earned when a zap
  to you cannot be delivered p2p. Cannot be cashed out.
- **Attached wallets** — per-user send/receive wallet attachments
  (`WalletTemplate` with `sendProtocols`/`recvProtocols`, including
  BOLT11, BOLT12, LNURL…) used for true p2p payment when both sides
  can.

The denormalization convention: aggregate surfaces carry a `sats` value
that *includes* credits (`item.msats` = sats + credits earned, with
`item.mcredits` carrying the credit share), but the `users` table keeps
the two balances strictly separate — because only one of them is real,
withdrawable money. The asset-preference rule: **spend the less
desirable asset first** (CCs before reward sats before lightning).

The regulatory shape of this is deliberate: the withdrawable balance is
something users *earned and can sweep out* (and autowithdraw keeps it
small — below), while the buffer that makes UX reliable (CCs) is
explicitly non-withdrawable, which keeps the platform out of the
business of warehousing other people's money.

## 2. The ledger: Pay Ins

Everything paid is a **pay-in type** plugged into one shared ledger and
one state machine (`api/payIn/README.md`). The parts worth stealing
wholesale:

- A `PayIn` row records every attempt: type, cost, payer, state, with
  `PayInStateChangedAt` per transition. It is created **atomically**
  with the records that fund it (`PayInCustodialToken` debits and/or
  `PayInBolt11`) and the records that say where the money goes
  (`PayOutCustodialToken` and/or `PayOutBolt11`). Custodial debits
  happen at creation time; failure refunds them.
- Each custodial token record stores **the resulting balance** the
  account will have if the pay-in reaches `PAID` — auditing is built
  into the rows, not reconstructed later.
- One **state machine** covers all flows:
  `PENDING_INVOICE_CREATION → PENDING → PAID/FAILED` (optimistic),
  hold-invoice `PENDING_HELD → HELD → PAID/CANCELLED` (pessimistic),
  `PENDING_INVOICE_WRAP → … → FORWARDING → FORWARDED → PAID` (p2p), and
  `PENDING_WITHDRAWAL → PAID/FAILED` (cash-out). Non-custodial legs are
  assumed *slow and unreliable* and therefore monitored and retriable
  by design — retries clone the `PayIn` with a `genesisId` chain and an
  optimistic lock (`successorId` set-if-null) so a payment can never be
  retried twice concurrently.
- A payment for one action can be **fractional across assets**: CCs +
  reward sats + a lightning invoice can jointly cover one cost.
- The README closes with hard-won Postgres discipline: `read committed`
  serialization anomalies (always `UPDATE … SET x = x + n`, never
  read-then-write), row-lock ordering to avoid deadlocks (their zap
  split deadlock was fixed by `ORDER BY userId` before `FOR UPDATE`).

## 3. Cash-in

Three ways money enters a user's balances:

1. **Buy credits** (`buyCredits` pay-in): pay a lightning invoice to SN,
   receive non-withdrawable CCs 1:1. This is the deliberate custodial
   on-ramp — users park spending money as credits precisely because
   credits never fail and never pay routing fees.
2. **Earn from zaps** (the fallback, §5): when someone zaps you and p2p
   delivery is not possible, you are credited CCs instantly.
3. **Rewards** (`users.msats`): the platform's own reward distributions
   land as withdrawable reward sats.

## 4. Cash-out

Withdrawable balance leaves in exactly one way: a **withdrawal pay-in**
whose payout is a `PayOutBolt11` (the only custodial pay-in that pays
out to lightning). Two triggers:

- Manual withdraw (user supplies an invoice).
- **Autowithdraw** (`worker/autowithdraw.js`): the user sets
  `autoWithdrawThreshold`, `autoWithdrawMaxFeePercent`, and
  `autoWithdrawMaxFeeTotal`; a worker pushes the **excess above the
  threshold** out to the user's attached receive wallet whenever the
  excess exceeds 10% of the threshold and at least 100 sats, fee-capped,
  with pending-withdrawal and recent-attempt dedup. The platform
  actively minimizes how much custodial money it holds, and it does the
  pushing **from the server side, on a worker schedule, with retries** —
  meaning recipient-side flakiness costs nothing: a failed attempt just
  waits for the next tick.

CCs never cash out. That is the design, not a gap.

## 5. The zap ladder — the part that answers today's problem

`api/payIn/types/zap.js` orders payment methods:
`P2P → FEE_CREDIT → REWARD_SATS → OPTIMISTIC → PESSIMISTIC`, and the
receive side is the masterstroke:

1. **Try p2p first, with thresholds.** Small zaps skip lightning
   entirely: senders with `sats < sendCreditsBelowSats` (default 10) pay
   from credits; recipients with a share below their
   `receiveCreditsBelowSats` (default 10) are not even attempted p2p.
   Micro-payments never touch the network — no fees, no latency, no
   invoice fetch.
2. **Test the invoice before committing** when the sender can't
   auto-retry (anon/no send wallet): `canWrapBolt11` validates the
   recipient's invoice up front.
3. **The p2p mechanism**: request an invoice from the recipient's
   attached wallet, **reuse its payment hash inside a wrapped hold
   invoice paid to SN** (collecting a 3% routing/sybil fee), and when
   the sender pays the wrapper, SN forwards its own funds to the
   recipient; the recipient's preimage reveal settles the wrapper. An
   application-layer lightning forward — sender-to-recipient custody
   never rests with SN, but SN absorbs the timing risk.
4. **Anyone who can't get p2p gets credits — instantly,
   unconditionally.** The loop in `getInitial` is explicit: one
   candidate may win the `PayOutBolt11`; *every other candidate*
   (including the p2p candidate when invoice creation throws) receives
   `PayOutCustodialToken` records in CREDITS. A zap **never fails
   because the recipient is unreachable**. The recipient's wallet being
   down converts the payment's form, not its success.

This is the reliability inversion we want: today our tip *fails* when
the recipient's node is down; SN's zap *degrades* to an instant credit
and the recipient sweeps real sats later when their wallet is up (or
spends the credits in place).

## 6. What OpenAgents should port

Mapping onto what we already have (forum direct tips and paid actions,
the MDK agent wallets, the treasury container, `treasury_transactions`
in D1, the payment-mode ladder vocabulary):

1. **Agent credit balances (CC-equivalent).** A per-agent,
   non-withdrawable `mcredits` balance in D1, spendable on platform
   actions (tips, boosts, paid actions) at 1 credit ≙ 1 sat. Tips
   *credit instantly and atomically* — D1's single-writer model makes
   SN's hardest concurrency problems (read-committed anomalies,
   deadlock ordering) largely moot, but keep the `x = x + n` increment
   discipline anyway.
2. **The receive ladder.** On every tip: attempt direct BOLT 12 to the
   recipient's registered offer (bounded attempt window, since we now
   know invoice-fetch latency is recipient-daemon-bound) → on failure or
   below-threshold, credit the recipient. The tip **always succeeds**;
   only its form varies. Record which rung served (`direct_bolt12` vs
   `credited`) in the receipt, mirroring our payment-mode vocabulary.
3. **Server-side sweep (autowithdraw-equivalent).** Agents set a
   threshold and their registered offer; a worker cron (the same
   scheduled handler Artanis ticks in) periodically attempts to push
   credited balances out over BOLT 12, with fee caps, minimums, dedup,
   and indefinite retry. This moves the recipient-must-be-online problem
   from the moment of tipping (where it breaks UX) to a background loop
   (where it costs nothing) — precisely the fix for today's Comunero
   case. Note an asset decision here: SN's credits never cash out, only
   reward sats do. We must pick: (a) SN-faithful — tips received as
   credits are spendable but not sweepable (cleanest custody posture),
   or (b) sweepable credits (better contributor story, heavier custody
   posture). The owner should make this call explicitly; the SN
   precedent is (a) plus a separate withdrawable balance for
   platform-paid rewards.
4. **The PayIn-shaped ledger.** Our forum paid-actions and
   `treasury_transactions` already gesture at this; unify toward
   SN's shape: every paid attempt is a row created atomically with its
   funding debits and payout intents, storing resulting balances, with
   a typed state machine and genesis/successor retry chains. This
   subsumes today's ad-hoc `recovery_pending` direct-tip attempts —
   #4704's stuck-reconciliation bug is exactly the class of problem the
   PayIn state machine plus refund-on-fail eliminates.
5. **Thresholds as user/agent preferences.** `sendCreditsBelowSats` /
   `receiveCreditsBelowSats` (default 10) are quietly the best UX
   feature in the file: micro-tips never touch lightning at all. Adopt
   both, as agent registration fields.

What does **not** port: SN's wrapped hold-invoice p2p forwarding
requires running a routing-capable node (their LND) and hold-invoice
support; the MDK agent-wallet surface doesn't expose that, and our
serverless-wallet model isn't positioned to be an application-layer
forwarder today. We get most of the reliability win without it: direct
BOLT 12 when the recipient is up, credits when not, background sweep to
make credits real. The hold-invoice wrap (which also collects SN's 3%
sybil fee trustlessly) is worth revisiting if/when we run an
always-on routing node.

## 7. Open questions for the owner

- **Credits custody posture**: SN-faithful non-withdrawable credits
  (sweep only for platform-earned rewards), or sweepable balances?
  This is the policy decision everything else hangs on.
- **Where the buffer lives**: D1 ledger backed by the treasury
  container's funds (credits are claims on the treasury wallet), or a
  dedicated buffer wallet? SN's equivalent is "claims on SN's node."
- **Fee policy**: SN takes 3% routing fee on p2p and 30%→rewards-pool
  splits on zaps; do tips carry a platform fee at all in our model?
- **Sybil pressure**: SN's fee structure is also their sybil defense;
  if credits are free-flowing, what replaces that here (the existing
  orange-check / payment-gated posting economics may already cover it)?

## 8. Bottom line

Stacker News' answer to "tips must be reliable" is not better lightning
— it is **a ladder where lightning is the first rung and an instant
internal credit is the floor**, plus a background worker that converts
credits into real settled sats whenever the recipient is actually
reachable, plus a single audited ledger that makes every attempt,
fallback, refund, and retry a typed row. Every piece of that maps onto
infrastructure we already run (D1, the worker cron, MDK wallets, the
treasury, the payment-mode vocabulary). The brittle part of today's
testing was never the payments — it was putting the recipient's uptime
on the critical path of the sender's experience. SN moved it off; we
should too.
