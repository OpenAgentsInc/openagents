// Demand-provenance classification for labor-product settlements
// (promise autopilot.agentic_labor_products.v1, yellow; rule from
// proof.demand_provenance.v1: "no external dollar, no demand claim").
//
// THE GAP THIS CLOSES (advances, does NOT clear,
// blocker.product_promises.agentic_labor_product_real_sale_receipt_missing):
// the carry-through can now MINT and DEREFERENCE a typed
// `LaborProductSettlementReceipt` when money genuinely moves
// (recordLaborProductSettlement / carryLaborProductOrderToSettlement /
// readLaborProductSettlementReceipt). But a receipt minted from a SELF-DEALT or
// operator-staged order — buyer == seller, or an internal first-party account —
// is byte-for-byte indistinguishable from one minted by a real external buyer.
// A claim-upgrade review under proof.demand_provenance.v1 cannot accept a
// settlement as evidence of a REAL SALE unless the demand behind it is labeled
// external (a third party paid real dollars), not internal plumbing. Nothing
// labeled a labor-product settlement's demand provenance — so every settled
// receipt silently looked like market demand. This module adds that typed,
// conservative label and the public projection that enforces the rule.
//
// SCOPE / HONESTY: PURE. It classifies and aggregates already-recorded receipts;
// it moves no money, reads no ledger, and mints no receipt. The classification
// is conservative by construction: `external` is granted ONLY on POSITIVE
// evidence of a third-party real-dollar order (a non-empty external demand ref)
// AND only when the order is not self-dealt and not from a known internal actor;
// absent that evidence a settlement is `unlabeled`, never `external`. So this can
// never UPGRADE a claim on its own — it can only WITHHOLD one. The promise stays
// yellow and the real-sale-receipt blocker stays uncleared until a real external
// settled receipt is published.

import { Schema as S } from 'effect'

import {
  AGENTIC_LABOR_PRODUCTS_PROMISE,
  LABOR_PRODUCT_NO_REAL_SALE_RECEIPT_REF,
  type LaborProductSettlementReceipt,
} from './agentic-labor-product'
import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

export const AGENTIC_LABOR_PRODUCT_DEMAND_SCHEMA =
  'openagents.agentic_labor_product.demand_provenance.v1' as const

/** The demand-provenance contract whose rule this projection enforces. */
export const DEMAND_PROVENANCE_CONTRACT = 'proof.demand_provenance.v1' as const

/** The rule a labor-product settlement's demand provenance is judged under. */
export const LABOR_PRODUCT_DEMAND_RULE =
  'no_external_dollar_no_demand_claim' as const

/**
 * The demand provenance behind ONE settled labor-product order:
 *   - `external` = a third party paid real dollars (genuine market demand);
 *   - `internal` = first-party / self-dealt / operator-staged (plumbing, not a
 *     market sale);
 *   - `unlabeled` = no positive evidence either way — treated as NOT external.
 * Only `external` may ever count toward a real-sale demand claim.
 */
export const LaborProductDemandKind = S.Literals([
  'external',
  'internal',
  'unlabeled',
])
export type LaborProductDemandKind = typeof LaborProductDemandKind.Type

/**
 * Signals used to classify ONE settlement's demand provenance. Neutral refs
 * only — no payment material.
 */
export type LaborProductDemandSignals = Readonly<{
  /**
   * Known internal / first-party / operator-staged actor refs. A settlement
   * whose buyer or debited account is in this set is `internal` (plumbing).
   */
  internalActorRefs?: ReadonlyArray<string> | undefined
  /**
   * POSITIVE evidence of a third-party, real-dollar order (e.g. an external
   * invoice / purchase-order ref). Only a non-empty value, on a settlement that
   * is neither self-dealt nor from a known internal actor, yields `external`.
   */
  externalDemandRef?: string | undefined
}>

/**
 * A typed, public-safe demand-provenance attestation for ONE settled
 * labor-product order. Carries the conservative classification, the reason ref
 * explaining it, and whether the settlement may count as external market demand.
 */
export const LaborProductDemandAttestation = S.Struct({
  schema: S.Literal(AGENTIC_LABOR_PRODUCT_DEMAND_SCHEMA),
  orderId: S.String,
  receiptRef: S.String,
  kind: LaborProductDemandKind,
  /** Machine-readable reason ref for the classification (audit trail). */
  reasonRef: S.String,
  /** True only when `kind === 'external'`: the rule's gate on a demand claim. */
  externalDemandClaimAllowed: S.Boolean,
  rule: S.Literal(LABOR_PRODUCT_DEMAND_RULE),
  contractRef: S.Literal(DEMAND_PROVENANCE_CONTRACT),
  promiseIds: S.Tuple([S.Literal(AGENTIC_LABOR_PRODUCTS_PROMISE)]),
  promiseState: S.Literal('yellow'),
})
export type LaborProductDemandAttestation =
  typeof LaborProductDemandAttestation.Type

const isNonEmpty = (value: string | undefined): value is string =>
  typeof value === 'string' && value.trim().length > 0

/**
 * Classify the demand provenance behind ONE settled labor-product receipt.
 * PURE and conservative — `external` requires POSITIVE third-party evidence and
 * a non-self-dealt, non-internal counterparty; otherwise the settlement is
 * `internal` (self-dealt / known internal actor) or `unlabeled` (no evidence).
 * It can only WITHHOLD an external demand claim, never manufacture one.
 */
export const classifyLaborProductSaleDemand = (
  receipt: LaborProductSettlementReceipt,
  signals: LaborProductDemandSignals = {},
): LaborProductDemandAttestation => {
  const base = {
    schema: AGENTIC_LABOR_PRODUCT_DEMAND_SCHEMA,
    orderId: receipt.orderId,
    receiptRef: receipt.receiptRef,
    rule: LABOR_PRODUCT_DEMAND_RULE,
    contractRef: DEMAND_PROVENANCE_CONTRACT,
    promiseIds: [AGENTIC_LABOR_PRODUCTS_PROMISE],
    promiseState: 'yellow',
  } as const

  const internalActorRefs = signals.internalActorRefs ?? []
  const isSelfDealt =
    receipt.buyerRef === receipt.sellerRef ||
    receipt.accountRef === receipt.sellerRef
  const isInternalActor =
    internalActorRefs.includes(receipt.buyerRef) ||
    internalActorRefs.includes(receipt.accountRef)

  // Self-dealt: you cannot buy from yourself and call it market demand.
  if (isSelfDealt) {
    return {
      ...base,
      kind: 'internal',
      reasonRef:
        'demand.labor_product.internal.self_dealt_buyer_is_seller',
      externalDemandClaimAllowed: false,
    }
  }
  // Known first-party / operator-staged actor: plumbing, not a market sale.
  if (isInternalActor) {
    return {
      ...base,
      kind: 'internal',
      reasonRef: 'demand.labor_product.internal.known_first_party_actor',
      externalDemandClaimAllowed: false,
    }
  }
  // Positive third-party evidence and no internal disqualifier: external.
  if (isNonEmpty(signals.externalDemandRef)) {
    return {
      ...base,
      kind: 'external',
      reasonRef: 'demand.labor_product.external.third_party_real_dollar_ref',
      externalDemandClaimAllowed: true,
    }
  }
  // No evidence either way: never presented as external market demand.
  return {
    ...base,
    kind: 'unlabeled',
    reasonRef: 'demand.labor_product.unlabeled.no_external_demand_evidence',
    externalDemandClaimAllowed: false,
  }
}

/** One settled receipt paired with the signals to classify its demand. */
export type LaborProductDemandEntry = Readonly<{
  receipt: LaborProductSettlementReceipt
  signals?: LaborProductDemandSignals | undefined
}>

export const LaborProductDemandStaleness: PublicProjectionStalenessContract =
  liveAtReadStaleness([
    'agentic_labor_product_settlement_receipt_published',
    'product_promise_registry_updated',
  ])

/**
 * Public-safe demand-provenance projection over the published labor-product
 * settlement receipts. Honest by the rule `no_external_dollar_no_demand_claim`:
 * `externalDemandClaimAllowed` is true ONLY when at least one settlement is
 * classified `external`. In production the receipt set is empty (no real sale
 * has been published), so every count is 0, no external demand claim is allowed,
 * and the uncleared real-sale-receipt blocker is surfaced. The promise stays
 * yellow.
 */
export const projectLaborProductDemandProvenance = (
  entries: ReadonlyArray<LaborProductDemandEntry>,
  options?: { generatedAt?: string },
): {
  schema: typeof AGENTIC_LABOR_PRODUCT_DEMAND_SCHEMA
  promiseIds: readonly [typeof AGENTIC_LABOR_PRODUCTS_PROMISE]
  promiseState: 'yellow'
  generatedAt: string
  staleness: PublicProjectionStalenessContract
  maxStalenessSeconds: number
  rule: typeof LABOR_PRODUCT_DEMAND_RULE
  contractRef: typeof DEMAND_PROVENANCE_CONTRACT
  totals: {
    settledReceiptCount: number
    externalCount: number
    internalCount: number
    unlabeledCount: number
  }
  externalDemandClaimAllowed: boolean
  unclearedBlockerRefs: ReadonlyArray<string>
  caveatRefs: ReadonlyArray<string>
  attestations: ReadonlyArray<LaborProductDemandAttestation>
} => {
  const attestations = entries.map(entry =>
    classifyLaborProductSaleDemand(entry.receipt, entry.signals ?? {}),
  )
  const externalCount = attestations.filter(a => a.kind === 'external').length
  const internalCount = attestations.filter(a => a.kind === 'internal').length
  const unlabeledCount = attestations.filter(a => a.kind === 'unlabeled').length

  return {
    schema: AGENTIC_LABOR_PRODUCT_DEMAND_SCHEMA,
    promiseIds: [AGENTIC_LABOR_PRODUCTS_PROMISE],
    promiseState: 'yellow',
    generatedAt: options?.generatedAt ?? currentIsoTimestamp(),
    staleness: LaborProductDemandStaleness,
    maxStalenessSeconds: LaborProductDemandStaleness.maxStalenessSeconds,
    rule: LABOR_PRODUCT_DEMAND_RULE,
    contractRef: DEMAND_PROVENANCE_CONTRACT,
    totals: {
      settledReceiptCount: attestations.length,
      externalCount,
      internalCount,
      unlabeledCount,
    },
    // The rule: no external dollar, no demand claim.
    externalDemandClaimAllowed: externalCount > 0,
    unclearedBlockerRefs: [LABOR_PRODUCT_NO_REAL_SALE_RECEIPT_REF],
    caveatRefs: [
      'caveat.demand_provenance.internal_demand_is_plumbing_not_market',
      'caveat.demand_provenance.no_external_dollar_no_demand_claim',
    ],
    attestations,
  }
}
