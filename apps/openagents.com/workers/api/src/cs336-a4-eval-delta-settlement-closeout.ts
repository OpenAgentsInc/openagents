import {
  assertCs336A4EvalDeltaDecontamination,
  type Cs336A4EvalDeltaDecontaminationReceipt,
} from './cs336-a4-eval-delta-decontamination'
import {
  settleCs336A4EvalDeltaPayment,
  type Cs336A4EvalDeltaFundingParameters,
  type Cs336A4EvalDeltaMeasurement,
  type Cs336A4EvalDeltaSettlement,
} from './cs336-a4-eval-delta-payment'
import { assertCs336A4EvalDeltaMeasurementBinding } from './cs336-a4-eval-delta-measurement-binding'
import {
  buildCs336A4EvalDeltaSettlementReceipt,
  type Cs336A4EvalDeltaSettlementReceipt,
} from './cs336-a4-eval-delta-settlement-receipt'
import type { Cs336A4ProvenanceReceipt } from './cs336-a4-provenance'

/**
 * Eval-delta settlement closeout for CS336 A4 refinery shards
 * (`blocker.product_promises.eval_delta_payment_missing`).
 *
 * Three deterministic gates already exist but nothing composes them, so the
 * one path that actually records a bonus
 * (`buildCs336A4EvalDeltaSettlementReceipt`) leaves two doors open:
 *
 *  1. The settlement-receipt builder enforces that a settlement and a
 *     provenance receipt name the same `assignmentRef` and that a payable
 *     bonus is bound to a recompute-verified receipt — but it never enforces
 *     that the eval delta was measured on the shard's REAL corpus source.
 *     `verifyCs336A4EvalDeltaMeasurementBinding` answers exactly that question,
 *     yet nothing on the settlement path calls it.
 *  2. `settleCs336A4EvalDeltaPayment` takes a free `stageRecomputeVerified`
 *     flag from the caller, which can disagree with the bound provenance
 *     receipt's own `recomputeVerified`. A contributor could claim the stage
 *     verified (earning a priced bonus) while binding it to a provenance
 *     receipt that says it did not.
 *
 * This module is the fail-closed composition that closes both doors. It is the
 * single entry point a settlement/closeout path should call, and it derives
 * every authority field from the provenance receipt rather than trusting a
 * loose caller-supplied copy:
 *
 *  - the `assignmentRef` settled and recorded is the provenance receipt's, so
 *    the settlement and the receipt cannot name different shards;
 *  - `stageRecomputeVerified` is the provenance receipt's `recomputeVerified`,
 *    so the priced settlement cannot claim a verification the bound receipt
 *    denies;
 *  - BEFORE pricing, the measurement is asserted to bind the receipt's source
 *    (`assertCs336A4EvalDeltaMeasurementBinding`), so a positive delta measured
 *    on an unrelated source can never be priced or recorded against this shard;
 *  - BEFORE pricing, a CLEAN decontamination receipt must cover the
 *    measurement's source AND held-out eval set
 *    (`assertCs336A4EvalDeltaDecontamination`), so a positive delta produced by
 *    leaking held-out eval examples into the "filtered" corpus (memorisation,
 *    not data quality) can never be priced or recorded against this shard. The
 *    decontamination receipt is REQUIRED — there is no settlement path that
 *    prices a bonus without one.
 *
 * It fabricates no eval score, sets no funding, and emits refs/digests/sats
 * only — never wallet, invoice, preimage, or private material (the underlying
 * builders' public-safety guards still apply).
 */

export type Cs336A4EvalDeltaSettlementCloseout = Readonly<{
  /** The shard's corpus source the measurement was confirmed to bind. */
  boundSourceRef: string
  /**
   * The content-addressed ref of the clean decontamination receipt that
   * cleared this bonus against the measurement's held-out eval set.
   */
  decontaminationReceiptRef: string
  /** The priced settlement decision (payable or blocked). */
  settlement: Cs336A4EvalDeltaSettlement
  /** The content-addressed bonus receipt binding settlement to provenance. */
  settlementReceipt: Cs336A4EvalDeltaSettlementReceipt
}>

/**
 * Composes the eval-delta measurement-binding, pricing, and settlement-receipt
 * gates into one deterministic, fail-closed closeout for a single refinery
 * shard. Fails closed (re-raising the underlying typed error) when the
 * measurement was not taken on the shard's admitted source, when pricing's
 * preconditions reject, or when the receipt builder rejects an assignment /
 * recompute / public-safety violation.
 *
 * Derives `assignmentRef` and `stageRecomputeVerified` from the trusted
 * provenance receipt so the settlement and the recorded receipt cannot drift
 * from the shard they pay for. The caller supplies the measurement, the
 * required decontamination receipt, and the optional operator funding
 * parameters; everything else is bound to the provenance the shard was admitted
 * under.
 */
export const closeCs336A4EvalDeltaSettlement = async (
  input: Readonly<{
    /** REQUIRED clean-decontamination evidence; no bonus is priced without it. */
    decontaminationReceipt: Cs336A4EvalDeltaDecontaminationReceipt
    measurement: Cs336A4EvalDeltaMeasurement
    provenanceReceipt: Cs336A4ProvenanceReceipt
    /** Unset until operator funding is approved; absent => blocked settlement. */
    fundingParameters?: Cs336A4EvalDeltaFundingParameters
  }>,
): Promise<Cs336A4EvalDeltaSettlementCloseout> => {
  const { decontaminationReceipt, fundingParameters, measurement, provenanceReceipt } =
    input

  // Gate 1: the eval delta must have been measured on the shard's admitted
  // source. Asserted BEFORE pricing so a wrong-source delta can never be
  // priced or recorded, even if it would otherwise be payable.
  assertCs336A4EvalDeltaMeasurementBinding({ measurement, provenanceReceipt })

  // Gate 2: the shard's corpus must be decontaminated against the measurement's
  // held-out eval set under a CLEAN receipt. Asserted BEFORE pricing so a delta
  // inflated by eval leakage (memorisation, not data quality) can never be
  // priced or recorded — even though every assignment/source/recompute check
  // above would otherwise pass. Required: there is no path that prices a bonus
  // without a clean decontamination receipt.
  const decontaminationReceiptRef = assertCs336A4EvalDeltaDecontamination({
    decontaminationReceipt,
    measurement,
  })

  // Gate 3: price the bonus. The assignment and the recompute-verified flag are
  // taken from the trusted provenance receipt, not from a loose caller copy, so
  // the settlement cannot name a different shard or claim a verification the
  // bound receipt denies.
  const settlement = settleCs336A4EvalDeltaPayment({
    assignmentRef: provenanceReceipt.assignmentRef,
    measurement,
    stageRecomputeVerified: provenanceReceipt.recomputeVerified,
    // Spread conditionally so an absent funding parameter stays absent under
    // exactOptionalPropertyTypes rather than becoming an explicit `undefined`.
    ...(fundingParameters === undefined ? {} : { fundingParameters }),
  })

  // Gate 4: bind the settlement decision to the provenance receipt as a
  // content-addressed, auditable bonus record (also re-checks assignment match,
  // payable-requires-recompute, and public safety).
  const settlementReceipt = await buildCs336A4EvalDeltaSettlementReceipt({
    provenanceReceipt,
    settlement,
  })

  return {
    boundSourceRef: provenanceReceipt.provenance.sourceRef.trim(),
    decontaminationReceiptRef,
    settlement,
    settlementReceipt,
  }
}
