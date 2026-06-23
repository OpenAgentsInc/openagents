# DE-2: Cloud Primitive Dereferenceable PAID-Charge Receipt (Registry 2026-06-23.1)

Date: 2026-06-23

Part of EPIC **#5525** (DE-2 Inference Gateway + Agent Cloud primitives), under
the Weekend Promise Assault master EPIC **#5523**.

## Promise advanced

`cloud.sandbox_compute_service.v1` (primary), with the same machinery
benefiting `cloud.fine_tuning_service.v1` and `cloud.primitives_suite.v1`.

This is a copy/evidence destale that closes the **missing dereferenceable
receipt artifact**. It flips **no** promise state and changes no green count.
The sandbox promise STAYS red; it is one real-runtime + real-renter step from
green, and that step is owner/demand-gated.

## Why this promise was the most-ready buildable now

Per the EPIC #5525 table, most DE-2 promises are owner-gated (need real prod
Stripe money, a paid purchase, or an owner flag): `gateway_credits_business`,
`fireworks_open_model_provider`, `referral_on_all_inference`, `hosted_gemini`,
and the `one_stop_revshare` capstone. The agent-claimable primitives
(`sandbox_compute`, `fine_tuning`) already shipped a mature flag-gated INERT
scaffold with a **real receipt-first credit-metering seam** (`cloud-metering.ts`)
that decrements credits through the same atomic PayIn ledger the mature Khala /
inference gateway uses.

Inspection showed the sandbox primitive was exactly one real implementation step
plus a receipt away: the metering *wrote* a ledger fact, but **nothing could
dereference the receipt it advertised**. Two concrete defects blocked it.

## The two real gaps fixed

1. **Charge never settled to `paid`.** `settleCloudPrimitiveCharge` wrote a
   debit-only `adjustment` pay-in and left it `pending` forever. The balance was
   already decremented by the funding `in` leg, but the row never reached the
   `paid` state a receipt projection requires. Fix: mark the charge `paid` in the
   **same atomic batch** (`markPayInPaidStatements` with empty payout legs) —
   the exact discipline the inference metering hook already uses for its
   debit-only charge.

2. **Advertised receipt ref ≠ ledger receipt ref.** The surfaces advertised
   `receipt.cloud.sandbox_compute.rental.<id>` (and `…fine_tuning.job.<id>`),
   but the ledger row's `public_receipt_ref` is
   `receipt.cloud.<primitive>.charge.<id>` (`cloudChargeReceiptRef`). The
   advertised ref could never resolve. Fix: align `sandboxRentalReceiptRef` and
   `fineTuningJobReceiptRef` to `cloudChargeReceiptRef`, so the ref a surface
   advertises is exactly the ref that dereferences.

## What was built

- **`cloud/cloud-primitive-receipts.ts`** — a D1 read store +
  public-safe projection (mirrors `inference-receipts.ts`). It reads the settled
  `pay_ins` row by `public_receipt_ref` and projects a public-safe PAID receipt
  for `receipt.cloud.sandbox_compute.rental.charge.*` and
  `receipt.cloud.fine_tuning.job.charge.*`. Redaction-guarded; only `state =
  'paid'` adjustment charges project; carries a caveat stating demand provenance
  and owner sign-off are still pending (it asserts no promise is green).
- **`cloud/public-cloud-primitive-receipt-routes.ts`** — the public read route
  `GET /api/public/cloud/receipts/:receiptRef`, wired into the worker router,
  `index.ts`, and the OpenAPI contract
  (`PublicCloudPrimitiveReceiptEnvelope`).
- **`cloud/cloud-metering.ts`** — settle-to-`paid` fix (gap 1).
- **`cloud/sandbox-compute-service-routes.ts`,
  `cloud/fine-tuning-service-routes.ts`** — receipt-ref alignment (gap 2).
- **`autopilot-composed-run-receipt.ts`** — updated to reflect that a cloud
  primitive component's surface and settlement refs now coincide.

Both primitive surfaces remain **flag-gated INERT** (default off → 404). On prod
they provision nothing and bill nothing.

## The dereferenceable receipt (the proof)

`cloud/cloud-primitive-receipts.test.ts` exercises the whole loop against
**real SQL** (node:sqlite with the load-bearing ledger constraints):

1. A funded account (10,000 msat) closes a sandbox rental with **real metered
   usage** (300 wall-seconds). The live ledger metering hook prices it from
   usage (not an estimate), debits credits receipt-first, and settles the charge
   to `paid`.
2. The debit actually moved credits: balance 10,000 → 7,000 msat.
3. The receipt the surface advertises
   (`receipt.cloud.sandbox_compute.rental.charge.sbx_proof_1`) is the SAME ref
   the ledger wrote, and **dereferences** through the store + projection into a
   public-safe PAID receipt — proving **rent → metered debit → dereferenced
   receipt**.

This is the receipt acceptance shape DE-2 names for sandbox ("rent → run →
metered → paid receipt") at the metering+receipt layer.

## State after this change

- `cloud.sandbox_compute_service.v1` — **STAYS red.** The receipt artifact and
  the settle-to-paid path now exist and are tested. Remaining blockers: a real
  isolated-session runtime, a live pricing function, and a **real renter**
  (demand provenance). The `cloud_sandbox_paid_receipt_missing` blocker is
  replaced by
  `cloud_sandbox_real_renter_demand_provenance_and_owner_signoff_missing`.
- `cloud.fine_tuning_service.v1` — unchanged red; inherits the same fixed
  receipt machinery for when its runtime lands.
- `cloud.primitives_suite.v1` — unchanged planned.

## What remains for the green flip (owner-gated)

1. Wire a **real** isolated-session sandbox runtime adapter + a live pricing
   function and enable `CLOUD_SANDBOX_COMPUTE_ENABLED` in prod.
2. A **real renter** (not first-party plumbing) rents, runs work, and is
   metered — demand provenance per `proof.demand_provenance.v1`.
3. Record the green-flip transition receipt with **owner sign-off** per
   `proof.claim_upgrade_receipts.v1`.

## Verification

From `apps/openagents.com/workers/api`:

- `bun run test -- src/cloud/` (incl. the new receipt + route suites) green.
- `bun run test -- src/autopilot-composed-run-receipt.test.ts` green (updated
  ref-coincidence assertion).
- `bun run test -- src/product-promises.test.ts` green (registry 2026-06-23.1).
- `typecheck`, `check:architecture`, `check:effect-topology` green.
