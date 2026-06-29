import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  AGENTIC_LABOR_PRODUCT_PRIMITIVE,
  AGENTIC_LABOR_PRODUCTS_PROMISE,
  AGENTIC_LABOR_PRODUCT_SETTLEMENT_RECEIPT_SCHEMA,
  advanceLaborProductFlow,
  buildLaborProductFlowPlan,
  decodeLaborProductOrderRequest,
  laborProductOrderReceiptRef,
  laborProductStageIndex,
  listLaborProductFlows,
  listLaborProductSettlementReceipts,
  makeInMemoryLaborProductFlowStore,
  makeInMemoryLaborProductReceiptStore,
  planSelfServeLaborProductOrder,
  readLaborProductFlow,
  readLaborProductSettlementReceipt,
  recordLaborProductSettlement,
  settleLaborProductOrder,
  type LaborProductListing,
  type SettleLaborProductOrderResult,
} from './agentic-labor-product'
import { cloudChargeReceiptRef } from './cloud/cloud-metering'

const listing: LaborProductListing = {
  listingId: 'listing-1',
  sellerRef: 'agent:raynor',
  title: 'Repo triage labor product',
  summary: 'Triage and label one repo backlog and deliver a report.',
  capabilityRef: 'promise:autopilot.agentic_labor_products.v1',
  priceSats: 100,
}

const okPlan = (
  overrides: Partial<Parameters<typeof buildLaborProductFlowPlan>[0]> = {},
) => {
  const result = buildLaborProductFlowPlan({
    orderId: 'order-1',
    buyerRef: 'agent:buyer',
    listing,
    stage: 'delivered',
    workerRef: 'agent:worker',
    artifactRef: 'artifact.repo_triage.order-1',
    createdAt: '2026-06-19T12:00:00.000Z',
    ...overrides,
  })
  if (!result.ok) {
    throw new Error(result.error.reason)
  }
  return result.plan
}

describe('labor-product stage lifecycle', () => {
  test('stages advance strictly forward', () => {
    expect(laborProductStageIndex('posted')).toBe(0)
    expect(laborProductStageIndex('ordered')).toBe(1)
    expect(laborProductStageIndex('dispatched')).toBe(2)
    expect(laborProductStageIndex('delivered')).toBe(3)
    expect(laborProductStageIndex('settled')).toBe(4)
  })
})

describe('buildLaborProductFlowPlan', () => {
  test('builds a coherent delivered flow plan, pinned yellow + inert', () => {
    const plan = okPlan()
    expect(plan.schema).toBe('openagents.agentic_labor_product.v1')
    expect(plan.promiseState).toBe('yellow')
    expect(plan.inert).toBe(true)
    expect(plan.promiseIds).toEqual([AGENTIC_LABOR_PRODUCTS_PROMISE])
    expect(plan.stage).toBe('delivered')
    expect(plan.workerRef).toBe('agent:worker')
    expect(plan.artifactRef).toBe('artifact.repo_triage.order-1')
  })

  test('derives the settlement receipt ref from the shared cloud-metering helper', () => {
    const plan = okPlan()
    expect(plan.settlement.receiptRef).toBe(
      cloudChargeReceiptRef(AGENTIC_LABOR_PRODUCT_PRIMITIVE, 'order-1'),
    )
    expect(plan.settlement.receiptRef).toBe(
      laborProductOrderReceiptRef('order-1'),
    )
    expect(plan.settlement.streamKind).toBe('labor')
    expect(plan.settlement.accountRef).toBe('agent:buyer')
  })

  test('records only the remaining uncleared blocker (self-serve is cleared)', () => {
    expect(okPlan().unclearedBlockerRefs).toEqual([
      'blocker.product_promises.agentic_labor_product_real_sale_receipt_missing',
    ])
  })

  test('a posted/ordered flow needs no worker or artifact', () => {
    const result = buildLaborProductFlowPlan({
      orderId: 'order-2',
      buyerRef: 'agent:buyer',
      listing,
      stage: 'ordered',
    })
    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.plan.workerRef).toBeNull()
      expect(result.plan.artifactRef).toBeNull()
    }
  })

  test('rejects a dispatched flow with no workerRef', () => {
    const result = buildLaborProductFlowPlan({
      orderId: 'order-3',
      buyerRef: 'agent:buyer',
      listing,
      stage: 'dispatched',
    })
    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.error.reason).toContain('workerRef')
    }
  })

  test('rejects a delivered flow with no artifactRef', () => {
    const result = buildLaborProductFlowPlan({
      orderId: 'order-4',
      buyerRef: 'agent:buyer',
      listing,
      stage: 'delivered',
      workerRef: 'agent:worker',
    })
    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.error.reason).toContain('artifactRef')
    }
  })

  test('rejects an empty order id, empty buyer, or fractional price', () => {
    expect(
      buildLaborProductFlowPlan({
        orderId: ' ',
        buyerRef: 'agent:buyer',
        listing,
        stage: 'posted',
      }).ok,
    ).toBe(false)
    expect(
      buildLaborProductFlowPlan({
        orderId: 'order-5',
        buyerRef: '',
        listing,
        stage: 'posted',
      }).ok,
    ).toBe(false)
    expect(
      buildLaborProductFlowPlan({
        orderId: 'order-6',
        buyerRef: 'agent:buyer',
        listing: { ...listing, priceSats: 1.5 },
        stage: 'posted',
      }).ok,
    ).toBe(false)
  })
})

describe('labor-product flow store + projection', () => {
  test('lists flows, always reporting yellow + inert', () => {
    const store = makeInMemoryLaborProductFlowStore([okPlan()])
    const projection = listLaborProductFlows(store)
    expect(projection.promiseState).toBe('yellow')
    expect(projection.inert).toBe(true)
    expect(projection.maxStalenessSeconds).toBe(0)
    expect(projection.flows.map(f => f.orderId)).toEqual(['order-1'])
  })

  test('reads one flow by order id, or null when absent', () => {
    const store = makeInMemoryLaborProductFlowStore([okPlan()])
    expect(readLaborProductFlow(store, 'order-1')?.orderId).toBe('order-1')
    expect(readLaborProductFlow(store, 'missing')).toBeNull()
  })
})

describe('self-serve order planning (PURE, INERT)', () => {
  test('decodes a valid order request', () => {
    const decoded = decodeLaborProductOrderRequest({
      orderId: 'order-self-1',
      buyerRef: 'agent:buyer',
      listing,
    })
    expect(decoded.ok).toBe(true)
    if (decoded.ok) {
      expect(decoded.request.orderId).toBe('order-self-1')
      expect(decoded.request.listing.priceSats).toBe(100)
    }
  })

  test('rejects a non-object body or a malformed order request', () => {
    expect(decodeLaborProductOrderRequest('nope').ok).toBe(false)
    expect(decodeLaborProductOrderRequest(null).ok).toBe(false)
    expect(
      decodeLaborProductOrderRequest({ orderId: 'x', buyerRef: 'y' }).ok,
    ).toBe(false)
    expect(
      decodeLaborProductOrderRequest({
        orderId: 'x',
        buyerRef: 'y',
        listing: { ...listing, priceSats: 'free' },
      }).ok,
    ).toBe(false)
  })

  test('plans an ordered-stage flow with no operator staging, still inert/yellow', () => {
    const planned = planSelfServeLaborProductOrder(
      { orderId: 'order-self-2', buyerRef: 'agent:buyer', listing },
      { createdAt: '2026-06-20T00:00:00.000Z' },
    )
    expect(planned.ok).toBe(true)
    if (planned.ok) {
      expect(planned.plan.stage).toBe('ordered')
      expect(planned.plan.inert).toBe(true)
      expect(planned.plan.promiseState).toBe('yellow')
      expect(planned.plan.workerRef).toBeNull()
      expect(planned.plan.artifactRef).toBeNull()
      // Self-serve plan carries the same public-safe would-be receipt ref.
      expect(planned.plan.settlement.receiptRef).toBe(
        laborProductOrderReceiptRef('order-self-2'),
      )
      expect(planned.plan.createdAt).toBe('2026-06-20T00:00:00.000Z')
    }
  })

  test('a self-serve plan validates the same way as an operator plan (empty fields rejected)', () => {
    const planned = planSelfServeLaborProductOrder({
      orderId: ' ',
      buyerRef: 'agent:buyer',
      listing,
    })
    expect(planned.ok).toBe(false)
  })
})

describe('advanceLaborProductFlow (forward-only, PURE/INERT)', () => {
  const orderedPlan = () =>
    okPlan({ stage: 'ordered', workerRef: null, artifactRef: null })

  test('carries a self-serve order forward: ordered -> dispatched -> delivered', () => {
    const ordered = planSelfServeLaborProductOrder(
      { orderId: 'order-1', buyerRef: 'agent:buyer', listing },
      { createdAt: '2026-06-20T00:00:00.000Z' },
    )
    expect(ordered.ok).toBe(true)
    if (!ordered.ok) return

    const dispatched = advanceLaborProductFlow(ordered.plan, {
      kind: 'dispatch',
      workerRef: 'agent:worker',
    })
    expect(dispatched.ok).toBe(true)
    if (!dispatched.ok) return
    expect(dispatched.plan.stage).toBe('dispatched')
    expect(dispatched.plan.workerRef).toBe('agent:worker')
    expect(dispatched.plan.artifactRef).toBeNull()

    const delivered = advanceLaborProductFlow(dispatched.plan, {
      kind: 'deliver',
      artifactRef: 'artifact.repo_triage.order-1',
    })
    expect(delivered.ok).toBe(true)
    if (!delivered.ok) return
    expect(delivered.plan.stage).toBe('delivered')
    expect(delivered.plan.workerRef).toBe('agent:worker')
    expect(delivered.plan.artifactRef).toBe('artifact.repo_triage.order-1')

    // Identity carried unchanged across the whole carry-through.
    expect(delivered.plan.orderId).toBe('order-1')
    expect(delivered.plan.buyerRef).toBe('agent:buyer')
    expect(delivered.plan.createdAt).toBe('2026-06-20T00:00:00.000Z')
    expect(delivered.plan.settlement.receiptRef).toBe(
      laborProductOrderReceiptRef('order-1'),
    )
    // Still honest at every step.
    expect(delivered.plan.inert).toBe(true)
    expect(delivered.plan.promiseState).toBe('yellow')
  })

  test('dispatch requires an ordered flow', () => {
    const result = advanceLaborProductFlow(okPlan(), {
      kind: 'dispatch',
      workerRef: 'agent:worker',
    })
    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.error.reason).toContain('ordered')
    }
  })

  test('dispatch rejects an empty workerRef', () => {
    const result = advanceLaborProductFlow(orderedPlan(), {
      kind: 'dispatch',
      workerRef: ' ',
    })
    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.error.reason).toContain('workerRef')
    }
  })

  test('deliver requires a dispatched flow', () => {
    const result = advanceLaborProductFlow(orderedPlan(), {
      kind: 'deliver',
      artifactRef: 'artifact.x',
    })
    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.error.reason).toContain('dispatched')
    }
  })

  test('deliver rejects an empty artifactRef', () => {
    const dispatched = advanceLaborProductFlow(orderedPlan(), {
      kind: 'dispatch',
      workerRef: 'agent:worker',
    })
    expect(dispatched.ok).toBe(true)
    if (!dispatched.ok) return
    const result = advanceLaborProductFlow(dispatched.plan, {
      kind: 'deliver',
      artifactRef: '',
    })
    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.error.reason).toContain('artifactRef')
    }
  })
})

describe('settleLaborProductOrder (FLAG-GATED INERT)', () => {
  // A D1 stub that THROWS on any IO. The disabled/not_authorized paths must
  // never touch it; reaching it would throw and fail the test.
  const throwingDb = {
    prepare: () => {
      throw new Error('settlement seam touched the ledger while inert')
    },
    batch: () => {
      throw new Error('settlement seam touched the ledger while inert')
    },
  } as unknown as D1Database

  test('disabled (default): plans only, never touches the ledger', async () => {
    const result = await Effect.runPromise(
      settleLaborProductOrder(
        { db: throwingDb, enabled: false },
        { plan: okPlan(), adapterId: 'labor-runtime', ownerSignOffRef: 'owner.sig.1' },
      ),
    )
    expect(result._tag).toBe('disabled')
    expect(result.receiptRef).toBe(laborProductOrderReceiptRef('order-1'))
  })

  test('armed but no owner sign-off: not_authorized, no ledger IO', async () => {
    const result = await Effect.runPromise(
      settleLaborProductOrder(
        { db: throwingDb, enabled: true },
        { plan: okPlan(), adapterId: 'labor-runtime' },
      ),
    )
    expect(result._tag).toBe('not_authorized')
    if (result._tag === 'not_authorized') {
      expect(result.reason).toContain('owner sign-off')
    }
  })

  test('armed but order not delivered: not_authorized, no ledger IO', async () => {
    const orderedPlan = okPlan({
      stage: 'ordered',
      workerRef: null,
      artifactRef: null,
    })
    const result = await Effect.runPromise(
      settleLaborProductOrder(
        { db: throwingDb, enabled: true },
        {
          plan: orderedPlan,
          adapterId: 'labor-runtime',
          ownerSignOffRef: 'owner.sig.1',
        },
      ),
    )
    expect(result._tag).toBe('not_authorized')
    if (result._tag === 'not_authorized') {
      expect(result.reason).toContain('delivered')
    }
  })
})

describe('recordLaborProductSettlement (PURE)', () => {
  const receiptRef = laborProductOrderReceiptRef('order-1')
  const meteredResult: SettleLaborProductOrderResult = {
    _tag: 'settled',
    receiptRef,
    outcome: { metered: true, receiptRef },
  }

  test('mints a settled flow plan + dereferenceable receipt from a metered settlement', () => {
    const recorded = recordLaborProductSettlement(okPlan(), meteredResult, {
      settledAt: '2026-06-20T01:00:00.000Z',
    })
    expect(recorded.ok).toBe(true)
    if (!recorded.ok) return

    // The terminal stage is now produced, with identity + receipt carried.
    expect(recorded.plan.stage).toBe('settled')
    expect(recorded.plan.orderId).toBe('order-1')
    expect(recorded.plan.workerRef).toBe('agent:worker')
    expect(recorded.plan.artifactRef).toBe('artifact.repo_triage.order-1')

    expect(recorded.receipt.schema).toBe(
      AGENTIC_LABOR_PRODUCT_SETTLEMENT_RECEIPT_SCHEMA,
    )
    expect(recorded.receipt.receiptRef).toBe(receiptRef)
    expect(recorded.receipt.settled).toBe(true)
    expect(recorded.receipt.streamKind).toBe('labor')
    expect(recorded.receipt.sellerRef).toBe('agent:raynor')
    expect(recorded.receipt.buyerRef).toBe('agent:buyer')
    expect(recorded.receipt.settledAt).toBe('2026-06-20T01:00:00.000Z')
    // Still yellow — one settled order does not flip the promise.
    expect(recorded.receipt.promiseState).toBe('yellow')
  })

  test('rejects a non-settled seam result (disabled / not_authorized)', () => {
    const recorded = recordLaborProductSettlement(okPlan(), {
      _tag: 'disabled',
      receiptRef,
    })
    expect(recorded.ok).toBe(false)
    if (!recorded.ok) {
      expect(recorded.error.reason).toContain('settled')
    }
  })

  test('rejects an unmetered settlement (no money moved => no receipt)', () => {
    const recorded = recordLaborProductSettlement(okPlan(), {
      _tag: 'settled',
      receiptRef,
      outcome: { metered: false, receiptRef: null },
    })
    expect(recorded.ok).toBe(false)
    if (!recorded.ok) {
      expect(recorded.error.reason).toContain('metered')
    }
  })

  test('rejects a receipt ref that does not match the order', () => {
    const recorded = recordLaborProductSettlement(okPlan(), {
      _tag: 'settled',
      receiptRef,
      outcome: { metered: true, receiptRef: 'receipt.someone.else' },
    })
    expect(recorded.ok).toBe(false)
    if (!recorded.ok) {
      expect(recorded.error.reason).toContain('does not match')
    }
  })

  test('rejects settling an order that was never delivered', () => {
    const ordered = okPlan({
      stage: 'ordered',
      workerRef: null,
      artifactRef: null,
    })
    const recorded = recordLaborProductSettlement(ordered, meteredResult)
    expect(recorded.ok).toBe(false)
    if (!recorded.ok) {
      expect(recorded.error.reason).toContain('delivered')
    }
  })

  test('closes the full carry-through: order -> dispatch -> deliver -> settle -> receipt', () => {
    const ordered = planSelfServeLaborProductOrder(
      { orderId: 'order-1', buyerRef: 'agent:buyer', listing },
      { createdAt: '2026-06-20T00:00:00.000Z' },
    )
    expect(ordered.ok).toBe(true)
    if (!ordered.ok) return
    const dispatched = advanceLaborProductFlow(ordered.plan, {
      kind: 'dispatch',
      workerRef: 'agent:worker',
    })
    expect(dispatched.ok).toBe(true)
    if (!dispatched.ok) return
    const delivered = advanceLaborProductFlow(dispatched.plan, {
      kind: 'deliver',
      artifactRef: 'artifact.repo_triage.order-1',
    })
    expect(delivered.ok).toBe(true)
    if (!delivered.ok) return

    const recorded = recordLaborProductSettlement(delivered.plan, meteredResult)
    expect(recorded.ok).toBe(true)
    if (!recorded.ok) return
    expect(recorded.plan.stage).toBe('settled')
    expect(recorded.receipt.receiptRef).toBe(
      delivered.plan.settlement.receiptRef,
    )
  })
})

describe('settlement-receipt dereference (read-only, INERT)', () => {
  const receiptRef = laborProductOrderReceiptRef('order-1')
  const mint = () => {
    const recorded = recordLaborProductSettlement(okPlan(), {
      _tag: 'settled',
      receiptRef,
      outcome: { metered: true, receiptRef },
    })
    if (!recorded.ok) throw new Error(recorded.error.reason)
    return recorded.receipt
  }

  test('dereferences a published receipt by its receiptRef', () => {
    const store = makeInMemoryLaborProductReceiptStore([mint()])
    const found = readLaborProductSettlementReceipt(store, receiptRef)
    expect(found?.receiptRef).toBe(receiptRef)
    expect(found?.settled).toBe(true)
    expect(found?.streamKind).toBe('labor')
    expect(found?.promiseState).toBe('yellow')
  })

  test('returns null for an unknown receiptRef', () => {
    const store = makeInMemoryLaborProductReceiptStore([mint()])
    expect(readLaborProductSettlementReceipt(store, 'receipt.unknown')).toBeNull()
  })

  test('empty production store dereferences nothing (INERT)', () => {
    const store = makeInMemoryLaborProductReceiptStore([])
    expect(readLaborProductSettlementReceipt(store, receiptRef)).toBeNull()
  })

  test('projection is honest: yellow, live_at_read, uncleared blocker surfaced', () => {
    const projection = listLaborProductSettlementReceipts(
      makeInMemoryLaborProductReceiptStore([mint()]),
    )
    expect(projection.promiseState).toBe('yellow')
    expect(projection.maxStalenessSeconds).toBe(0)
    expect(projection.unclearedBlockerRefs).toContain(
      'blocker.product_promises.agentic_labor_product_real_sale_receipt_missing',
    )
    expect(projection.receipts.map(r => r.receiptRef)).toEqual([receiptRef])
  })
})
