# autopilot.all_in_one_business_system.v1 — composed-run receipt reconciliation

Promise: `autopilot.all_in_one_business_system.v1` (state: **planned** — unchanged).

## Blocker advanced (NOT cleared)

`blocker.product_promises.autopilot_business_system_real_business_receipt_missing`

The promise needs a dereferenceable receipt that shows composed usage billed (and,
where revenue applies, settled) from one balance. The composed-run scaffold already
had two halves of this:

- **Plan** (`autopilot-composed-run.ts`) — assembles the composed-run PLAN and a
  receipt envelope whose component refs are each primitive's **surface** receipt ref
  (the one the primitive's public surface advertises, e.g. `fineTuningJobReceiptRef`
  → `receipt.cloud.fine_tuning.job.<id>`).
- **Execution** (`autopilot-composed-run-execution.ts`) — assembles the INERT
  per-component charges, each settling under the cloud-metering **ledger** receipt
  ref (`cloudChargeReceiptRef` → `receipt.cloud.fine_tuning.job.charge.<id>`).

These two ref shapes **never reconciled**: for fine-tuning/sandbox the plan envelope
advertised one ref while the execution settled under a different one, so there was no
single composed-run receipt a reviewer (or a future armed run) could dereference at
both layers.

## What this change adds

`apps/openagents.com/workers/api/src/autopilot-composed-run-receipt.ts` — a PURE,
INERT seam that builds + verifies the ONE composed-run receipt SHAPE from a plan +
its execution:

- binds, per component, the **surface** receipt ref (from the plan envelope) to the
  **settlement** receipt ref (from the execution charge);
- proves the plan and execution describe the SAME components 1:1 by `componentRunId`
  (no plan component dropped, no execution charge orphaned, no duplicates);
- proves the run id / envelope ref are consistent and there are ≥ 2 components;
- reconciles `composedSpendMsat` to the sum of the per-component charges (the "one
  balance" debit reconciles to the components it composes);
- emits a public-safe projection that carries both ref layers but **no amounts,
  idempotency keys, or destinations**, and honestly marks the receipt
  `billed: false`, `settled: false`, `inert: true`.

Tests: `apps/openagents.com/workers/api/src/autopilot-composed-run-receipt.test.ts`
(5 tests) — including a regression that asserts the surface and settlement refs
genuinely differ for fine-tuning and that the receipt binds both.

## Follow-up: real-business-receipt acceptance gate (this run)

`apps/openagents.com/workers/api/src/autopilot-composed-run-receipt-gate.ts` — a
PURE acceptance gate that turns the prose green-flip criteria (which lived only in
module comments and the registry `verification` string) into a TYPED, TESTABLE
predicate. `evaluateRealBusinessReceiptGate(evidence)` reports, per criterion,
whether a composed-run receipt's evidence satisfies the bar, and whether the whole
set would clear
`blocker.product_promises.autopilot_business_system_real_business_receipt_missing`:

- composes ≥ 2 distinct primitives (the all-in-one invariant);
- one shared balance ref;
- composed spend reconciles to the sum of component charges;
- every component charge actually billed (settled against the ledger);
- revenue settled where it applies (or no revenue applies);
- owner sign-off transition receipt recorded (`proof.claim_upgrade_receipts.v1`);
- demand provenance is external market, not internal first-party plumbing
  (`proof.demand_provenance.v1`).

It DECIDES NOTHING IRREVERSIBLE: it flips no promise, drops no blocker, and moves
no money — acting on a `true` result stays an owner-gated step outside the module.
`inertReceiptGateEvidence(receipt)` derives the honest status-quo evidence for the
current inert receipt; the gate returns `clearsBlocker: false` and names the unmet
criteria (components not billed, no owner sign-off, demand not external market).

Tests: `apps/openagents.com/workers/api/src/autopilot-composed-run-receipt-gate.test.ts`
(5 tests) — inert receipt fails honestly, fully-armed evidence clears with no open
ref, revenue-applies-but-unsettled fails, revenue-N/A passes, and a single-primitive
receipt fails the composition criterion.

## What remains (blocker stays listed)

This is the receipt **shape**, reconciled over an INERT execution. The blocker stays
open until a REAL business provisions and runs ≥ 2 composed primitives against one
balance and a dereferenceable receipt shows the composed usage actually **billed**
(and, where revenue applies, **settled**) — with owner sign-off per
`proof.claim_upgrade_receipts.v1` and demand provenance per
`proof.demand_provenance.v1` (internal first-party use is plumbing proof, not market
proof). No promise state was changed; no blocker was dropped.
