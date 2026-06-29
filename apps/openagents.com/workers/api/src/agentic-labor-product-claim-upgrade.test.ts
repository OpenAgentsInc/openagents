import { describe, expect, test } from 'vitest'

import {
  AGENTIC_LABOR_PRODUCT_SETTLEMENT_RECEIPT_SCHEMA,
  type LaborProductSettlementReceipt,
} from './agentic-labor-product'
import {
  AGENTIC_LABOR_PRODUCT_CLAIM_UPGRADE_SCHEMA,
  REAL_SALE_GATE_ATTESTATION_MATCHES,
  REAL_SALE_GATE_EXTERNAL_DEMAND,
  REAL_SALE_GATE_OWNER_SIGN_OFF,
  assessLaborProductRealSaleClaim,
  projectLaborProductRealSaleClaims,
} from './agentic-labor-product-claim-upgrade'
import {
  classifyLaborProductSaleDemand,
  type LaborProductDemandAttestation,
} from './agentic-labor-product-demand'

const receipt = (
  overrides: Partial<LaborProductSettlementReceipt> = {},
): LaborProductSettlementReceipt => ({
  schema: AGENTIC_LABOR_PRODUCT_SETTLEMENT_RECEIPT_SCHEMA,
  orderId: 'order-1',
  listingId: 'listing-1',
  sellerRef: 'agent:raynor',
  buyerRef: 'agent:buyer',
  accountRef: 'agent:buyer',
  streamKind: 'labor',
  receiptRef: 'receipt.autopilot.agentic_labor_product.order.order-1',
  settled: true,
  promiseIds: ['autopilot.agentic_labor_products.v1'],
  promiseState: 'yellow',
  settledAt: '2026-06-20T02:00:00.000Z',
  ...overrides,
})

// A real external attestation derived from the matching receipt.
const externalAttestation = (
  r: LaborProductSettlementReceipt,
): LaborProductDemandAttestation =>
  classifyLaborProductSaleDemand(r, {
    externalDemandRef: 'external.invoice.acme-corp.778',
  })

const AT = '2026-06-20T03:00:00.000Z'

describe('assessLaborProductRealSaleClaim', () => {
  test('substantiated ONLY when every gate passes', () => {
    const r = receipt()
    const claim = assessLaborProductRealSaleClaim(
      {
        receipt: r,
        demand: externalAttestation(r),
        ownerSignOffRef: 'owner.sign-off.raynor.2026-06-20',
      },
      { assessedAt: AT },
    )
    expect(claim.schema).toBe(AGENTIC_LABOR_PRODUCT_CLAIM_UPGRADE_SCHEMA)
    expect(claim.realSaleSubstantiated).toBe(true)
    expect(claim.failingGateRefs).toEqual([])
    expect(claim.gates).toEqual({
      settlementReceiptPresent: true,
      demandAttestationMatchesReceipt: true,
      externalDemandProvenance: true,
      ownerSignOffPresent: true,
    })
    // Even a substantiated claim NEVER flips the promise.
    expect(claim.promiseState).toBe('yellow')
    expect(claim.unclearedBlockerRefs).toEqual([])
    expect(claim.assessedAt).toBe(AT)
  })

  test('withholds when no owner sign-off (green is owner-gated)', () => {
    const r = receipt()
    const claim = assessLaborProductRealSaleClaim({
      receipt: r,
      demand: externalAttestation(r),
    })
    expect(claim.realSaleSubstantiated).toBe(false)
    expect(claim.gates.ownerSignOffPresent).toBe(false)
    expect(claim.failingGateRefs).toContain(REAL_SALE_GATE_OWNER_SIGN_OFF)
    expect(claim.unclearedBlockerRefs).toEqual([
      'blocker.product_promises.agentic_labor_product_real_sale_receipt_missing',
    ])
  })

  test('whitespace-only owner sign-off does not pass the gate', () => {
    const r = receipt()
    const claim = assessLaborProductRealSaleClaim({
      receipt: r,
      demand: externalAttestation(r),
      ownerSignOffRef: '   ',
    })
    expect(claim.gates.ownerSignOffPresent).toBe(false)
    expect(claim.realSaleSubstantiated).toBe(false)
  })

  test('withholds when demand is not external (self-dealt order)', () => {
    // buyer == seller => internal demand, not market.
    const r = receipt({ buyerRef: 'agent:raynor', accountRef: 'agent:raynor' })
    const claim = assessLaborProductRealSaleClaim({
      receipt: r,
      demand: externalAttestation(r),
      ownerSignOffRef: 'owner.sign-off.raynor.2026-06-20',
    })
    expect(claim.gates.externalDemandProvenance).toBe(false)
    expect(claim.realSaleSubstantiated).toBe(false)
    expect(claim.failingGateRefs).toContain(REAL_SALE_GATE_EXTERNAL_DEMAND)
  })

  test('rejects an external attestation that belongs to a DIFFERENT receipt', () => {
    // The classic hole: an external attestation for order A waved over receipt B.
    const settled = receipt()
    const otherExternal = externalAttestation(receipt({ orderId: 'order-OTHER' }))
    const claim = assessLaborProductRealSaleClaim({
      receipt: settled,
      demand: otherExternal,
      ownerSignOffRef: 'owner.sign-off.raynor.2026-06-20',
    })
    expect(claim.gates.demandAttestationMatchesReceipt).toBe(false)
    // A mismatched attestation can never satisfy the external-demand gate.
    expect(claim.gates.externalDemandProvenance).toBe(false)
    expect(claim.realSaleSubstantiated).toBe(false)
    expect(claim.failingGateRefs).toContain(REAL_SALE_GATE_ATTESTATION_MATCHES)
    expect(claim.failingGateRefs).toContain(REAL_SALE_GATE_EXTERNAL_DEMAND)
  })
})

describe('projectLaborProductRealSaleClaims', () => {
  test('empty input (production): nothing substantiated, blocker surfaced', () => {
    const projection = projectLaborProductRealSaleClaims([], { generatedAt: AT })
    expect(projection.totals).toEqual({
      assessedCount: 0,
      substantiatedCount: 0,
      withheldCount: 0,
    })
    expect(projection.realSaleClaimSubstantiated).toBe(false)
    expect(projection.promiseState).toBe('yellow')
    expect(projection.generatedAt).toBe(AT)
    expect(projection.unclearedBlockerRefs).toEqual([
      'blocker.product_promises.agentic_labor_product_real_sale_receipt_missing',
    ])
  })

  test('mixed input: counts substantiated vs withheld', () => {
    const good = receipt({ orderId: 'order-good' })
    const bad = receipt({ orderId: 'order-bad' })
    const projection = projectLaborProductRealSaleClaims(
      [
        {
          receipt: good,
          demand: externalAttestation(good),
          ownerSignOffRef: 'owner.sign-off.raynor',
        },
        // withheld: no owner sign-off
        { receipt: bad, demand: externalAttestation(bad) },
      ],
      { generatedAt: AT },
    )
    expect(projection.totals).toEqual({
      assessedCount: 2,
      substantiatedCount: 1,
      withheldCount: 1,
    })
    expect(projection.realSaleClaimSubstantiated).toBe(true)
    // The projection still never flips the promise.
    expect(projection.promiseState).toBe('yellow')
  })
})
