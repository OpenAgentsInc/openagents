import { describe, expect, test } from 'vitest'

import {
  AGENTIC_LABOR_PRODUCT_SETTLEMENT_RECEIPT_SCHEMA,
  type LaborProductSettlementReceipt,
} from './agentic-labor-product'
import {
  AGENTIC_LABOR_PRODUCT_DEMAND_SCHEMA,
  classifyLaborProductSaleDemand,
  projectLaborProductDemandProvenance,
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

describe('classifyLaborProductSaleDemand', () => {
  test('external ONLY on positive third-party evidence and a clean counterparty', () => {
    const att = classifyLaborProductSaleDemand(receipt(), {
      externalDemandRef: 'external.invoice.acme-corp.778',
    })
    expect(att.schema).toBe(AGENTIC_LABOR_PRODUCT_DEMAND_SCHEMA)
    expect(att.kind).toBe('external')
    expect(att.externalDemandClaimAllowed).toBe(true)
    expect(att.rule).toBe('no_external_dollar_no_demand_claim')
    expect(att.contractRef).toBe('proof.demand_provenance.v1')
    expect(att.promiseState).toBe('yellow')
  })

  test('self-dealt (buyer is seller) is internal, never external', () => {
    const att = classifyLaborProductSaleDemand(
      receipt({ buyerRef: 'agent:raynor', accountRef: 'agent:raynor' }),
      // even WITH an external ref, self-dealing disqualifies the claim
      { externalDemandRef: 'external.invoice.fake.1' },
    )
    expect(att.kind).toBe('internal')
    expect(att.externalDemandClaimAllowed).toBe(false)
    expect(att.reasonRef).toContain('self_dealt')
  })

  test('debited account equal to seller is also self-dealt internal', () => {
    const att = classifyLaborProductSaleDemand(
      receipt({ buyerRef: 'agent:buyer', accountRef: 'agent:raynor' }),
      { externalDemandRef: 'external.invoice.fake.2' },
    )
    expect(att.kind).toBe('internal')
    expect(att.externalDemandClaimAllowed).toBe(false)
  })

  test('a known internal/operator actor is internal, never external', () => {
    const att = classifyLaborProductSaleDemand(receipt(), {
      internalActorRefs: ['agent:buyer'],
      externalDemandRef: 'external.invoice.acme-corp.778',
    })
    expect(att.kind).toBe('internal')
    expect(att.externalDemandClaimAllowed).toBe(false)
    expect(att.reasonRef).toContain('known_first_party_actor')
  })

  test('no evidence => unlabeled, NOT external (no external dollar, no claim)', () => {
    const att = classifyLaborProductSaleDemand(receipt())
    expect(att.kind).toBe('unlabeled')
    expect(att.externalDemandClaimAllowed).toBe(false)
  })

  test('blank external ref is not positive evidence', () => {
    const att = classifyLaborProductSaleDemand(receipt(), {
      externalDemandRef: '   ',
    })
    expect(att.kind).toBe('unlabeled')
    expect(att.externalDemandClaimAllowed).toBe(false)
  })
})

describe('projectLaborProductDemandProvenance', () => {
  test('empty (production) projection: no external demand claim, blocker surfaced', () => {
    const projection = projectLaborProductDemandProvenance([], {
      generatedAt: '2026-06-20T03:00:00.000Z',
    })
    expect(projection.totals.settledReceiptCount).toBe(0)
    expect(projection.externalDemandClaimAllowed).toBe(false)
    expect(projection.promiseState).toBe('yellow')
    expect(projection.unclearedBlockerRefs).toContain(
      'blocker.product_promises.agentic_labor_product_real_sale_receipt_missing',
    )
  })

  test('aggregates the typed internal/external/unlabeled split', () => {
    const projection = projectLaborProductDemandProvenance([
      {
        receipt: receipt({ orderId: 'o-ext', receiptRef: 'r-ext' }),
        signals: { externalDemandRef: 'external.invoice.acme.1' },
      },
      {
        receipt: receipt({
          orderId: 'o-int',
          receiptRef: 'r-int',
          buyerRef: 'agent:raynor',
          accountRef: 'agent:raynor',
        }),
      },
      { receipt: receipt({ orderId: 'o-unl', receiptRef: 'r-unl' }) },
    ])
    expect(projection.totals).toEqual({
      settledReceiptCount: 3,
      externalCount: 1,
      internalCount: 1,
      unlabeledCount: 1,
    })
    expect(projection.externalDemandClaimAllowed).toBe(true)
    expect(projection.attestations).toHaveLength(3)
  })
})
