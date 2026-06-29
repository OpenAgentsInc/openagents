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

## Follow-up: real-business-receipt evidence manifest (this run)

`apps/openagents.com/workers/api/src/autopilot-composed-run-receipt-manifest.ts` —
a PURE, INERT manifest that turns each acceptance-gate criterion into the concrete,
dereferenceable EVIDENCE a real armed run must produce. The gate decides whether
supplied evidence clears the blocker; it does not say WHERE each piece of evidence
comes from. The manifest closes that gap: per criterion it records the evidence
field(s) the gate reads, the governing ref (an existing proof primitive, e.g.
`proof.claim_upgrade_receipts.v1` / `proof.demand_provenance.v1`, or the real
in-repo seam that emits the artifact, e.g. `cloud/cloud-metering.ts`,
`marketplace-monetize-any-layer-accrual.ts`), the required artifact, and a
human-readable requirement.

It is keyed by the gate's own `RealBusinessReceiptCriterionId` union, so TypeScript
enforces the manifest stays 1:1 with the gate — you cannot add a gate criterion
without adding its evidence requirement (and vice versa). Helpers:
`unmetEvidenceRequirements(result)` maps a gate result's unsatisfied criteria to the
artifacts still owed; `reconcileManifestWithGate(result)` proves alignment at
runtime. It DECIDES NOTHING IRREVERSIBLE: flips no promise, drops no blocker, moves
no money.

Tests: `apps/openagents.com/workers/api/src/autopilot-composed-run-receipt-manifest.test.ts`
(4 tests) — every entry self-keys with non-empty refs, the manifest is 1:1 with the
gate criteria, the inert receipt names exactly the honest unmet artifacts with their
governing proof primitives, and fully-armed evidence owes no outstanding artifact.

## Follow-up: real-business-receipt readiness report (this run)

`apps/openagents.com/workers/api/src/autopilot-composed-run-receipt-readiness.ts` —
a PURE module that produces the ONE reviewer-facing artifact joining the two
upstream halves. The gate answers "does this evidence satisfy each criterion?"
(satisfied + detail); the manifest answers "WHERE does each criterion's evidence
come from / what artifact must a real run produce?" (governingRef +
requiredArtifact). Neither alone is the single thing a reviewer (or a future armed
run) reads to see, in one ordered list: per criterion, whether it currently holds,
why, and — when it does not — exactly which dereferenceable artifact is still owed
and which seam/proof primitive governs it.

`buildRealBusinessReceiptReadinessReport(evidence)` runs the existing gate, joins
each criterion to its manifest requirement, projects a public-safe receipt context
(refs only — no amounts, idempotency keys, or destinations), and reports the
satisfied/total tally, the outstanding artifacts, and the overall verdict. It
INTRODUCES no new pass/fail rule — `clearsBlocker` mirrors the gate exactly. It
DECIDES NOTHING IRREVERSIBLE: flips no promise, drops no blocker, moves no money;
a `true` verdict is a REPORT, not an action. `inertReadinessReport(receipt)`
renders the honest status quo (not billed, no owner sign-off, internal first-party
demand): verdict `clearsBlocker: false`, naming the outstanding artifacts with
their governing proof primitives.

Tests: `apps/openagents.com/workers/api/src/autopilot-composed-run-receipt-readiness.test.ts`
(5 tests) — one line per gate criterion in gate order each joined to its manifest
requirement, public-safe receipt context (no amounts leak), fully-armed evidence
clears with no outstanding artifact, the inert report fails honestly and names the
outstanding artifacts (= the unsatisfied lines), and outstanding artifacts carry
their governing proof primitives.

## Follow-up: real-business-receipt readiness DIGEST (this run)

`apps/openagents.com/workers/api/src/autopilot-composed-run-receipt-readiness-digest.ts`
— a PURE renderer that turns the structured readiness report into the LITERAL
human-readable artifact a reviewer reads. The readiness module describes itself as
"the ONE reviewer-facing artifact", but it emits a structured object, not the
markdown a human reads. `renderRealBusinessReceiptReadinessDigest(report)` renders
one report into a stable, ordered, public-safe markdown digest: a header (run,
blocker, verdict, satisfied/total, billed/settled/inert posture), the public-safe
receipt context (balance, envelope, referral state, surface↔settlement component
refs), one checkbox line per criterion (with the owed artifact + governing ref
when unsatisfied), the outstanding-artifacts list, and the uncleared blockers.

It is PURE PRESENTATION: it reads only the report's existing fields (refs +
booleans + prose), introduces NO new pass/fail rule, and the digest verdict
mirrors `report.clearsBlocker` exactly. The output is deterministic (no
timestamps/randomness) and carries a stable trailing newline, so it is safe to
snapshot or concatenate into a runbook. It DECIDES NOTHING IRREVERSIBLE: flips no
promise, drops no blocker, moves no money.

Tests: `apps/openagents.com/workers/api/src/autopilot-composed-run-receipt-readiness-digest.test.ts`
(4 tests) — the inert digest renders DOES NOT CLEAR and names every unmet
criterion + owed artifact, the armed digest renders CLEARS with no outstanding
artifacts and no unchecked markers, no per-component amounts leak (public-safe),
and the render is deterministic with no trailing whitespace.

## Follow-up: demand-provenance binding for the gate (this run)

`apps/openagents.com/workers/api/src/autopilot-composed-run-receipt-demand-provenance.ts`
— a PURE derivation that resolves the acceptance gate's `demand_provenance_external`
criterion from the REAL `proof.demand_provenance.v1` surface instead of a reviewer
hand-asserting the value. The gate carries a free-union `demandProvenance` evidence
field and the manifest already names `proof.demand_provenance.v1` as its governing
ref, but nothing BOUND the two — a reviewer (or future armed run) could type
`external_market` by hand with no link to the actual provenance projection, exactly
the demand-provenance theater the capstone's real-business-receipt blocker guards
against.

`deriveComposedRunDemandProvenance(signal)` maps the provenance surface's own rule
(`no_external_dollar_no_demand_claim`, surfaced as `externalDemandClaimAllowed`) plus
the accepted-outcome totals to the gate's union: `external_market` when the surface
permits the external-demand claim, `internal_first_party` when only internal
first-party outcomes exist (plumbing proof, not market proof), `unknown` when
nothing is labeled. `demandProvenanceSignalFromProjection(projection)` lifts a live
`DemandProvenanceProjection` into the narrow public-safe signal;
`withDerivedDemandProvenance(evidence, signal)` rebinds gate evidence's
`demandProvenance` from the surface — overwriting any hand-asserted value. It
INTRODUCES no new demand rule (it honors the projection's) and DECIDES NOTHING
IRREVERSIBLE: flips no promise, drops no blocker, moves no money. Against the current
internal-only surface the derivation returns a non-external provenance, so the gate's
external-demand criterion stays UNSATISFIED — the honest status quo, now read from
the governing surface rather than asserted.

Tests: `apps/openagents.com/workers/api/src/autopilot-composed-run-receipt-demand-provenance.test.ts`
(5 tests) — the live internal-only surface derives non-external, external accepted-
outcome demand derives `external_market` and satisfies the criterion, internal-only
derives `internal_first_party`, unlabeled derives `unknown`, and
`withDerivedDemandProvenance` corrects a hand-asserted `external_market` lie back to
the surface truth (gate stays uncleared) while a real-external surface satisfies it.

## What remains (blocker stays listed)

This is the receipt **shape**, reconciled over an INERT execution. The blocker stays
open until a REAL business provisions and runs ≥ 2 composed primitives against one
balance and a dereferenceable receipt shows the composed usage actually **billed**
(and, where revenue applies, **settled**) — with owner sign-off per
`proof.claim_upgrade_receipts.v1` and demand provenance per
`proof.demand_provenance.v1` (internal first-party use is plumbing proof, not market
proof). No promise state was changed; no blocker was dropped.
