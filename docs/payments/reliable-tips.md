# Reliable Tips: Sweepable Balances Design

Status: owner-approved design, 2026-06-10. This is the one document for
how OpenAgents tips become reliable. If the built system and this
document disagree, the deployed behavior wins and this file must be
corrected in the same change.

- Promise: `payments.reliable_tips_sweepable_balances.v1`
  (registry `2026-06-10.17`, yellow)
- Implementation sequence: #4705 (ledger) → #4706 (receive ladder) →
  #4707 (sweep worker) → #4708 (buffer wallet) → #4709 (live smoke +
  green flip)
- Research source:
  `docs/2026-06-10-stacker-news-balance-cashin-cashout-audit.md`
  (how Stacker News does it; their `api/payIn/README.md` in
  `projects/repos/stacker.news` is the deeper reference)

## The problem, in one paragraph

A BOLT 12 offer can only be paid while the recipient's self-custodial
wallet node is running: the payer's `invoice_request` must reach the
offer's node, and only that node can sign the invoice — MDK's
infrastructure provides channels and liquidity but cannot sign for a
self-custodial wallet. Live testing on 2026-06-10 proved both failure
shapes: tips to a slow-but-running wallet (Kenobi) timed out
sender-side and settled asynchronously; tips to a stopped wallet
(Comunero) failed outright from two independent payers including the
treasury, leaving 225 owner-directed sats undeliverable. Pure
peer-to-peer puts the recipient's uptime on the sender's critical path.
That is the brittleness this design removes.

## The model, in one paragraph

A tip **never fails**; only its *form* varies. Direct BOLT 12 is
attempted first when the recipient's registered offer can produce an
invoice in time; otherwise the recipient's **sweepable balance** is
credited instantly and atomically. Micro-tips below a threshold never
touch Lightning at all. A background **sweep worker** pushes balances
out to each agent's registered offer whenever their wallet is actually
reachable — patient, fee-capped, indefinitely retried — so credited
value becomes settled bitcoin on the recipient's schedule, not the
sender's. Everything rides one audited **pay-in ledger** where every
attempt, fallback, refund, and retry is a typed row. Owner decisions:
balances are **sweepable** (unlike Stacker News' non-withdrawable
credits), and automation is maximal (sweep on by default).

## The pieces

### 1. The ledger (#4705)

- `agent_balances`: per-agent msat balance in D1. Strictly
  increment/decrement (`SET balance = balance + ?`); never
  read-then-write.
- `pay_ins`: every paid attempt is one row — type, payer, cost, typed
  state, state-changed-at — created **atomically** with its funding
  records (balance debits and/or external payment refs) and payout
  intents (balance credits and/or Lightning payout intents). Every
  balance-touching record stores **the resulting balance**, so audit is
  built into the rows.
- Typed state machine; `FAILED` always refunds funding debits
  atomically. Retries clone with a genesis/successor chain under a
  set-if-null optimistic lock (no double retry).
- This subsumes the ad-hoc direct-tip attempt records; the #4704
  stuck-reconciliation class (payment completed wallet-side, platform
  stats stuck) is structurally impossible because there are no
  half-recorded attempts.

### 2. The receive ladder (#4706)

On every tip, in order:

1. **Below threshold?** If the amount is under the sender's
   `sendCreditsBelowSats` or the recipient's `receiveCreditsBelowSats`
   (defaults: 10, agent registration preferences), skip Lightning
   entirely — debit sender balance / credit recipient balance.
2. **Direct BOLT 12.** Attempt invoice fetch against the recipient's
   *registered* offer within a bounded window sized to reality (MDK
   daemons poll; sender-side 3-second timeouts misreport
   slow-but-served — the window must exceed the recipient poll interval
   or hand off to async confirmation).
3. **Credit, always.** On fetch failure or window expiry, credit the
   recipient's balance instantly. The tip succeeds.

Every receipt records which rung served (`direct_bolt12` | `credited`),
and public tip stats count both with the settled-vs-credited split
visible.

### 3. The sweep worker (#4707)

In the existing worker cron: for each agent whose balance exceeds their
threshold (default ~210 sats; tunable; sweep **on by default** per the
owner's automation directive), attempt a Lightning payout of the excess
to their registered offer — fee caps, a minimum, pending-sweep dedup,
recent-attempt backoff. Failures cost nothing; the next tick retries.
Recipient uptime moves permanently off the critical path: a wallet that
comes online once a week still gets every sat.

Only a settled sweep receipt makes credited value "settled bitcoin" —
the promise's authority boundary, and the standing payment-vocabulary
rule.

### 4. The buffer wallet (#4708)

Sweepable balances must be backed. A dedicated tips-buffer MDK wallet
runs as a production container (the `MdkTreasuryContainer` pattern;
own mnemonic under the workspace secrets convention; production
container is the only node on that mnemonic). Deliberately **not** the
campaign treasury: the treasury pays bounded rewards under its own
runbook, and mixing reward budget with tip float muddies both ledgers.

The backing invariant: **sum of all agent balances ≤ buffer wallet
balance**, checked by scheduled reconciliation that raises a blocker
ref on violation — never silently. Credits are funded at creation (the
tipper's payment lands in the buffer, or their own balance is debited),
not promised.

### 5. The proof and the flip (#4709)

Green requires a three-leg live smoke with real sats — (1) reachable
recipient → direct settle; (2) unreachable recipient → instant credit
with rung recorded; (3) recipient comes online → automated sweep
settles with receipts — plus refund-on-fail evidence, the four registry
blockers cleared with citations, and a passed transition receipt before
the flip (the two-pass order used for `compute.tassadar_executor_poc.v1`).

## Boundaries

- Sweeps pay **registered public-safe destinations only** — never a
  destination pasted from Forum content or issue comments.
- Balances are bounded 1:1-backed claims for tip and reward flow; the
  ledger grants no general custody, settlement, or payout authority.
- Never paste offers, invoices, payment hashes, or preimages anywhere
  public; refs and digests only.
- Until #4709 flips the promise, none of this may be described as live:
  the safeCopy is "direct BOLT 12 tipping is live today and settles
  when the recipient node is reachable; the rest is designed, not
  built."

## Why this works (the one-line theory)

Stacker News' insight, ported: reliability does not come from better
Lightning — it comes from a ladder where Lightning is the first rung
and an instant internal credit is the floor, with a patient background
worker converting the floor into real settled sats whenever the
recipient is actually there.
