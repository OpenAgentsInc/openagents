// Autopilot composed-run EXECUTION composition — the inert seam that turns the
// composed-run PLAN (autopilot-composed-run.ts, #5519) from a shape-demo into a
// real (but inert) composition that exercises the actual metering + referral
// seams the merged primitives own (EPIC #5510; promises
// cloud.primitives_suite.v1, cloud.agent_cloud_one_stop_revshare.v1,
// autopilot.all_in_one_business_system.v1 — all planned).
//
// Episode 239 ("Let's Make Money", docs/transcripts/239.md): a business runs on
// Autopilot, COMPOSED of the OpenAgents Cloud primitives bought from ONE balance
// (USD or Bitcoin), with revshare to the contributor who served the work and the
// referrer who brought the customer. This module assembles ONE composed run that:
//
//   - composes >= 2 REAL primitive scaffolds (inference + one of
//     fine-tuning/sandbox) onto ONE shared balance;
//   - derives EACH component's charge shape from its OWN primitive helper —
//     fine-tuning/sandbox through the merged receipt-first metering seam
//     (cloud/cloud-metering.ts: cloudChargePayInPlan + cloudChargeReceiptRef +
//     cloudChargeIdempotencyKey), inference through the inference metering hook's
//     receipt-ref helper — so the composition speaks the EXACT charge/receipt
//     vocabulary the primitives bill under, never a parallel one;
//   - sums the per-component charges into ONE shared-balance debit total (the
//     "one balance" invariant made concrete: the run debits a single balance for
//     the whole composed spend);
//   - feeds the composed spend through the MERGED referral bridge
//     (marketplace-monetize-any-layer-accrual.ts -> the ONE RL-1 cross-category
//     ledger) to compute the referrer's cut on the composed run.
//
// SCOPE / HONESTY: this is FLAG-GATED INERT. It moves NO money, settles NO
// charge, writes NO receipt, and accrues NO ledger row:
//   - it builds the per-component CloudPrimitiveCharge PLANS but never calls
//     settleCloudPrimitiveCharge (no D1 batch, no balance debit);
//   - it calls the referral bridge with `enabled: false` ALWAYS, so the bridge
//     returns its `disabled` plan and touches no ledger.
// All three capstone promises STAY planned. Nothing here flips one green: a green
// flip needs a REAL billed composed run with a dereferenceable revshare receipt
// and owner sign-off per proof.claim_upgrade_receipts.v1, and demand provenance
// per proof.demand_provenance.v1 (internal first-party use is plumbing proof,
// not market proof).

import {
  cloudChargeIdempotencyKey,
  cloudChargeReceiptRef,
  type CloudPrimitiveCharge,
} from './cloud/cloud-metering'
import {
  FINE_TUNING_PRIMITIVE,
} from './cloud/fine-tuning-service-routes'
import {
  SANDBOX_COMPUTE_PRIMITIVE,
} from './cloud/sandbox-compute-service-routes'
import {
  inferenceChargeIdempotencyKey,
  inferenceChargeReceiptRef,
} from './inference/metering-hook'
import {
  type AccrueMonetizeLayerReferralResult,
  accrueMonetizeLayerReferral,
} from './marketplace-monetize-any-layer-accrual'
import {
  buildLayerMonetizationDefinition,
} from './marketplace-monetize-any-layer'
import type { ReferredPrincipal } from './referral-cross-category-accrual'
import type { ComposedRunBalance, ComposedRunPlan } from './autopilot-composed-run'

export const AUTOPILOT_COMPOSED_RUN_EXECUTION_SCHEMA =
  'openagents.autopilot_composed_run_execution.v1' as const

// The capstone promises this composition exercises. ALL stay planned; the
// composition makes no live-product claim for any of them.
export const COMPOSED_RUN_EXECUTION_PROMISE_IDS = [
  'cloud.primitives_suite.v1',
  'cloud.agent_cloud_one_stop_revshare.v1',
  'autopilot.all_in_one_business_system.v1',
] as const

// The blockers this execution composition documents and does NOT clear: an inert
// composition is neither a billed run nor a settled revshare receipt.
export const COMPOSED_RUN_EXECUTION_NOT_BILLED_REF =
  'blocker.product_promises.autopilot_business_system_unified_billing_unbuilt' as const
export const COMPOSED_RUN_EXECUTION_NO_REVSHARE_RECEIPT_REF =
  'blocker.product_promises.agent_cloud_cross_category_revshare_unbuilt' as const
export const COMPOSED_RUN_EXECUTION_REAL_RECEIPT_MISSING_REF =
  'blocker.product_promises.autopilot_business_system_real_business_receipt_missing' as const

// The primitives this execution seam knows how to derive a REAL charge shape for.
// Inference uses the inference metering hook; fine-tuning/sandbox use the merged
// cloud-metering seam. (Other primitives compose at the PLAN layer but have no
// billed charge shape yet, so the execution seam requires the billable ones.)
const FINE_TUNING_RUN_PRIMITIVE = 'fine_tuning' as const
const SANDBOX_RUN_PRIMITIVE = 'sandbox' as const
const INFERENCE_RUN_PRIMITIVE = 'inference' as const

/**
 * The REAL, receipt-first charge a single composed component would settle under,
 * derived from its OWN primitive helper. INERT: this is the PLAN of the charge,
 * never a settled debit. `chargeMsat` is the receipt-first amount the component's
 * own pricing function reported from real runtime usage (the caller supplies it;
 * the seam never invents a price).
 */
export type ComposedComponentCharge = Readonly<{
  primitive: string
  componentRunId: string
  /** Receipt-first charge in integer msat (>= 0). */
  chargeMsat: number
  /** Public-safe receipt ref, from the owning primitive's helper. */
  receiptRef: string
  /** Public-safe idempotency key, from the owning primitive's helper. */
  idempotencyKey: string
  /**
   * The CloudPrimitiveCharge the merged metering seam (cloud-metering.ts) WOULD
   * settle, for fine-tuning/sandbox components. `null` for inference (the
   * inference gateway owns its own charge plan shape, not the cloud-metering
   * one). Present so a future armed run can hand the exact charge to
   * settleCloudPrimitiveCharge without rebuilding it.
   */
  cloudCharge: CloudPrimitiveCharge | null
}>

const isNonNegativeIntegerMsat = (value: number): boolean =>
  Number.isFinite(value) && Number.isInteger(value) && value >= 0

/**
 * Derive the REAL charge shape for one composed component from its OWN primitive
 * helper. Fine-tuning and sandbox build a full CloudPrimitiveCharge (the merged
 * cloud-metering seam's input shape) so a future armed run settles them through
 * the SAME atomic, idempotent, never-negative ledger the primitives bill under.
 * Inference derives its receipt-ref + idempotency key from the inference metering
 * hook (the gateway owns the actual debit plan). INERT: builds the plan only.
 */
export const composedComponentCharge = (input: {
  primitive: string
  componentRunId: string
  accountRef: string
  chargeMsat: number
  adapterId: string
}): ComposedComponentCharge => {
  switch (input.primitive) {
    case FINE_TUNING_RUN_PRIMITIVE: {
      const cloudCharge: CloudPrimitiveCharge = {
        accountRef: input.accountRef,
        chargeId: input.componentRunId,
        chargeMsat: input.chargeMsat,
        primitive: FINE_TUNING_PRIMITIVE,
        adapterId: input.adapterId,
      }
      return {
        primitive: input.primitive,
        componentRunId: input.componentRunId,
        chargeMsat: input.chargeMsat,
        receiptRef: cloudChargeReceiptRef(
          FINE_TUNING_PRIMITIVE,
          input.componentRunId,
        ),
        idempotencyKey: cloudChargeIdempotencyKey(
          FINE_TUNING_PRIMITIVE,
          input.componentRunId,
        ),
        cloudCharge,
      }
    }
    case SANDBOX_RUN_PRIMITIVE: {
      const cloudCharge: CloudPrimitiveCharge = {
        accountRef: input.accountRef,
        chargeId: input.componentRunId,
        chargeMsat: input.chargeMsat,
        primitive: SANDBOX_COMPUTE_PRIMITIVE,
        adapterId: input.adapterId,
      }
      return {
        primitive: input.primitive,
        componentRunId: input.componentRunId,
        chargeMsat: input.chargeMsat,
        receiptRef: cloudChargeReceiptRef(
          SANDBOX_COMPUTE_PRIMITIVE,
          input.componentRunId,
        ),
        idempotencyKey: cloudChargeIdempotencyKey(
          SANDBOX_COMPUTE_PRIMITIVE,
          input.componentRunId,
        ),
        cloudCharge,
      }
    }
    case INFERENCE_RUN_PRIMITIVE:
      return {
        primitive: input.primitive,
        componentRunId: input.componentRunId,
        chargeMsat: input.chargeMsat,
        receiptRef: inferenceChargeReceiptRef(input.componentRunId),
        idempotencyKey: inferenceChargeIdempotencyKey(input.componentRunId),
        cloudCharge: null,
      }
    default:
      // The execution seam only bills the primitives whose merged scaffolds own a
      // charge shape. A run composing other primitives at the PLAN layer is fine,
      // but it cannot be EXECUTED (billed) here.
      return {
        primitive: input.primitive,
        componentRunId: input.componentRunId,
        chargeMsat: input.chargeMsat,
        receiptRef: `receipt.autopilot.composed_run.component.${input.primitive}.${input.componentRunId}`,
        idempotencyKey: `autopilot.composed_run.${input.primitive}:charge:${input.componentRunId}`,
        cloudCharge: null,
      }
  }
}

const isBillablePrimitive = (primitive: string): boolean =>
  primitive === FINE_TUNING_RUN_PRIMITIVE ||
  primitive === SANDBOX_RUN_PRIMITIVE ||
  primitive === INFERENCE_RUN_PRIMITIVE

/**
 * Raw component input for composing an executable run: the primitive, its own
 * per-component run id, and the receipt-first charge its own pricing function
 * reported from REAL runtime usage (>= 0 integer msat). The seam never invents
 * the price; the caller derives it from real usage per the primitive's own gate.
 */
export type ComposedRunExecutionComponentInput = Readonly<{
  primitive: string
  componentRunId: string
  chargeMsat: number
}>

/**
 * The referral parameters for the composed run's revshare cut. Mirrors the
 * monetize-any-layer offer vocabulary so the composed run feeds the SAME merged
 * bridge -> the ONE RL-1 ledger; the seam never opens a parallel ledger.
 */
export type ComposedRunReferralInput = Readonly<{
  /** Stable per-run referral event id (idempotency anchor for the ledger). */
  eventId: string
  /** Neutral seller ref (the business running the composed run). */
  sellerRef: string
  /** Neutral referrer ref the cut accrues to (must differ from seller). */
  referrerRef: string
  /** Referral cut in basis points of the composed spend. */
  referralBps: number
  /** The paying principal whose PERMANENT attribution decides the real referrer. */
  principal: ReferredPrincipal
}>

export class ComposedRunExecutionError extends Error {
  readonly _tag = 'ComposedRunExecutionError'
  constructor(readonly reason: string) {
    super(reason)
  }
}

/**
 * The composed, INERT execution of a run: the per-component charges (each from
 * its own primitive helper), the ONE shared-balance debit total, and the
 * referral accrual the composed spend WOULD produce through the merged bridge.
 */
export type ComposedRunExecution = Readonly<{
  schema: typeof AUTOPILOT_COMPOSED_RUN_EXECUTION_SCHEMA
  runId: string
  /** All three capstone promises, planned — the run over-claims none. */
  promiseIds: typeof COMPOSED_RUN_EXECUTION_PROMISE_IDS
  promiseState: 'planned'
  /** Always true — moves no money, settles nothing, accrues nothing. */
  inert: true
  /** The ONE balance every component debits. */
  balance: ComposedRunBalance
  /** Per-component receipt-first charge plans, from each primitive's helper. */
  componentCharges: ReadonlyArray<ComposedComponentCharge>
  /** Sum of the per-component charges (msat) — the one shared-balance debit. */
  composedSpendMsat: number
  /**
   * The referral accrual the composed spend WOULD produce, through the MERGED
   * bridge -> the ONE RL-1 ledger. ALWAYS the bridge's `disabled` result here:
   * the composition calls the bridge with `enabled: false`, so it computes the
   * plan and touches NO ledger. (The composed-run flag arms the listing surface;
   * it never arms a real ledger accrual or a real debit.)
   */
  referral: AccrueMonetizeLayerReferralResult
  /** The blockers this inert composition documents and does NOT clear. */
  unclearedBlockerRefs: ReadonlyArray<string>
}>

/**
 * Compose an INERT executable run from a built ComposedRunPlan + per-component
 * charges + referral params. Validating and PURE-ish (the only IO is the
 * referral bridge call, which runs with `enabled: false` and so never touches
 * the ledger). Enforces:
 *   - the plan's components and the supplied charges align 1:1 by componentRunId;
 *   - >= 2 components compose (the all-in-one invariant), and the run includes
 *     inference + at least one of fine-tuning/sandbox (a real billable mix, not
 *     two of the same shape);
 *   - every supplied charge is a non-negative integer msat (receipt-first, never
 *     an estimate);
 *   - the referrer differs from the seller (self-referral is not a referral; the
 *     merged bridge also guards this, but we fail fast with a clear reason).
 *
 * Returns the composed execution (inert) on success. NEVER settles a charge and
 * NEVER accrues a ledger row.
 */
export const composeRunExecution = async (
  db: D1Database,
  input: {
    plan: ComposedRunPlan
    accountRef: string
    components: ReadonlyArray<ComposedRunExecutionComponentInput>
    referral: ComposedRunReferralInput
    adapterId?: string
  },
): Promise<
  | { ok: true; execution: ComposedRunExecution }
  | { ok: false; error: ComposedRunExecutionError }
> => {
  const fail = (
    reason: string,
  ): { ok: false; error: ComposedRunExecutionError } => ({
    ok: false,
    error: new ComposedRunExecutionError(reason),
  })

  if (input.components.length < 2) {
    return fail(
      'an executable composed run must compose at least two primitives on one balance',
    )
  }

  // The plan and the charges must describe the SAME components (by run id), so a
  // composed execution can never bill a component the plan did not include.
  const planRunIds = new Set(
    input.plan.components.map(component => component.componentRunId),
  )
  for (const component of input.components) {
    if (!planRunIds.has(component.componentRunId)) {
      return fail(
        `charge for component ${component.componentRunId} is not in the composed-run plan`,
      )
    }
    if (!isNonNegativeIntegerMsat(component.chargeMsat)) {
      return fail(
        `charge for component ${component.componentRunId} must be a non-negative integer msat`,
      )
    }
  }

  // A real billable mix: inference + at least one of fine-tuning/sandbox. This is
  // the execution-layer expression of "compose >= 2 REAL primitive scaffolds
  // (inference + one of fine-tuning/sandbox) on one balance".
  const primitives = new Set(input.components.map(c => c.primitive))
  if (!primitives.has(INFERENCE_RUN_PRIMITIVE)) {
    return fail('an executable composed run must include the inference primitive')
  }
  if (
    !primitives.has(FINE_TUNING_RUN_PRIMITIVE) &&
    !primitives.has(SANDBOX_RUN_PRIMITIVE)
  ) {
    return fail(
      'an executable composed run must include fine-tuning or sandbox alongside inference',
    )
  }
  for (const primitive of primitives) {
    if (!isBillablePrimitive(primitive)) {
      return fail(
        `primitive ${primitive} has no billable charge shape and cannot be executed`,
      )
    }
  }

  if (input.referral.referrerRef === input.referral.sellerRef) {
    return fail('referrerRef must differ from sellerRef (self-referral is not a referral)')
  }

  const adapterId = input.adapterId ?? 'autopilot.composed_run'

  const componentCharges: ReadonlyArray<ComposedComponentCharge> =
    input.components.map(component =>
      composedComponentCharge({
        primitive: component.primitive,
        componentRunId: component.componentRunId,
        accountRef: input.accountRef,
        chargeMsat: component.chargeMsat,
        adapterId,
      }),
    )

  // ONE shared-balance debit total: the whole composed spend against ONE balance.
  const composedSpendMsat = componentCharges.reduce(
    (sum, charge) => sum + charge.chargeMsat,
    0,
  )

  // Feed the composed spend through the MERGED referral bridge. We build a
  // monetize-any-layer offer over the composed run as the `agentic_work` layer
  // (the only always-authorizable, non-resale kind for a composed business run)
  // and call the bridge with `enabled: false`, so it returns the `disabled`
  // plan and touches NO ledger. INERT by construction.
  const offer = buildLayerMonetizationDefinition({
    offerId: `composed_run:${input.plan.runId}`,
    sellerRef: input.referral.sellerRef,
    layer: 'agentic_work',
    monetizationKind: 'agentic_work',
    unitPriceMsat: composedSpendMsat,
    priceAsset: composedRunAssetToPriceAsset(input.plan.balance.asset),
    referralBps: input.referral.referralBps,
    referrerRef: input.referral.referrerRef,
  })
  if (!offer.ok) {
    return fail(`referral offer invalid: ${offer.error.reason}`)
  }

  const referral = await accrueMonetizeLayerReferral(
    db,
    // ALWAYS inert: the composed-run composition never arms a real ledger accrual.
    { enabled: false },
    {
      definition: offer.definition,
      meteredSpendMsat: composedSpendMsat,
      eventId: input.referral.eventId,
      principal: input.referral.principal,
    },
  )

  return {
    ok: true,
    execution: {
      schema: AUTOPILOT_COMPOSED_RUN_EXECUTION_SCHEMA,
      runId: input.plan.runId,
      promiseIds: COMPOSED_RUN_EXECUTION_PROMISE_IDS,
      promiseState: 'planned',
      inert: true,
      balance: input.plan.balance,
      componentCharges,
      composedSpendMsat,
      referral,
      unclearedBlockerRefs: [
        COMPOSED_RUN_EXECUTION_NOT_BILLED_REF,
        COMPOSED_RUN_EXECUTION_NO_REVSHARE_RECEIPT_REF,
        COMPOSED_RUN_EXECUTION_REAL_RECEIPT_MISSING_REF,
      ],
    },
  }
}

// Map the composed-run balance asset onto the monetize-any-layer price-asset
// vocabulary. The balance speaks credit/bitcoin/usd/free; the offer speaks the
// same set. (`free` carries no revenue to share, so the bridge accrues nothing.)
const composedRunAssetToPriceAsset = (
  asset: ComposedRunBalance['asset'],
): 'bitcoin' | 'credit' | 'usd' | 'free' => asset

/**
 * A public-safe projection of a composed execution: the run id, the shared-
 * balance ref + asset, every component's receipt ref (NO amounts, NO idempotency
 * keys, NO payment material), the referrer's would-be cut state, and the inert/
 * planned posture. Suitable for the read-only listing surface. Honest: an inert
 * composition is neither a billed run nor a settled revshare receipt.
 */
export type ComposedRunExecutionProjection = Readonly<{
  schema: typeof AUTOPILOT_COMPOSED_RUN_EXECUTION_SCHEMA
  runId: string
  promiseIds: typeof COMPOSED_RUN_EXECUTION_PROMISE_IDS
  promiseState: 'planned'
  inert: true
  balanceRef: string
  balanceAsset: ComposedRunBalance['asset']
  /** Component receipt refs only — no amounts, keys, or destinations. */
  componentReceiptRefs: ReadonlyArray<string>
  /** The referral bridge tag for the composed spend (always `disabled` here). */
  referralState: AccrueMonetizeLayerReferralResult['_tag']
  unclearedBlockerRefs: ReadonlyArray<string>
}>

export const composedRunExecutionProjection = (
  execution: ComposedRunExecution,
): ComposedRunExecutionProjection => ({
  schema: execution.schema,
  runId: execution.runId,
  promiseIds: execution.promiseIds,
  promiseState: execution.promiseState,
  inert: execution.inert,
  balanceRef: execution.balance.balanceRef,
  balanceAsset: execution.balance.asset,
  componentReceiptRefs: execution.componentCharges.map(
    charge => charge.receiptRef,
  ),
  referralState: execution.referral._tag,
  unclearedBlockerRefs: execution.unclearedBlockerRefs,
})
