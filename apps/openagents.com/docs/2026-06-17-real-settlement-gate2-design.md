# Real Settlement (Gate 2) — Design + Gated Scaffold

Issue: openagents #5232 — *Flip accepted-work / training settlement from
simulation to REAL Bitcoin (`realBitcoinMoved:true`) — the paid-settlement
promise (Gate 2).*

Status: **DESIGN + SAFE GATED SCAFFOLD ONLY.** No real money moves. The real
branch is inert until an owner sets an explicit, default-OFF flag **and** a
bounded per-payout cap **and** a single allowed recipient/run. With the gate
OFF, behavior is byte-for-byte the current simulation.

This document is the map and the design. The accompanying code adds the
owner-gated switch and tests; it does **not** wire a live Spark container call
into the route. The remaining live-money step is owner-only (see
*What remains for the owner*).

---

## 1. Map of the current settlement / payout path

### 1.1 Where a settlement event/receipt is created for accepted work / training runs

The training-run settlement path is the live one and the subject of this gate.

- **Route:** `POST /api/training/runs/:trainingRunRef/settlement-receipt`
  → `routeRunSettlementReceipt` in
  `workers/api/src/training-run-window-routes.ts` (admin-only via
  `requireAdmin`). It:
  1. Decodes the body as `TassadarRunSettlementRequest`.
  2. Loads the run, the Verified `exact_trace_replay` verification challenge,
     and the window lease.
  3. Calls `buildTassadarRunSettlement(...)` to build the ledger chain.
  4. Writes the chain **directly to the payout ledger store**
     (`createPayoutTargetApproval` → `createPayoutIntent` →
     `createPayoutAttempt` → `createReconciliationEvent` →
     `createPaymentAuthorityReceipt`).
  5. Links the `settlementReceiptRef` onto the run and re-projects the summary.

- **Builder:** `buildTassadarRunSettlement` in
  `workers/api/src/tassadar-run-settlement.ts`. Pure. Validates settleability
  (run active, challenge belongs to run + `Verified` + `exact_trace_replay`,
  lease belongs to run, contributor present, amount ≤ run `spendCapSats` and ≤
  `TassadarRunSettlementHardPerPayoutCapSats` (100_000 sats)). Produces:
  `targetApproval`, `intent`, `attempt`, `reconciliationEvent`,
  `settlementReceipt` (`receiptKind: 'settlement_recorded'`),
  `settlementReceiptRef`, `amountSats`, `contributorRef`.

### 1.2 Where `movementMode` / `realBitcoinMoved` are set

There are two surfaces, and they read each other only through the receipt
projection's `moneyMovement` field:

- **Builder (`tassadar-run-settlement.ts:179-181`):**
  ```ts
  const adapterKind = request.adapterKind ?? 'simulation'
  const moneyMovement =
    adapterKind === 'simulation' ? 'none' : 'treasury_mdk_bounded_spend'
  ```
  `moneyMovement` is stamped into the public projection JSON of the intent,
  attempt, reconciliation event, and the `settlement_recorded` receipt. **The
  default is `simulation` → `moneyMovement: 'none'`.** Note: this builder's
  real branch (`mdk_agent_wallet`) currently writes `'treasury_mdk_bounded_spend'`,
  **not** `'real_bitcoin'`.

- **Public projection (`nexus-pylon-visibility.ts:488-498`):**
  `nexusPylonPublicReceiptDetailFromLedger` derives:
  ```ts
  const movementMode =
    (any of receipt/attempt/event/intent projection.moneyMovement === 'real_bitcoin')
      ? 'real_bitcoin' : 'simulation'
  const realBitcoinMoved =
    movementMode === 'real_bitcoin' &&
    receipt.receiptKind === 'settlement_recorded' &&
    event?.status === 'matched'
  ```
  So a public `realBitcoinMoved: true` requires the *projection* to carry
  `moneyMovement: 'real_bitcoin'` — which today only the **Spark treasury
  adapter** writes (`treasury-payment-spark-payout-adapter.ts:114-125`,
  `publicProjectionJson(...) → moneyMovement: 'real_bitcoin'`). The simulation
  fixture hardcodes `movementMode: 'simulation'`, `realBitcoinMoved: false`
  (`nexus-pylon-visibility.ts:335,351`).

- **Settled-sats read (`training-leaderboards.ts:75-91`):**
  `settledSatsFromPaymentAuthorityReceipt` counts a receipt's `amountSats` only
  when `receiptKind === 'settlement_recorded'` and projection `state ===
  'settled'`. **It does not currently gate on `moneyMovement`** — so today a
  simulation settlement contributes to `settledPayoutSats` totals. The public
  *paid-work* gate that requires real movement is enforced separately (see
  INVARIANTS "Receipt-Backed Public Pylon Paid-Work Totals" and the
  product-promise gate in §1.4).

### 1.3 How the recipient is identified

- Recipient = `lease.pylonRef` (`contributorRef` in the builder). It flows into
  the payout-target approval (`pylonRef`), the receipt projection
  (`contributorRef`), and is the leaderboard grouping key.
- The **payout destination** (the actual Lightning/Spark address) is never in
  the public projection. The Spark adapter resolves it via an injected
  `resolveDestination` callback and only stores a `redactedDestinationRef` /
  `redactedPaymentRef`. The builder pre-computes `redactedDestinationRef =
  destination.redacted.tassadar_run_settlement.<suffix>`.

### 1.4 How it ties to the product-promise gate

`workers/api/src/product-promises.ts` holds the registry. The relevant
promises (`training.monday_decentralized_training_launch.v1`, the install→earn
promise) explicitly accept the current Orrery receipt
(`receipt.nexus.tassadar_run_settlement.idem.tassadar.settlement.59ba1f30.orrery.v2`)
as **simulation-backed only** (`realBitcoinMoved:false`) and state, verbatim:

> Real paid-settlement copy requires a linked receipt with
> `realBitcoinMoved:true` and no private payment material.

So the promise is deliberately blocked on `realBitcoinMoved:true`. The flip is
**receipt-first**: a real receipt must exist and be dereferenceable before any
promise copy/`acceptanceProof` is edited to cite it.

### 1.5 The proven real rail (the enabler)

`workers/api/src/treasury-payment-spark-payout-adapter.ts` —
`makeSparkTreasuryPayoutAdapter`. Implements the
`TreasuryPaymentAuthorityAdapter` interface (`dispatch` / `preview` /
`reconcile`). On `dispatch` it POSTs `/spark/pay` to the treasury container with
`{ amountSat, destination, idempotencyKey: attempt.idempotencyKeyHash }`,
requires `status === 'succeeded'`, stamps `moneyMovement: 'real_bitcoin'`,
`rawMaterialStored: false`, and stores only redacted refs. Proven for real
sends this week (#5176/#5185/#5196/#5208, warm session #5207).

The orchestration that gives no-double-pay safety is
`makeTreasuryPaymentAuthority` (`treasury-payment-authority.ts`):
- `createPayoutIntent` rejects a replayed `idempotencyKeyHash`
  (`replayed_idempotency_key`).
- `dispatchPayout` reads `readPayoutAttemptByIdempotencyKeyHash` first and
  returns the **existing** attempt without re-dispatching — the deterministic
  idempotency key → SDK dedupe pattern from #5185.
- Policy gates: pause states, wallet readiness, accepted-work refs, payout
  target approval, spend cap, large-payout approval.

**The gap (why "no sats move" today):** `routeRunSettlementReceipt` never calls
`TreasuryPaymentAuthority`/the Spark adapter. It writes the builder's records
straight to the ledger. Even with `adapterKind: 'mdk_agent_wallet'`, the builder
stamps `treasury_mdk_bounded_spend` (not `real_bitcoin`) and dispatches nothing.

---

## 2. Design for real settlement

### 2.1 Rail choice

Use the **treasury Spark payout adapter** (`spark_treasury`,
`makeSparkTreasuryPayoutAdapter`) driven through `TreasuryPaymentAuthority`.
Rationale: it is the rail proven with real sats this week, it already
implements idempotency-keyed dispatch, redaction, and the
`moneyMovement: 'real_bitcoin'` projection that the public `realBitcoinMoved`
derivation keys on. Reuse, do not rebuild.

### 2.2 Settlement → real payout mapping

1. Build the settlement chain via `buildTassadarRunSettlement` with the
   **resolved** adapter kind (`spark_treasury` when the gate authorizes it,
   else `simulation`). The builder must, in the real branch, stamp
   `moneyMovement: 'real_bitcoin'` so the projection and `realBitcoinMoved`
   derivation are correct end-to-end. (Scaffold change in §3.)
2. Real branch only: drive `TreasuryPaymentAuthority.createPayoutIntent` →
   `dispatchPayout` (Spark adapter) → `reconcilePayout`, then persist the
   resulting `settlement_recorded` receipt that carries the adapter's
   `moneyMovement: 'real_bitcoin'`, redacted payment ref, and `state: 'settled'`.
3. Simulation branch: unchanged — write the builder's records directly to the
   ledger (no authority, no dispatch).

### 2.3 Idempotency (no double-pay)

- Key = the settlement `idempotencyRef` (derived from work/run id + challenge +
  contributor), surfaced as `attempt.idempotencyKeyHash`
  (`hash.tassadar_run_settlement.attempt.<suffix>`).
- Two layers, both already present:
  - `TreasuryPaymentAuthority.createPayoutIntent` rejects a duplicate intent.
  - `TreasuryPaymentAuthority.dispatchPayout` returns the existing attempt for a
    duplicate `idempotencyKeyHash` **without re-dispatching to Spark**.
  - The Spark container itself dedupes on `idempotencyKey`.
- A retry of the whole route therefore re-finds the existing attempt/receipt and
  pays at most once. The gated scaffold proves this with a fake dispatcher and a
  call-count assertion.

### 2.4 Real receipt shape (public-safe refs only)

`settlement_recorded` receipt, projection JSON:
```json
{
  "adapter": "spark_treasury",
  "amountSats": <int>,
  "asset": "bitcoin",
  "contributorRef": "pylon.contributor.<id>",
  "moneyMovement": "real_bitcoin",
  "rawMaterialStored": false,
  "state": "settled",
  "trainingRunRef": "run.tassadar.executor.<date>",
  "verificationChallengeRef": "challenge.<id>",
  "windowRef": "training.window.<id>"
}
```
Public detail (`nexusPylonPublicReceiptDetailFromLedger`) then yields
`movementMode: 'real_bitcoin'`, `realBitcoinMoved: true` (given matched event),
`caveatRefs: ['caveat.public.nexus_pylon.real_bitcoin_receipt', ...]`. **Never**
include raw invoice/preimage/payment-hash/address/mnemonic/amount-beyond-policy.
`assertNexusPylonPublicSafe` and the adapter's `unsafeRefPattern` enforce this.

### 2.5 Reconciliation back to the work/run

- The receipt's `metadataRefs` and projection carry `trainingRunRef`,
  `verificationChallengeRef`, `windowRef`, `contributorRef` (all public-safe
  refs). The route links `settlementReceiptRef` onto the run via
  `appendTrainingRunReceiptRefs`, and `resolveRunSettlements` reads it back.
- Recipient-confirmed credit is a **separate** observation (the contributor
  wallet confirms receive). The receipt asserts dispatch+reconcile; wallet
  landing can still be in-flight (covered by the Spark backup-receive path).
  Public copy must not claim "landed in wallet" from the receipt alone.

### 2.6 Owner gate + bounded rollout

New module `tassadar-run-settlement-gate.ts`:
- `TassadarRealSettlementGate` schema, all bounding fields explicit.
- `readTassadarRealSettlementGate(env)` parses env via the typed boundary
  (no raw `JSON.parse`); **absent/unset/malformed → disabled (fail closed).**
- `resolveTassadarSettlementAdapter({ gate, request, contributorRef })` returns
  `'simulation'` unless **all** hold:
  - `gate.enabled === true` (env flag `OPENAGENTS_REAL_SETTLEMENT_ENABLED`,
    default OFF),
  - `request.amountSats <= gate.maxPayoutSats` (bounded per-payout cap),
  - `contributorRef` ∈ `gate.allowedContributorRefs` (one recipient),
  - `request.trainingRunRef`-equivalent ∈ `gate.allowedRunRefs` (one workload),
  - `gate.allowedAdapterKind === request.adapterKind` (must explicitly ask for
    the real adapter).
  Otherwise → `'simulation'` (so even an admin passing `spark_treasury` with
  the gate OFF still simulates).
- Hard ceiling: `gate.maxPayoutSats` is itself clamped ≤
  `TassadarRunSettlementHardPerPayoutCapSats` (100_000) and ≤ the run
  `spendCapSats`; the builder re-checks the amount against both independently.

### 2.7 Receipt-first promise-flip sequence (owner-only, not done here)

1. Owner sets the gate env (flag + cap + one recipient + one run).
2. Owner triggers a single small real settlement; the route dispatches through
   Spark; a `realBitcoinMoved:true` receipt is written and dereferenceable.
3. Contributor confirms the credit.
4. **Only then** the owner edits the product-promise `acceptanceProof` to cite
   the new real receipt ref, per the registry's `realBitcoinMoved:true` rule.
5. Disable the gate again after the bounded proof unless continuing rollout.

---

## 3. Gated scaffold (what the code change does)

- **New:** `workers/api/src/tassadar-run-settlement-gate.ts` — the typed owner
  gate, env parse (fail-closed), and `resolveTassadarSettlementAdapter`.
- **Change:** `tassadar-run-settlement.ts` — the real branch stamps
  `moneyMovement: 'real_bitcoin'` (so the public derivation is correct) when the
  adapter is `spark_treasury`; `mdk_agent_wallet` keeps its existing
  `treasury_mdk_bounded_spend` label; `simulation` stays `'none'`. Adds a pure
  `realSettlementMovementMode(adapterKind)` helper. No behavior change for the
  default `simulation` path.
- **New:** `workers/api/src/tassadar-run-settlement-gate.test.ts` — proves:
  (a) gate absent/OFF → `simulation`, builder yields `moneyMovement:'none'`,
  `realBitcoinMoved` derives false, and (with a fake dispatcher) no payout call;
  (b) gate ON + within cap + allowlisted → real adapter, a mocked dispatch
  yields a `realBitcoinMoved:true` receipt; (c) a retry through the authority
  dedupe path makes **one** dispatch; (d) amount over the gate cap →
  `simulation` (fail-closed), no payout call.

The route (`routeRunSettlementReceipt`) is **left unchanged** in this scaffold:
wiring the live Spark container call into the admin route is the owner step and
must be done with the gate, the cap, and a live smoke. The scaffold provides the
decision function and the corrected receipt shape so that wiring is a small,
reviewed change rather than a redesign.

---

## 4. Failure modes

| Failure | Handling |
| --- | --- |
| Gate unset / malformed env | Fail closed → `simulation`. No real branch reachable. |
| Admin passes `spark_treasury` with gate OFF | `resolveTassadarSettlementAdapter` returns `simulation`; byte-for-byte sim. |
| Amount over gate cap or run `spendCapSats` | `simulation` (resolver) + builder re-rejects over hard cap; no dispatch. |
| Recipient/run not allowlisted | `simulation`. |
| Dispatch fails after intent created | Intent persists `approved`, no `settlement_recorded` receipt → not counted as settled; safe to retry (idempotent). |
| Payout succeeds but receipt write fails | Spark dedupes on `idempotencyKey`; retry re-finds the existing attempt (no second pay) and re-writes the receipt. |
| Duplicate route call (retry) | `createPayoutIntent` rejects replay / `dispatchPayout` returns existing attempt → at most one pay. |
| Wallet landing in-flight | Receipt asserts dispatch+reconcile only; public copy must not claim "in wallet". |
| Partial / wrong amount | Amount is fixed from the request and capped two ways; Spark dispatches the exact `amountSat`; reconciliation must be `matched` before `realBitcoinMoved`. |

---

## 5. Invariants touched / honored

- **Receipt-Backed Public Pylon Paid-Work Totals** — real receipts must prove
  real movement; simulation receipts must not count. This design keeps
  `moneyMovement:'real_bitcoin'` as the only path to `realBitcoinMoved:true`.
- **MDK Payout Mode Declaration / MDK Agent-Wallet Send Readiness** — real
  dispatch goes through `TreasuryPaymentAuthority` wallet-readiness +
  pause + cap gates; the gate adds an owner flag on top.
- **User-Facing Live Data Integrity** — no copy flip here; the flip is
  owner-only and receipt-first.
- No invariant is weakened. The gate only **adds** a default-OFF condition on
  top of every existing safety check.

---

## 6. What remains for the owner (live money — NOT done here)

1. Set `OPENAGENTS_REAL_SETTLEMENT_ENABLED=true` plus the bounded gate JSON
   (one `allowedContributorRefs`, one `allowedRunRefs`, `maxPayoutSats` small,
   `allowedAdapterKind:'spark_treasury'`).
2. Wire `routeRunSettlementReceipt` to drive `TreasuryPaymentAuthority` +
   `makeSparkTreasuryPayoutAdapter` (with the treasury container fetch +
   `resolveDestination`) on the resolved real branch.
3. Run one small real settlement; verify a dereferenceable
   `realBitcoinMoved:true` receipt and contributor-confirmed credit.
4. Flip the product-promise `acceptanceProof` to cite that receipt
   (receipt-first), then disable the gate.
