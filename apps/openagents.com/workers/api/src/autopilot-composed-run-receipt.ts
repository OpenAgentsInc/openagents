// Autopilot composed-run RECEIPT reconciliation — the inert seam that binds a
// composed-run PLAN's advertised receipt envelope (autopilot-composed-run.ts,
// #5519) to the composed-run EXECUTION's actual per-component charge receipt refs
// (autopilot-composed-run-execution.ts), producing ONE dereferenceable composed-
// run receipt SHAPE (EPIC #5510; promises cloud.primitives_suite.v1,
// cloud.agent_cloud_one_stop_revshare.v1, autopilot.all_in_one_business_system.v1
// — all planned).
//
// Episode 239 ("Let's Make Money", docs/transcripts/239.md): a business runs on
// Autopilot, composed of OpenAgents Cloud primitives bought from ONE balance,
// and the run produces ONE receipt that shows the composed usage billed. For that
// receipt to be REAL it must DEREFERENCE the actual charges. Each component is
// named at TWO layers:
//
//   - the PLAN's receipt envelope advertises each component's SURFACE receipt ref
//     (the one the owning primitive's public surface advertises, e.g.
//     fineTuningJobReceiptRef);
//   - the EXECUTION settles each component under the cloud-metering LEDGER receipt
//     ref (cloudChargeReceiptRef -> `receipt.cloud.<primitive>.charge.<id>`),
//     which is the ref the atomic credit ledger actually writes.
//
// The cloud primitives' surface refs are now ALIGNED to the ledger ref (the
// surface advertises exactly the ref `GET /api/public/cloud/receipts/:ref`
// dereferences), so a component's surface and settlement refs coincide. This
// module still binds BOTH explicitly, proves the plan and execution describe the
// SAME components, and proves the composed spend total reconciles to the sum of
// the per-component charges — so the binding stays correct even if a future
// primitive reintroduces distinct surface/settlement ref shapes.
//
// SCOPE / HONESTY: this is PURE and INERT. It moves no money, settles no charge,
// writes no receipt row, and reads no balance. It only assembles + verifies the
// composed-run receipt SHAPE from an already-inert execution. The receipt it
// builds is honestly marked `billed: false`, `settled: false`, `inert: true`: it
// is the receipt's SHAPE, NOT a settled receipt. All three capstone promises STAY
// planned — nothing here flips one green. A green flip needs a REAL billed
// composed run whose receipt dereferences settled charges, with owner sign-off
// per proof.claim_upgrade_receipts.v1 and demand provenance per
// proof.demand_provenance.v1 (internal first-party use is plumbing proof, not
// market proof).

import type { ComposedRunPlan } from './autopilot-composed-run'
import type { ComposedRunExecution } from './autopilot-composed-run-execution'

export const AUTOPILOT_COMPOSED_RUN_RECEIPT_SCHEMA =
  'openagents.autopilot_composed_run_receipt.v1' as const

// The capstone promises this receipt shape sits under. ALL stay planned; the
// receipt makes no live/billed-product claim for any of them.
export const COMPOSED_RUN_RECEIPT_PROMISE_IDS = [
  'cloud.primitives_suite.v1',
  'cloud.agent_cloud_one_stop_revshare.v1',
  'autopilot.all_in_one_business_system.v1',
] as const

// The blocker this receipt shape advances and does NOT clear: a reconciled,
// dereferenceable receipt SHAPE over an INERT execution is not a real-business
// receipt for a REAL billed composed run.
export const COMPOSED_RUN_RECEIPT_REAL_RECEIPT_MISSING_REF =
  'blocker.product_promises.autopilot_business_system_real_business_receipt_missing' as const

/**
 * One component's receipt binding inside the composed-run receipt: the primitive,
 * its per-component run id, the SURFACE receipt ref the owning primitive's public
 * surface advertises (from the PLAN envelope), the SETTLEMENT receipt ref the
 * cloud-metering ledger / inference gateway actually writes (from the EXECUTION),
 * and the receipt-first charge in integer msat. Public-safe: refs only, never an
 * amount destination, idempotency key, or payment material beyond the msat the
 * execution already carries.
 */
export type ComposedRunReceiptComponent = Readonly<{
  primitive: string
  componentRunId: string
  /** Public surface receipt ref the owning primitive advertises (from the plan). */
  surfaceReceiptRef: string
  /** Ledger/gateway receipt ref the charge settles under (from the execution). */
  settlementReceiptRef: string
  /** Receipt-first charge in integer msat (>= 0). */
  chargeMsat: number
}>

/**
 * The ONE dereferenceable composed-run receipt SHAPE: the run id, the shared
 * balance, the per-component receipt bindings (surface <-> settlement), the
 * reconciled composed spend total, the referral bridge state, and an HONEST
 * billed/settled posture. INERT: `billed`/`settled` are ALWAYS false here — this
 * is the receipt's shape over an inert execution, not a settled receipt.
 */
export type ComposedRunReceipt = Readonly<{
  schema: typeof AUTOPILOT_COMPOSED_RUN_RECEIPT_SCHEMA
  runId: string
  /** All three capstone promises, planned — the receipt over-claims none. */
  promiseIds: typeof COMPOSED_RUN_RECEIPT_PROMISE_IDS
  promiseState: 'planned'
  /** Always true — the receipt is a shape over an inert execution. */
  inert: true
  /** The ONE balance the whole composed spend debits. */
  balanceRef: string
  balanceAsset: ComposedRunExecution['balance']['asset']
  /** The envelope ref the whole run settles under (from the plan). */
  envelopeRef: string
  /** Per-component receipt bindings, surface <-> settlement. */
  components: ReadonlyArray<ComposedRunReceiptComponent>
  /** Sum of the per-component charges (msat) — the one shared-balance debit. */
  composedSpendMsat: number
  /** The referral bridge tag for the composed spend (always `disabled` here). */
  referralState: ComposedRunExecution['referral']['_tag']
  /** Always false — INERT: no charge has been settled against the ledger. */
  billed: false
  /** Always false — INERT: no revshare has settled. */
  settled: false
  /** The blocker this receipt shape advances and does NOT clear. */
  unclearedBlockerRefs: ReadonlyArray<string>
}>

export class ComposedRunReceiptError extends Error {
  readonly _tag = 'ComposedRunReceiptError'
  constructor(readonly reason: string) {
    super(reason)
  }
}

const isNonNegativeIntegerMsat = (value: number): boolean =>
  Number.isFinite(value) && Number.isInteger(value) && value >= 0

/**
 * Build + verify the ONE composed-run receipt from a PLAN and its INERT
 * EXECUTION. PURE and validating. Reconciles:
 *   - the plan and execution describe the SAME components (1:1 by componentRunId,
 *     no plan component missing from the execution, no execution charge absent
 *     from the plan);
 *   - the execution's envelope/run id matches the plan's;
 *   - each component binds a non-empty SURFACE ref (from the plan envelope) AND a
 *     non-empty SETTLEMENT ref (from the execution charge);
 *   - every per-component charge is a non-negative integer msat;
 *   - the reported composedSpendMsat equals the sum of the per-component charges
 *     (the "one balance" debit reconciles to the components it composes);
 *   - the receipt spans >= 2 components (the all-in-one invariant).
 *
 * Returns the composed-run receipt (inert, honestly unbilled/unsettled) on
 * success. NEVER settles a charge and NEVER writes a receipt row.
 */
export const buildComposedRunReceipt = (input: {
  plan: ComposedRunPlan
  execution: ComposedRunExecution
}):
  | { ok: true; receipt: ComposedRunReceipt }
  | { ok: false; error: ComposedRunReceiptError } => {
  const { plan, execution } = input
  const fail = (
    reason: string,
  ): { ok: false; error: ComposedRunReceiptError } => ({
    ok: false,
    error: new ComposedRunReceiptError(reason),
  })

  if (plan.runId !== execution.runId) {
    return fail(
      `plan runId ${plan.runId} does not match execution runId ${execution.runId}`,
    )
  }

  if (execution.componentCharges.length < 2) {
    return fail(
      'a composed-run receipt must compose at least two component charges on one balance',
    )
  }

  // The SURFACE receipt ref per component, keyed by run id, from the PLAN. This is
  // the ref the owning primitive's public surface advertises.
  const surfaceRefByRunId = new Map<string, string>(
    plan.components.map(component => [
      component.componentRunId,
      component.componentReceiptRef,
    ]),
  )

  const chargeRunIds = new Set<string>()
  const components: ComposedRunReceiptComponent[] = []
  let summedSpendMsat = 0

  for (const charge of execution.componentCharges) {
    if (chargeRunIds.has(charge.componentRunId)) {
      return fail(
        `component ${charge.componentRunId} appears more than once in the execution`,
      )
    }
    chargeRunIds.add(charge.componentRunId)

    const surfaceReceiptRef = surfaceRefByRunId.get(charge.componentRunId)
    if (surfaceReceiptRef === undefined) {
      return fail(
        `execution charge for component ${charge.componentRunId} has no matching plan component`,
      )
    }
    if (surfaceReceiptRef.trim().length === 0) {
      return fail(
        `plan component ${charge.componentRunId} has an empty surface receipt ref`,
      )
    }
    if (charge.receiptRef.trim().length === 0) {
      return fail(
        `execution charge for component ${charge.componentRunId} has an empty settlement receipt ref`,
      )
    }
    if (!isNonNegativeIntegerMsat(charge.chargeMsat)) {
      return fail(
        `charge for component ${charge.componentRunId} must be a non-negative integer msat`,
      )
    }

    summedSpendMsat += charge.chargeMsat
    components.push({
      primitive: charge.primitive,
      componentRunId: charge.componentRunId,
      surfaceReceiptRef,
      settlementReceiptRef: charge.receiptRef,
      chargeMsat: charge.chargeMsat,
    })
  }

  // Every plan component must be covered by an execution charge (no silent drop).
  for (const component of plan.components) {
    if (!chargeRunIds.has(component.componentRunId)) {
      return fail(
        `plan component ${component.componentRunId} has no matching execution charge`,
      )
    }
  }

  // The one shared-balance debit must reconcile to the components it composes.
  if (summedSpendMsat !== execution.composedSpendMsat) {
    return fail(
      `composedSpendMsat ${execution.composedSpendMsat} does not equal the sum of component charges ${summedSpendMsat}`,
    )
  }

  return {
    ok: true,
    receipt: {
      schema: AUTOPILOT_COMPOSED_RUN_RECEIPT_SCHEMA,
      runId: plan.runId,
      promiseIds: COMPOSED_RUN_RECEIPT_PROMISE_IDS,
      promiseState: 'planned',
      inert: true,
      balanceRef: execution.balance.balanceRef,
      balanceAsset: execution.balance.asset,
      envelopeRef: plan.receiptEnvelope.envelopeRef,
      components,
      composedSpendMsat: execution.composedSpendMsat,
      referralState: execution.referral._tag,
      billed: false,
      settled: false,
      unclearedBlockerRefs: [COMPOSED_RUN_RECEIPT_REAL_RECEIPT_MISSING_REF],
    },
  }
}

/**
 * A public-safe projection of a composed-run receipt: the run id, the shared-
 * balance ref + asset, the envelope ref, every component's surface + settlement
 * receipt refs (NO amounts, NO idempotency keys, NO destinations), the referral
 * state, and the inert/unbilled/unsettled posture. Suitable for a read-only
 * surface. Honest: a reconciled receipt SHAPE over an inert execution is not a
 * real-business receipt.
 */
export type ComposedRunReceiptProjection = Readonly<{
  schema: typeof AUTOPILOT_COMPOSED_RUN_RECEIPT_SCHEMA
  runId: string
  promiseIds: typeof COMPOSED_RUN_RECEIPT_PROMISE_IDS
  promiseState: 'planned'
  inert: true
  balanceRef: string
  balanceAsset: ComposedRunExecution['balance']['asset']
  envelopeRef: string
  /** Surface <-> settlement receipt refs per component — no amounts or keys. */
  componentReceiptRefs: ReadonlyArray<
    Readonly<{
      primitive: string
      surfaceReceiptRef: string
      settlementReceiptRef: string
    }>
  >
  referralState: ComposedRunExecution['referral']['_tag']
  billed: false
  settled: false
  unclearedBlockerRefs: ReadonlyArray<string>
}>

export const composedRunReceiptProjection = (
  receipt: ComposedRunReceipt,
): ComposedRunReceiptProjection => ({
  schema: receipt.schema,
  runId: receipt.runId,
  promiseIds: receipt.promiseIds,
  promiseState: receipt.promiseState,
  inert: receipt.inert,
  balanceRef: receipt.balanceRef,
  balanceAsset: receipt.balanceAsset,
  envelopeRef: receipt.envelopeRef,
  componentReceiptRefs: receipt.components.map(component => ({
    primitive: component.primitive,
    surfaceReceiptRef: component.surfaceReceiptRef,
    settlementReceiptRef: component.settlementReceiptRef,
  })),
  referralState: receipt.referralState,
  billed: receipt.billed,
  settled: receipt.settled,
  unclearedBlockerRefs: receipt.unclearedBlockerRefs,
})
