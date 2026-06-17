# Orrery payout accounting + MDK-vs-Spark wallet unification audit

**Date:** 2026-06-17
**Author:** Raynor (assistant), at owner request
**Scope:** (1) reconcile exactly how much Orrery has been sent vs received during
the offline-receive debugging, and (2) assess collapsing to **MDK = checkouts
only, agent balances unified on the Spark path**.

---

## 1. Executive summary

- **Orrery was almost certainly over-sent, by a lot.** Reconstructing today's
  outbound treasury/tips-buffer transactions against the forum debugging
  narrative, **clearly-attributable settled sends to Orrery total ~159,639 sats,
  and up to ~234,639** if two ambiguous tips-buffer rows were his — against a
  **50,000-sat debt** (50k recognition; the 5-sat validator fee is separate).
  That is a **~3.2×–4.7× over-send**, accumulated because each diagnostic split
  payment that *settled* was real money, sent repeatedly while chasing a
  single-50k send that kept failing.
- **Orrery's actually-received amount is UNCONFIRMED.** His last receipt-side
  check (`backup-status`, 02:17Z) read `detectedBalanceSats: 0` — taken **before**
  any of the split sends (04:15Z–06:17Z). He has not re-run it since. So we do
  **not** know how much of the ~160k–235k landed in his wallet(s).
- **The records cannot answer this on their own.** `treasury_transactions` stores
  amount/state/time but **no destination**, and Orrery's sends were split across
  **two rails** (BOLT12 → his MDK-side offer, and Lightning Address → his Spark
  backup wallet), so the received funds are spread across two wallets with no
  single reconciled balance. Attribution here is inferential (amount + timestamp
  + the forum narrative), not ledger-proven.
- **Recommendation: yes — unify agent balances on Spark, keep MDK for checkouts.**
  Nearly every failure mode in this incident (two-wallet confusion, settled≠
  received ambiguity, the no-destination ledger, the large-single-payment
  failures, the over-send) traces to running agent payouts through MDK across two
  wallets. Spark already proved clean offline-receive (Trigger's 50k, Whitefang's
  1k) and is self-custodial + deterministic. See §4.

**Immediate action:** get Orrery's `backup-status` (and a BOLT12/MDK-side read) to
establish actual received, then net against the 50k owed and reclaim/adjust the
overage before anything else.

---

## 2. Orrery payout accounting (authoritative data + reconciliation)

Source: `treasury_transactions` (D1, `openagents-autopilot`), `direction='out'`,
`created_at >= 2026-06-16`. Times UTC.

### 2.1 Totals (all recipients, today)

| State | Rows | Sats |
|---|---|---|
| settled | 29 | **393,739** |
| pending | 3 | 100,005 |
| failed | 6 | 260,000 |

### 2.2 Attribution of the 29 settled rows

Certain (cross-checked to a specific recipient/purpose):

| Time (UTC) | Sats | Attribution |
|---|---|---|
| 21:32:59 | 50,000 | **Trigger** 50k recognition (confirmed received: `detectedBalanceSats: 50000`) |
| 22:06:51 | 1,000 | **Whitefang** canary (confirmed received: `detectedBalanceSats: 1000`) |
| 01:14–01:31 | 500×4 | test wallet on `pylon-gcp-1` (rc.11/rc.12 proof) |
| 03:57:15 | 100 | smoke/test |
| 16:10:13 | 101,000 | buffer refill / internal (treasury→buffer), not a contributor payout |
| 16:12:09 | 5,000 | early test/refill |

Orrery (the 04:15Z–06:17Z debugging pass — split payments to his BOLT12 + Lightning Address):

| Time (UTC) | Sats | Rail |
|---|---|---|
| 04:15:34 / 04:17:32 / 04:18:08 | 250 ×3 | BOLT12 ×2 + LA |
| 04:30:57 | 5,000 | LA |
| 04:33:46 | 20,000 | LA |
| 04:34:12 | 25,000 | LA |
| 04:48:53 | 5,000 | LA |
| 05:03:59 | 5,000 | LA |
| 05:07:57 | 5,000 | LA |
| 05:09:11 | 25,000 | LA |
| 05:25:27 | 5,000 | LA |
| 05:25:57 | 30,000 | LA |
| 05:49:38 | 5,000 | LA |
| 06:00:59 | 5,000 | tips-buffer → LA |
| 06:01:23 | 20,000 | tips-buffer → LA |
| 06:17:10 | 389 | tips-buffer (fractional fallback) |
| 06:17:48 | 1,000 | tips-buffer → LA |
| 06:17:57 | 2,500 | tips-buffer → LA |
| **Subtotal (clearly Orrery)** | **159,639** | |

Ambiguous (settled, tips-buffer, not clearly tied to Orrery in the narrative —
could be Orrery LA payments or buffer movement; **no destination in the table**):

| Time (UTC) | Sats |
|---|---|
| 05:03:47 | 60,000 |
| 05:07:44 | 15,000 |

→ **Orrery settled-sent range: 159,639 (low) to 234,639 (high).** Owed: 50,000.
**Over-send: ~110k–185k sats.**

### 2.3 Failed + pending (did NOT settle)

- **Failed (260,000):** every single large-amount attempt — 50k (04:20), 40k
  (04:32), 50k (05:04), 50k (05:08:14), 40k (05:08:52), 30k (05:09:05). All
  failed **before dispatch**; treasury balance unchanged; no payment id. These
  did not move money (see §3.2).
- **Pending (100,005):** Orrery's original 50,000 (16:21, tips-buffer — the
  BOLT12 dispatch he reported "never settled"), a 50,000 (18:21), and a 5-sat
  (18:04). "Pending" here is unresolved — likely stuck/failed-as-pending; needs
  manual reconciliation (could still be holding 100k of treasury intent).

### 2.4 What Orrery actually RECEIVED

**Unknown / unconfirmed.** Last confirmed receipt-side reading was `0` at 02:17Z,
**before** the split sends. Because Trigger's "settled" treasury row did
correspond to a real recipient-side credit (he saw 50k), the strong prior is that
Orrery's ~160k–235k of *settled* sends **also credited** — i.e. Orrery is
**holding far more than the 50k he is owed**, split across his Spark backup wallet
(LA payments) and possibly his MDK/BOLT12 side (the 250-sat BOLT12 sends). This
must be confirmed by Orrery running `backup-status` (Spark side) and a BOLT12/MDK
balance read, then netted against 50k.

---

## 3. Why the accounting is murky (root causes to fix regardless)

### 3.1 `treasury_transactions` has no destination
The ledger records `direction / amount_sat / state / payment_ref / created_at` but
**not who was paid**. With multiple recipients in one day, per-recipient
reconciliation is impossible from the table alone — we had to infer from amounts +
timestamps + the forum narrative. **Fix:** record a redacted recipient ref
(actor/pylon ref or a destination hash) on each payout row.

### 3.2 "settled" ≠ "recipient-confirmed", and large sends fail pre-dispatch
- A `settled` row means MDK reported the send succeeded; it is not a
  recipient-confirmed receipt. The whole offline-receive saga showed the gap
  (Trigger's "succeeded" 50k was invisible until rc.12 read his Spark balance).
- Single large LA→BOLT11 sends (40k/50k) **fail before dispatch** (`GenericFailure`,
  balance unchanged) while chunks ≤~25–30k settle. The debugging worked around it
  by splitting — which is exactly what produced the over-send. Root cause is
  unproven (single-large-invoice route/liquidity or MDK/Spark/LSP fragility).

### 3.3 Two wallets per agent, two rails per payment
Each agent has an **MDK** wallet (BOLT12 offer, the checkout/MDK spend wallet) and
a **Spark backup** wallet (Lightning Address and Spark spend rail in Pylon). A
payout can land in either depending on which rail resolved. This is no longer a
hard "Spark funds are stuck" problem after #5177, but it is still a product and
accounting split unless the user sweeps Spark into MDK or the UI clearly labels
the two buckets.

---

## 4. Assessment: MDK = checkouts only, unify agent balances on Spark

**Verdict: worth doing, and it directly removes this incident's failure surface.**

### Why Spark for agent balances
- **Proven offline-receive.** Trigger (50k) and Whitefang (1k) both confirmed
  recipient-side via the Spark backup read path. The rail works end-to-end.
- **Self-custodial + deterministic.** The Spark wallet is derived from the node's
  identity mnemonic; same seed → same wallet/address/balance. No separate custody.
- **One balance, one address.** A single Spark balance per agent + a stable
  Lightning Address removes the two-wallet split and the "which wallet did it land
  in" ambiguity that made this audit necessary.
- **Modern Lightning Address receive** is already wired and live.

### Why keep MDK for checkouts
- MDK/the treasury is the established **customer-facing payment-IN + treasury**
  surface (donation invoices, BOLT12 offers, the operator payout origin). It does
  not need to also be the agent's payout *balance*.
- Keeping MDK scoped to checkouts/treasury avoids a risky full rip-out while still
  removing it from the per-agent balance path.

### What this requires (and the open risks)
1. **Spark send/withdraw into Pylon.** #5177 wires `wallet send --rail spark
   --confirm-send` for credited Spark backup funds. BOLT11/Spark payment requests
   use `prepareSendPayment` -> `sendPayment`; Lightning Addresses use `parse` ->
   `prepareLnurlPay` -> `lnurlPay`. Raw destinations stay local and public output
   emits only digest refs, amount/fee, method, and status. #5169 separately wires
   the Spark→MDK sweep path for users who want a single MDK-spendable balance.
2. **Make Spark the single agent balance.** #5178 now makes `pylon wallet status`,
   local control wallet-status, readiness/heartbeat, and the operator snapshot
   report Spark as the one agent-facing balance. MDK is excluded from that public
   agent balance and remains auxiliary for treasury/checkouts/legacy paths.
3. **Solve large-single-payment delivery** (the 40k/50k pre-dispatch failures), or
   formalize chunked payout ≤25–30k as policy. Either way the treasury pays agents
   via their Spark Lightning Address.
4. **Add recipient attribution + a confirmed-receipt step** to the payout ledger so
   "sent" reconciles against "received" automatically (would have prevented the
   over-send going unnoticed).
5. **One-time cleanup:** reconcile every agent's Spark (and residual MDK/BOLT12)
   balance against what they are owed, starting with Orrery's overage.

### Options the owner weighed
The owner chose the Spark-single-wallet end state for agent balances. #5177
removed the immediate Spark-can't-spend gap, and #5178 makes Spark the
agent-facing primary balance. MDK remains available for treasury/checkouts and
legacy recovery, but no longer contributes to the displayed agent balance.

---

## 5. Action items (in priority order)

1. **Confirm Orrery's actual received** (Spark `backup-status` + BOLT12/MDK read),
   net against 50k, and reclaim/adjust the ~110k–185k overage.
2. **Reconcile the 3 pending rows (100,005 sats)** — are they stuck-in-flight,
   refundable, or lost intent?
3. **Use #5177/#5178 to make Spark funds the primary agent balance and spend
   path**; keep #5169 only as an explicit local recovery/compatibility sweep.
4. **Add destination + confirmed-receipt to the payout ledger.**
5. **Fix or formalize large-payment delivery** (chunk ≤25–30k, or solve the
   route/liquidity failure).
6. **Reconcile MDK leftovers out of the agent-balance UX** as part of the MDK
   scope-down work.

> Receipt-first note: this audit treats `settled` treasury rows as *sent*, not
> *received*. No recipient amount here is "confirmed received" except Trigger's 50k
> and Whitefang's 1k. Orrery's figures are **sent**; his received is pending his own
> read.
