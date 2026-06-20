import { Cs336A4DataRefineryJobKind } from './cs336-a4-data-refinery'
import {
  type Cs336A4EvalDeltaFundingParameters,
  type Cs336A4EvalDeltaSettlement,
} from './cs336-a4-eval-delta-payment'

/**
 * Eval-delta funding-budget ledger for CS336 A4 refinery shards
 * (`blocker.product_promises.eval_delta_payment_missing`).
 *
 * `settleCs336A4EvalDeltaPayment` prices ONE bonus, and the settlement
 * closeout records ONE bonus against ONE shard's provenance. Both reason
 * about a single shard in isolation: pricing checks that the per-shard
 * funding parameters (`deltaCap`, `bonusRateSatsPerUnit`) are positive,
 * but nothing bounds what an operator actually has to PAY across a whole
 * dispatched BATCH. That leaves two ways a batch of individually-valid
 * settlements can silently overspend or misprice:
 *
 *  1. CUMULATIVE OVERSPEND — every settlement is priced correctly under
 *     positive funding parameters, but the SUM of their bonuses exceeds the
 *     fixed budget the operator authorized for the batch. Each shard looks
 *     payable; the operator is on the hook for more than it funded.
 *  2. UNAUTHORISED PER-SHARD PRICE — a settlement priced under DIFFERENT
 *     (larger) funding parameters than the operator authorized carries a
 *     bonus above the authorized per-shard maximum
 *     (`round(deltaCap * bonusRateSatsPerUnit)`), so a single shard can
 *     out-claim the price the operator approved.
 *
 * This module is the fail-closed gate that closes both. Given one operator
 * funding AUTHORIZATION (authority + total budget cap + the authorized
 * funding parameters) and the batch of already-priced settlements, it
 * proves the batch fits the authorization, then emits a deterministic,
 * content-addressed ledger binding the authorization to the ordered
 * per-shard charges and the remaining budget.
 *
 * It prices nothing (the settlement function's job), binds no provenance
 * (the settlement receipt's job), and settles no payment instrument: it
 * emits refs, sats amounts, and a content digest only — never wallet,
 * invoice, preimage, or private material; the public-safety guard fails
 * closed first.
 */

export const Cs336A4EvalDeltaFundingBudgetSchemaVersion =
  'openagents.training.data_refinery.eval_delta_funding_budget.v1' as const

/**
 * An operator-set funding authorization for a batch of eval-delta bonuses.
 * `budgetCapSats` is the TOTAL the operator authorizes for the batch;
 * `fundingParameters` are the per-shard pricing parameters the settlements
 * must have been priced under. Both are unset until operator funding is
 * approved, so no ledger can be built without an explicit authorization.
 */
export type Cs336A4EvalDeltaFundingAuthorization = Readonly<{
  /** Stable identifier of the approving operator/authority. */
  authorityRef: string
  /** Total sats authorized for the whole batch; a positive integer. */
  budgetCapSats: number
  /** The per-shard pricing parameters settlements were priced under. */
  fundingParameters: Cs336A4EvalDeltaFundingParameters
}>

/** One recorded per-shard charge in the ledger. */
export type Cs336A4EvalDeltaFundingBudgetEntry = Readonly<{
  assignmentRef: string
  /** Sats charged for this shard; 0 for a blocked (non-payable) settlement. */
  chargedBonusSats: number
  payable: boolean
}>

export type Cs336A4EvalDeltaFundingBudgetLedger = Readonly<{
  authorityRef: string
  budgetCapSats: number
  /** SHA-256 over the canonical ledger body (hex). */
  contentDigestRef: string
  /** Ordered per-shard charges (input order preserved). */
  entries: ReadonlyArray<Cs336A4EvalDeltaFundingBudgetEntry>
  jobKind: typeof Cs336A4DataRefineryJobKind
  /** Content-addressed ledger ref derived from contentDigestRef. */
  ledgerRef: string
  /** Number of payable settlements that drew on the budget. */
  payableSettlementCount: number
  /** Authorized per-shard maximum: round(deltaCap * bonusRateSatsPerUnit). */
  perShardMaxBonusSats: number
  /** budgetCapSats - totalChargedBonusSats; always >= 0. */
  remainingBudgetSats: number
  schemaVersion: typeof Cs336A4EvalDeltaFundingBudgetSchemaVersion
  /** Sum of payable charges; always <= budgetCapSats. */
  totalChargedBonusSats: number
}>

export const Cs336A4EvalDeltaFundingBudgetFailures = [
  'empty_authority_ref',
  'budget_cap_not_positive_integer',
  'funding_parameters_invalid',
  'per_shard_max_exceeds_budget',
  'duplicate_assignment',
  'settlement_bonus_not_nonnegative_integer',
  'settlement_bonus_exceeds_per_shard_max',
  'cumulative_bonus_exceeds_budget',
] as const
export type Cs336A4EvalDeltaFundingBudgetFailure =
  (typeof Cs336A4EvalDeltaFundingBudgetFailures)[number]

export class Cs336A4EvalDeltaFundingBudgetError extends Error {
  readonly _tag = 'Cs336A4EvalDeltaFundingBudgetError'
  readonly reason: Cs336A4EvalDeltaFundingBudgetFailure

  constructor(reason: Cs336A4EvalDeltaFundingBudgetFailure, detail: string) {
    super(detail)
    this.reason = reason
  }
}

export class Cs336A4EvalDeltaFundingBudgetUnsafeMaterialError extends Error {
  readonly _tag = 'Cs336A4EvalDeltaFundingBudgetUnsafeMaterialError'
}

const unsafeMaterialPattern =
  /(\/Users\/|\/home\/|api[_-]?key|bearer|bolt11|bolt12|invoice|lnbc|lntb|lnbcrt|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|private|raw[_-]?(crawl|dataset|invoice|payment|payload|prompt|runner|shard|warc)|secret|seed[_-]?phrase|sk-[a-z0-9]|wallet)/i

const assertJsonPublicSafe = (json: string): void => {
  if (unsafeMaterialPattern.test(json)) {
    throw new Cs336A4EvalDeltaFundingBudgetUnsafeMaterialError(
      'CS336 A4 eval-delta funding budget ledger contains wallet, payment, or private material.',
    )
  }
}

const sha256Hex = async (value: string): Promise<string> => {
  const digest = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(value),
  )

  return [...new Uint8Array(digest)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
}

const isNonNegativeInteger = (value: number): boolean =>
  Number.isInteger(value) && value >= 0

const isPositiveFinite = (value: number): boolean =>
  Number.isFinite(value) && value > 0

const canonicalLedgerBody = (
  input: Readonly<{
    authorityRef: string
    budgetCapSats: number
    entries: ReadonlyArray<Cs336A4EvalDeltaFundingBudgetEntry>
    perShardMaxBonusSats: number
    remainingBudgetSats: number
    totalChargedBonusSats: number
  }>,
): string =>
  JSON.stringify({
    authorityRef: input.authorityRef,
    budgetCapSats: input.budgetCapSats,
    entries: input.entries.map(entry => ({
      assignmentRef: entry.assignmentRef,
      chargedBonusSats: entry.chargedBonusSats,
      payable: entry.payable,
    })),
    jobKind: Cs336A4DataRefineryJobKind,
    perShardMaxBonusSats: input.perShardMaxBonusSats,
    remainingBudgetSats: input.remainingBudgetSats,
    schemaVersion: Cs336A4EvalDeltaFundingBudgetSchemaVersion,
    totalChargedBonusSats: input.totalChargedBonusSats,
  })

/**
 * Builds a deterministic, content-addressed eval-delta funding-budget
 * ledger for one operator authorization plus a batch of already-priced
 * settlements. Fails closed (throwing `Cs336A4EvalDeltaFundingBudgetError`
 * carrying the reason) when:
 *  - the authority ref is empty;
 *  - the total budget cap is not a positive integer;
 *  - the authorized funding parameters are not positive/finite;
 *  - the authorized per-shard maximum exceeds the whole budget (a single
 *    shard could overspend the batch — a misconfiguration);
 *  - any assignment appears twice (a shard double-charged against the budget);
 *  - a payable settlement's bonus is not a non-negative integer;
 *  - a payable settlement's bonus exceeds the authorized per-shard maximum
 *    (priced under unauthorized, larger funding parameters); or
 *  - the cumulative payable bonus exceeds the authorized budget cap.
 *
 * Blocked settlements are recorded with a 0 charge so the ledger is a
 * complete record of the batch. The `ledgerRef` is content-addressed via
 * SHA-256 over a canonical body, so the same authorization + settlements
 * (in the same order) always yield the same ref.
 */
export const buildCs336A4EvalDeltaFundingBudgetLedger = async (
  input: Readonly<{
    authorization: Cs336A4EvalDeltaFundingAuthorization
    settlements: ReadonlyArray<Cs336A4EvalDeltaSettlement>
  }>,
): Promise<Cs336A4EvalDeltaFundingBudgetLedger> => {
  const { authorization, settlements } = input

  const authorityRef = authorization.authorityRef.trim()
  if (authorityRef === '') {
    throw new Cs336A4EvalDeltaFundingBudgetError(
      'empty_authority_ref',
      'CS336 A4 eval-delta funding authorization requires a non-empty authorityRef.',
    )
  }

  const { budgetCapSats } = authorization
  if (!Number.isInteger(budgetCapSats) || budgetCapSats <= 0) {
    throw new Cs336A4EvalDeltaFundingBudgetError(
      'budget_cap_not_positive_integer',
      `CS336 A4 eval-delta funding authorization budgetCapSats must be a positive integer; got ${String(budgetCapSats)}.`,
    )
  }

  const { bonusRateSatsPerUnit, deltaCap } = authorization.fundingParameters
  if (!isPositiveFinite(deltaCap) || !isPositiveFinite(bonusRateSatsPerUnit)) {
    throw new Cs336A4EvalDeltaFundingBudgetError(
      'funding_parameters_invalid',
      'CS336 A4 eval-delta funding authorization requires positive, finite deltaCap and bonusRateSatsPerUnit.',
    )
  }

  // The authorized maximum a single shard can be paid, matching the pricing
  // formula in settleCs336A4EvalDeltaPayment.
  const perShardMaxBonusSats = Math.round(deltaCap * bonusRateSatsPerUnit)

  if (perShardMaxBonusSats > budgetCapSats) {
    throw new Cs336A4EvalDeltaFundingBudgetError(
      'per_shard_max_exceeds_budget',
      `CS336 A4 eval-delta authorized per-shard maximum (${perShardMaxBonusSats} sats) exceeds the whole batch budget (${budgetCapSats} sats); a single shard could overspend the batch.`,
    )
  }

  const seenAssignmentRefs = new Set<string>()
  const entries: Cs336A4EvalDeltaFundingBudgetEntry[] = []
  let totalChargedBonusSats = 0
  let payableSettlementCount = 0

  for (const settlement of settlements) {
    const assignmentRef = settlement.assignmentRef.trim()

    if (seenAssignmentRefs.has(assignmentRef)) {
      throw new Cs336A4EvalDeltaFundingBudgetError(
        'duplicate_assignment',
        `CS336 A4 eval-delta funding ledger has assignment ${assignmentRef} more than once; a shard cannot be charged twice against the budget.`,
      )
    }
    seenAssignmentRefs.add(assignmentRef)

    if (!settlement.payable) {
      entries.push({ assignmentRef, chargedBonusSats: 0, payable: false })
      continue
    }

    const chargedBonusSats = settlement.settledBonusSats

    if (!isNonNegativeInteger(chargedBonusSats)) {
      throw new Cs336A4EvalDeltaFundingBudgetError(
        'settlement_bonus_not_nonnegative_integer',
        `CS336 A4 eval-delta settlement for ${assignmentRef} has a non-integer or negative settledBonusSats: ${String(chargedBonusSats)}.`,
      )
    }

    if (chargedBonusSats > perShardMaxBonusSats) {
      throw new Cs336A4EvalDeltaFundingBudgetError(
        'settlement_bonus_exceeds_per_shard_max',
        `CS336 A4 eval-delta settlement for ${assignmentRef} charges ${chargedBonusSats} sats, above the authorized per-shard maximum ${perShardMaxBonusSats} sats; it was priced under unauthorized funding parameters.`,
      )
    }

    totalChargedBonusSats += chargedBonusSats
    payableSettlementCount += 1

    if (totalChargedBonusSats > budgetCapSats) {
      throw new Cs336A4EvalDeltaFundingBudgetError(
        'cumulative_bonus_exceeds_budget',
        `CS336 A4 eval-delta cumulative bonus (${totalChargedBonusSats} sats) exceeds the authorized budget (${budgetCapSats} sats) at assignment ${assignmentRef}.`,
      )
    }

    entries.push({ assignmentRef, chargedBonusSats, payable: true })
  }

  const remainingBudgetSats = budgetCapSats - totalChargedBonusSats

  const body = canonicalLedgerBody({
    authorityRef,
    budgetCapSats,
    entries,
    perShardMaxBonusSats,
    remainingBudgetSats,
    totalChargedBonusSats,
  })

  assertJsonPublicSafe(body)

  const contentDigestRef = await sha256Hex(body)
  const ledgerRef = `ledger.cs336_a4.eval_delta_funding_budget.${contentDigestRef.slice(0, 16)}`

  const ledger: Cs336A4EvalDeltaFundingBudgetLedger = {
    authorityRef,
    budgetCapSats,
    contentDigestRef,
    entries,
    jobKind: Cs336A4DataRefineryJobKind,
    ledgerRef,
    payableSettlementCount,
    perShardMaxBonusSats,
    remainingBudgetSats,
    schemaVersion: Cs336A4EvalDeltaFundingBudgetSchemaVersion,
    totalChargedBonusSats,
  }

  assertJsonPublicSafe(JSON.stringify(ledger))

  return ledger
}
