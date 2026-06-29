import { Cs336A4DataRefineryJobKind } from './cs336-a4-data-refinery'

/**
 * Eval-delta quality-bonus settlement computation for CS336 A4 refinery
 * shards.
 *
 * The deterministic refinery stages already pay a per-verified-shard
 * base (`policy.cs336_a4.pay_per_verified_shard_processed`). The
 * `training.data_refinery_corpus.v1` promise additionally requires "at
 * least one eval-delta payment computed from a fixed reference model".
 * The bonus formula and anti-gaming boundaries are documented in
 * docs/2026-06-10-cs336-a4-data-refinery-payment-policy.md
 * ("Eval-Delta Bonus Design"), but until now there was NO code that
 * turns a measured eval delta into a settlement decision — the policy
 * was prose only.
 *
 * This module is that deterministic, fail-closed settlement function. It
 * does NOT fabricate any eval delta: the caller must supply a real
 * downstream eval metric measured on the SAME source under a
 * held-constant trainer (filtered output vs unfiltered baseline). The
 * function decides, deterministically and with no I/O, whether a bonus
 * is payable and how many sats it is worth, enforcing every documented
 * boundary:
 *
 *  - the producing stage must have passed `deterministic_recompute`
 *    verification (an unverified stage earns no bonus);
 *  - the bonus pays measured downstream delta, never raw volume
 *    (`boundary.cs336_a4.pay_quality_delta_not_raw_volume_or_private_data`);
 *  - `delta > 0` is required (no penalty for neutral filtering, no bonus
 *    for quality regressions);
 *  - the delta is clamped to `deltaCap` before payment;
 *  - operator funding parameters (`deltaCap`, `bonusRateSatsPerUnit`)
 *    must be set and positive — they are unset until funding is approved,
 *    so the default path returns `blocked`, never a fabricated payout.
 *
 * No wallet, invoice, preimage, or private payload is ever accepted or
 * emitted; this module computes a public-safe sats amount and a basis,
 * not a payment instrument.
 */

export const Cs336A4EvalDeltaPaymentSchemaVersion =
  'openagents.training.data_refinery.eval_delta_payment.v1' as const

export const Cs336A4EvalDeltaMeasurementRef =
  'measurement.cs336_a4.downstream_eval_delta_fixed_reference_model' as const

export const Cs336A4EvalDeltaBonusPolicyRef =
  'policy.cs336_a4.eval_delta_quality_bonus_pending' as const

export const Cs336A4EvalDeltaBoundaryRef =
  'boundary.cs336_a4.pay_quality_delta_not_raw_volume_or_private_data' as const

/**
 * Why an eval-delta bonus was not paid. Each maps to a documented
 * boundary or blocker in the payment policy. A `payable` settlement
 * carries none of these.
 */
export const Cs336A4EvalDeltaNonPaymentReasons = [
  'funding_parameters_unset',
  'stage_recompute_unverified',
  'delta_not_positive',
] as const
export type Cs336A4EvalDeltaNonPaymentReason =
  (typeof Cs336A4EvalDeltaNonPaymentReasons)[number]

/**
 * Operator-set funding parameters. Both are unset until funding is
 * approved; a settlement attempted without them returns `blocked` with
 * `funding_parameters_unset` rather than a fabricated payout.
 */
export type Cs336A4EvalDeltaFundingParameters = Readonly<{
  /** Maximum delta paid for; larger measured deltas are clamped to this. */
  deltaCap: number
  /** Sats paid per unit of clamped delta. */
  bonusRateSatsPerUnit: number
}>

/** A held-constant-trainer downstream eval measurement for one shard. */
export type Cs336A4EvalDeltaMeasurement = Readonly<{
  /** Stable ref of the held-constant reference trainer config. */
  fixedReferenceModelRef: string
  /** Downstream eval metric of the trainer on the contributor's filtered output. */
  filteredScore: number
  /** Downstream eval metric of the trainer on the unfiltered baseline. */
  baselineScore: number
  /** Held-out eval set the contributor does not control. */
  heldOutEvalSetRef: string
  /** Source the filtered output and baseline were both derived from. */
  sourceRef: string
}>

export type Cs336A4EvalDeltaPaymentInput = Readonly<{
  assignmentRef: string
  /** True only when the producing stage passed deterministic_recompute. */
  stageRecomputeVerified: boolean
  measurement: Cs336A4EvalDeltaMeasurement
  /** Unset by default; supplied only once operator funding is approved. */
  fundingParameters?: Cs336A4EvalDeltaFundingParameters
}>

type Cs336A4EvalDeltaPaymentBase = Readonly<{
  assignmentRef: string
  boundaryRef: typeof Cs336A4EvalDeltaBoundaryRef
  jobKind: typeof Cs336A4DataRefineryJobKind
  measuredDelta: number
  measurementRef: typeof Cs336A4EvalDeltaMeasurementRef
  schemaVersion: typeof Cs336A4EvalDeltaPaymentSchemaVersion
}>

export type Cs336A4EvalDeltaPayableSettlement = Cs336A4EvalDeltaPaymentBase &
  Readonly<{
    bonusPolicyRef: 'policy.cs336_a4.eval_delta_quality_bonus_settled'
    /** Delta after clamping to the funding cap. */
    clampedDelta: number
    payable: true
    /** Bonus in sats; always >= 0, paid in addition to the base. */
    settledBonusSats: number
  }>

export type Cs336A4EvalDeltaBlockedSettlement = Cs336A4EvalDeltaPaymentBase &
  Readonly<{
    blockerRefs: ReadonlyArray<string>
    bonusPolicyRef: typeof Cs336A4EvalDeltaBonusPolicyRef
    payable: false
    reason: Cs336A4EvalDeltaNonPaymentReason
  }>

export type Cs336A4EvalDeltaSettlement =
  | Cs336A4EvalDeltaPayableSettlement
  | Cs336A4EvalDeltaBlockedSettlement

export class Cs336A4EvalDeltaPaymentValidationError extends Error {
  readonly _tag = 'Cs336A4EvalDeltaPaymentValidationError'
}

const blockerRefsForReason = (
  reason: Cs336A4EvalDeltaNonPaymentReason,
): ReadonlyArray<string> => {
  switch (reason) {
    case 'funding_parameters_unset':
      return [
        'blocker.cs336_a4.operator_funding_required_for_bonus_settlement',
        'blocker.cs336_a4.fixed_trainer_eval_loop_required_for_quality_bonus',
      ]
    case 'stage_recompute_unverified':
      return ['blocker.cs336_a4.bonus_requires_deterministic_recompute_verified_stage']
    case 'delta_not_positive':
      return ['boundary.cs336_a4.no_bonus_for_neutral_or_regressed_filtering']
  }
}

const requireFiniteNumber = (label: string, value: number): number => {
  if (!Number.isFinite(value)) {
    throw new Cs336A4EvalDeltaPaymentValidationError(
      `CS336 A4 eval-delta payment requires a finite ${label}.`,
    )
  }

  return value
}

const requireNonEmptyRef = (label: string, value: string): string => {
  const trimmed = value.trim()

  if (trimmed === '') {
    throw new Cs336A4EvalDeltaPaymentValidationError(
      `CS336 A4 eval-delta payment requires a non-empty ${label}.`,
    )
  }

  return trimmed
}

const clamp = (value: number, min: number, max: number): number =>
  Math.min(Math.max(value, min), max)

/**
 * Computes the eval-delta quality-bonus settlement for one refinery
 * shard, deterministically and with no I/O. Returns a `payable`
 * settlement only when the producing stage is recompute-verified,
 * operator funding parameters are set and positive, and the measured
 * downstream delta (filtered minus baseline, both under the same
 * held-constant trainer on the same source) is strictly positive.
 * Otherwise returns a `blocked` settlement carrying the documented
 * reason and blocker refs — never a fabricated payout.
 *
 * Bonus formula (matches the documented policy):
 *   bonus_sats = round(clamp(delta, 0, deltaCap) * bonusRateSatsPerUnit)
 */
export const settleCs336A4EvalDeltaPayment = (
  input: Cs336A4EvalDeltaPaymentInput,
): Cs336A4EvalDeltaSettlement => {
  const assignmentRef = requireNonEmptyRef('assignmentRef', input.assignmentRef)

  requireNonEmptyRef(
    'measurement.fixedReferenceModelRef',
    input.measurement.fixedReferenceModelRef,
  )
  requireNonEmptyRef(
    'measurement.heldOutEvalSetRef',
    input.measurement.heldOutEvalSetRef,
  )
  requireNonEmptyRef('measurement.sourceRef', input.measurement.sourceRef)

  const filteredScore = requireFiniteNumber(
    'measurement.filteredScore',
    input.measurement.filteredScore,
  )
  const baselineScore = requireFiniteNumber(
    'measurement.baselineScore',
    input.measurement.baselineScore,
  )

  const measuredDelta = filteredScore - baselineScore

  const base: Cs336A4EvalDeltaPaymentBase = {
    assignmentRef,
    boundaryRef: Cs336A4EvalDeltaBoundaryRef,
    jobKind: Cs336A4DataRefineryJobKind,
    measuredDelta,
    measurementRef: Cs336A4EvalDeltaMeasurementRef,
    schemaVersion: Cs336A4EvalDeltaPaymentSchemaVersion,
  }

  const blocked = (
    reason: Cs336A4EvalDeltaNonPaymentReason,
  ): Cs336A4EvalDeltaBlockedSettlement => ({
    ...base,
    blockerRefs: blockerRefsForReason(reason),
    bonusPolicyRef: Cs336A4EvalDeltaBonusPolicyRef,
    payable: false,
    reason,
  })

  // The bonus must first pass deterministic-recompute verification of the
  // stage that produced the filtered output.
  if (!input.stageRecomputeVerified) {
    return blocked('stage_recompute_unverified')
  }

  // No penalty for neutral filtering, no bonus for quality regressions.
  if (!(measuredDelta > 0)) {
    return blocked('delta_not_positive')
  }

  const funding = input.fundingParameters

  // Funding parameters are unset until operator funding is approved, and a
  // non-positive cap or rate cannot price a bonus; fail closed.
  if (
    funding === undefined ||
    !Number.isFinite(funding.deltaCap) ||
    !Number.isFinite(funding.bonusRateSatsPerUnit) ||
    funding.deltaCap <= 0 ||
    funding.bonusRateSatsPerUnit <= 0
  ) {
    return blocked('funding_parameters_unset')
  }

  const clampedDelta = clamp(measuredDelta, 0, funding.deltaCap)
  const settledBonusSats = Math.round(
    clampedDelta * funding.bonusRateSatsPerUnit,
  )

  return {
    ...base,
    bonusPolicyRef: 'policy.cs336_a4.eval_delta_quality_bonus_settled',
    clampedDelta,
    payable: true,
    settledBonusSats,
  }
}
