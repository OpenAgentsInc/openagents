// Real-sale claim-upgrade gate for labor-product settlements
// (promise autopilot.agentic_labor_products.v1, yellow; contract
// proof.claim_upgrade_receipts.v1).
//
// THE GAP THIS CLOSES (advances, does NOT clear,
// blocker.product_promises.agentic_labor_product_real_sale_receipt_missing):
// the pipeline can now MINT a settled receipt (recordLaborProductSettlement /
// carryLaborProductOrderToSettlement), DEREFERENCE it
// (readLaborProductSettlementReceipt), and CLASSIFY its demand provenance
// (classifyLaborProductSaleDemand). But those three live in separate modules and
// nothing assembled them into the SINGLE verdict a claim-upgrade review under
// proof.claim_upgrade_receipts.v1 is actually handed: "does THIS settlement
// substantiate a real external sale?" A reviewer had to eyeball three artifacts
// and hand-correlate them — and, worse, NOTHING checked that the external-demand
// attestation actually belonged to the settled receipt being reviewed. An
// `external` attestation for order A could be waved over a self-dealt receipt for
// order B and look like a real sale. This module is the conservative gate that
// closes that hole.
//
// SCOPE / HONESTY: PURE. It moves no money, reads no ledger, mints no receipt,
// and — critically — NEVER flips a promise state. Every output carries
// `promiseState: 'yellow'`. The verdict can only WITHHOLD a real-sale claim: a
// settlement is `realSaleSubstantiated` ONLY when every gate passes (a genuine
// settled receipt, an `external` demand attestation that demonstrably belongs to
// THAT receipt, and an owner sign-off ref). Absent any gate it is withheld. So
// this can never upgrade a claim on its own; it assembles the evidence a human
// owner sign-off + claim-upgrade review weighs. The real-sale-receipt blocker
// stays uncleared until a real external settled receipt is published and signed.

import { Schema as S } from 'effect'

import {
  AGENTIC_LABOR_PRODUCTS_PROMISE,
  LABOR_PRODUCT_NO_REAL_SALE_RECEIPT_REF,
  type LaborProductSettlementReceipt,
} from './agentic-labor-product'
import {
  DEMAND_PROVENANCE_CONTRACT,
  type LaborProductDemandAttestation,
} from './agentic-labor-product-demand'
import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

export const AGENTIC_LABOR_PRODUCT_CLAIM_UPGRADE_SCHEMA =
  'openagents.agentic_labor_product.real_sale_claim.v1' as const

/** The claim-upgrade contract whose receipt-first, owner-signed rule this gate serves. */
export const CLAIM_UPGRADE_CONTRACT = 'proof.claim_upgrade_receipts.v1' as const

/** Machine-readable refs for the gates a real-sale claim must clear. */
export const REAL_SALE_GATE_SETTLEMENT_RECEIPT =
  'gate.real_sale.settlement_receipt_present' as const
export const REAL_SALE_GATE_ATTESTATION_MATCHES =
  'gate.real_sale.demand_attestation_matches_receipt' as const
export const REAL_SALE_GATE_EXTERNAL_DEMAND =
  'gate.real_sale.external_demand_provenance' as const
export const REAL_SALE_GATE_OWNER_SIGN_OFF =
  'gate.real_sale.owner_sign_off_present' as const

/**
 * The four gates a settlement must clear to substantiate a real external sale.
 * Each is independently reported so a review can see exactly what is missing.
 */
export const LaborProductRealSaleGates = S.Struct({
  /** A genuine settled receipt is present (money moved). */
  settlementReceiptPresent: S.Boolean,
  /** The demand attestation demonstrably belongs to THIS receipt (orderId + receiptRef). */
  demandAttestationMatchesReceipt: S.Boolean,
  /** The matched attestation classifies the demand as `external` market demand. */
  externalDemandProvenance: S.Boolean,
  /** An owner sign-off ref authorizing the upgrade is present (green is owner-gated). */
  ownerSignOffPresent: S.Boolean,
})
export type LaborProductRealSaleGates = typeof LaborProductRealSaleGates.Type

/**
 * The typed verdict a claim-upgrade review under proof.claim_upgrade_receipts.v1
 * is handed for ONE settlement. Public-safe: neutral refs only, never a raw
 * amount or payment material. ALWAYS `promiseState: 'yellow'` — this gate assembles
 * evidence; it never flips a promise.
 */
export const LaborProductRealSaleClaim = S.Struct({
  schema: S.Literal(AGENTIC_LABOR_PRODUCT_CLAIM_UPGRADE_SCHEMA),
  orderId: S.String,
  receiptRef: S.String,
  gates: LaborProductRealSaleGates,
  /** The gate refs that did NOT pass (empty iff every gate passed). */
  failingGateRefs: S.Array(S.String),
  /** True ONLY when every gate passes: a substantiated real external sale. */
  realSaleSubstantiated: S.Boolean,
  contractRef: S.Literal(CLAIM_UPGRADE_CONTRACT),
  demandContractRef: S.Literal(DEMAND_PROVENANCE_CONTRACT),
  promiseIds: S.Tuple([S.Literal(AGENTIC_LABOR_PRODUCTS_PROMISE)]),
  /** Always yellow — substantiating evidence is not a green flip. */
  promiseState: S.Literal('yellow'),
  /** Surfaced unless the claim is substantiated (then the blocker is genuinely clearable). */
  unclearedBlockerRefs: S.Array(S.String),
  assessedAt: S.String,
})
export type LaborProductRealSaleClaim = typeof LaborProductRealSaleClaim.Type

const isNonEmpty = (value: string | undefined): value is string =>
  typeof value === 'string' && value.trim().length > 0

/** Inputs to assess one settlement's real-sale claim. */
export type LaborProductRealSaleClaimInput = Readonly<{
  /** The settled receipt for the order (money moved). */
  receipt: LaborProductSettlementReceipt
  /** The demand attestation produced for the order's settlement. */
  demand: LaborProductDemandAttestation
  /**
   * Owner sign-off ref authorizing the upgrade. Absent => the owner-sign-off gate
   * fails and the claim is withheld (green is owner-gated).
   */
  ownerSignOffRef?: string | undefined
}>

/**
 * Assess whether ONE settlement substantiates a real external sale. PURE and
 * conservative — `realSaleSubstantiated` is true ONLY when all four gates pass:
 *   1. a genuine settled receipt is present;
 *   2. the demand attestation belongs to THAT receipt (orderId AND receiptRef
 *      match) — so an `external` attestation can never be waved over a different
 *      (e.g. self-dealt) settlement;
 *   3. that matched attestation classifies the demand as `external`;
 *   4. an owner sign-off ref is present.
 * It can only WITHHOLD a claim, never manufacture one; the promise stays yellow.
 */
export const assessLaborProductRealSaleClaim = (
  input: LaborProductRealSaleClaimInput,
  options?: { assessedAt?: string },
): LaborProductRealSaleClaim => {
  const { receipt, demand } = input

  const settlementReceiptPresent = receipt.settled === true
  // The attestation must be FOR this receipt, by both keys — never trust an
  // attestation paired with a mismatched receipt.
  const demandAttestationMatchesReceipt =
    demand.orderId === receipt.orderId &&
    demand.receiptRef === receipt.receiptRef
  // Only a MATCHED attestation's external verdict counts.
  const externalDemandProvenance =
    demandAttestationMatchesReceipt &&
    demand.kind === 'external' &&
    demand.externalDemandClaimAllowed === true
  const ownerSignOffPresent = isNonEmpty(input.ownerSignOffRef)

  const gates: LaborProductRealSaleGates = {
    settlementReceiptPresent,
    demandAttestationMatchesReceipt,
    externalDemandProvenance,
    ownerSignOffPresent,
  }

  const failingGateRefs: string[] = []
  if (!settlementReceiptPresent) {
    failingGateRefs.push(REAL_SALE_GATE_SETTLEMENT_RECEIPT)
  }
  if (!demandAttestationMatchesReceipt) {
    failingGateRefs.push(REAL_SALE_GATE_ATTESTATION_MATCHES)
  }
  if (!externalDemandProvenance) {
    failingGateRefs.push(REAL_SALE_GATE_EXTERNAL_DEMAND)
  }
  if (!ownerSignOffPresent) {
    failingGateRefs.push(REAL_SALE_GATE_OWNER_SIGN_OFF)
  }

  const realSaleSubstantiated = failingGateRefs.length === 0

  return {
    schema: AGENTIC_LABOR_PRODUCT_CLAIM_UPGRADE_SCHEMA,
    orderId: receipt.orderId,
    receiptRef: receipt.receiptRef,
    gates,
    failingGateRefs,
    realSaleSubstantiated,
    contractRef: CLAIM_UPGRADE_CONTRACT,
    demandContractRef: DEMAND_PROVENANCE_CONTRACT,
    promiseIds: [AGENTIC_LABOR_PRODUCTS_PROMISE],
    promiseState: 'yellow',
    // Surface the blocker unless THIS claim is substantiated. A substantiated
    // claim does not itself clear the blocker (one sale + owner review does), but
    // it stops asserting the blocker as unconditionally open for this settlement.
    unclearedBlockerRefs: realSaleSubstantiated
      ? []
      : [LABOR_PRODUCT_NO_REAL_SALE_RECEIPT_REF],
    assessedAt: options?.assessedAt ?? currentIsoTimestamp(),
  }
}

export const LaborProductRealSaleClaimStaleness: PublicProjectionStalenessContract =
  liveAtReadStaleness([
    'agentic_labor_product_settlement_receipt_published',
    'product_promise_registry_updated',
  ])

/**
 * Public-safe projection over the assessed real-sale claims. Honest by
 * proof.claim_upgrade_receipts.v1: `realSaleClaimSubstantiated` is true ONLY when
 * at least one settlement clears every gate. In production the input set is empty
 * (no real external settled receipt has been published), so the count is 0, no
 * real-sale claim is substantiated, and the uncleared real-sale-receipt blocker is
 * surfaced. The promise stays yellow.
 */
/**
 * A read-only store of the evidence bundles (receipt + demand attestation +
 * owner sign-off) a claim-upgrade review weighs. Injected so the public surface
 * stays INERT by default: in production the Worker passes the EMPTY store (no
 * real external settled receipt has been published), so the verdict surface
 * honestly reports nothing substantiated. It is only non-empty when real
 * evidence bundles are deliberately published into it.
 */
export type LaborProductRealSaleClaimStore = {
  list: () => ReadonlyArray<LaborProductRealSaleClaimInput>
}

/** The default, empty claim-evidence store used while the surface is INERT. */
export const emptyLaborProductRealSaleClaimStore: LaborProductRealSaleClaimStore =
  {
    list: () => [],
  }

/** Build a fixed in-memory claim-evidence store (tests / deliberate publish). */
export const makeInMemoryLaborProductRealSaleClaimStore = (
  inputs: ReadonlyArray<LaborProductRealSaleClaimInput>,
): LaborProductRealSaleClaimStore => ({
  list: () => inputs,
})

export const projectLaborProductRealSaleClaims = (
  inputs: ReadonlyArray<LaborProductRealSaleClaimInput>,
  options?: { generatedAt?: string },
): {
  schema: typeof AGENTIC_LABOR_PRODUCT_CLAIM_UPGRADE_SCHEMA
  promiseIds: readonly [typeof AGENTIC_LABOR_PRODUCTS_PROMISE]
  promiseState: 'yellow'
  generatedAt: string
  staleness: PublicProjectionStalenessContract
  maxStalenessSeconds: number
  contractRef: typeof CLAIM_UPGRADE_CONTRACT
  totals: {
    assessedCount: number
    substantiatedCount: number
    withheldCount: number
  }
  realSaleClaimSubstantiated: boolean
  unclearedBlockerRefs: ReadonlyArray<string>
  claims: ReadonlyArray<LaborProductRealSaleClaim>
} => {
  const generatedAt = options?.generatedAt ?? currentIsoTimestamp()
  const claims = inputs.map(input =>
    assessLaborProductRealSaleClaim(input, { assessedAt: generatedAt }),
  )
  const substantiatedCount = claims.filter(c => c.realSaleSubstantiated).length

  return {
    schema: AGENTIC_LABOR_PRODUCT_CLAIM_UPGRADE_SCHEMA,
    promiseIds: [AGENTIC_LABOR_PRODUCTS_PROMISE],
    promiseState: 'yellow',
    generatedAt,
    staleness: LaborProductRealSaleClaimStaleness,
    maxStalenessSeconds: LaborProductRealSaleClaimStaleness.maxStalenessSeconds,
    contractRef: CLAIM_UPGRADE_CONTRACT,
    totals: {
      assessedCount: claims.length,
      substantiatedCount,
      withheldCount: claims.length - substantiatedCount,
    },
    realSaleClaimSubstantiated: substantiatedCount > 0,
    unclearedBlockerRefs: [LABOR_PRODUCT_NO_REAL_SALE_RECEIPT_REF],
    claims,
  }
}
