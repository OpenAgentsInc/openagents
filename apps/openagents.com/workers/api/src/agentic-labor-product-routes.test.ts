import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  buildLaborProductFlowPlan,
  laborProductOrderReceiptRef,
  makeInMemoryLaborProductFlowStore,
  makeInMemoryLaborProductReceiptStore,
  recordLaborProductSettlement,
  type LaborProductListing,
} from './agentic-labor-product'
import {
  AgenticLaborProductEndpoint,
  handleAgenticLaborProductApi,
  isAgenticLaborProductsEnabled,
} from './agentic-labor-product-routes'

const listing: LaborProductListing = {
  listingId: 'listing-1',
  sellerRef: 'agent:raynor',
  title: 'Repo triage labor product',
  summary: 'Triage one repo backlog and deliver a report.',
  capabilityRef: 'promise:autopilot.agentic_labor_products.v1',
  priceSats: 100,
}

const flowStore = () => {
  const result = buildLaborProductFlowPlan({
    orderId: 'order-1',
    buyerRef: 'agent:buyer',
    listing,
    stage: 'delivered',
    workerRef: 'agent:worker',
    artifactRef: 'artifact.repo_triage.order-1',
  })
  if (!result.ok) {
    throw new Error(result.error.reason)
  }
  return makeInMemoryLaborProductFlowStore([result.plan])
}

const receiptRef = laborProductOrderReceiptRef('order-1')

const receiptStore = () => {
  const built = buildLaborProductFlowPlan({
    orderId: 'order-1',
    buyerRef: 'agent:buyer',
    listing,
    stage: 'delivered',
    workerRef: 'agent:worker',
    artifactRef: 'artifact.repo_triage.order-1',
  })
  if (!built.ok) {
    throw new Error(built.error.reason)
  }
  const recorded = recordLaborProductSettlement(built.plan, {
    _tag: 'settled',
    receiptRef,
    outcome: { metered: true, receiptRef },
  })
  if (!recorded.ok) {
    throw new Error(recorded.error.reason)
  }
  return makeInMemoryLaborProductReceiptStore([recorded.receipt])
}

const request = (suffix = '') =>
  new Request(`https://openagents.com${AgenticLaborProductEndpoint}${suffix}`)

describe('agentic labor-product flag', () => {
  test('flag defaults OFF', () => {
    expect(isAgenticLaborProductsEnabled(undefined)).toBe(false)
    expect(isAgenticLaborProductsEnabled('false')).toBe(false)
    expect(isAgenticLaborProductsEnabled('0')).toBe(false)
    expect(isAgenticLaborProductsEnabled('on')).toBe(true)
    expect(isAgenticLaborProductsEnabled('TRUE')).toBe(true)
  })
})

describe('agentic labor-product route', () => {
  test('is INERT (empty list) when disabled, even with a populated store', async () => {
    const response = await Effect.runPromise(
      handleAgenticLaborProductApi(request(), {
        enabled: false,
        store: flowStore(),
      }),
    )
    const body = (await response.json()) as {
      inert: boolean
      promiseState: string
      flows: ReadonlyArray<unknown>
    }
    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.inert).toBe(true)
    expect(body.promiseState).toBe('yellow')
    expect(body.flows).toHaveLength(0)
  })

  test('lists flows when armed, still reporting inert/yellow', async () => {
    const response = await Effect.runPromise(
      handleAgenticLaborProductApi(request(), {
        enabled: true,
        store: flowStore(),
      }),
    )
    const body = (await response.json()) as {
      inert: boolean
      promiseState: string
      promiseIds: ReadonlyArray<string>
      flows: ReadonlyArray<{ orderId: string }>
    }
    expect(body.inert).toBe(true)
    expect(body.promiseState).toBe('yellow')
    expect(body.promiseIds).toEqual(['autopilot.agentic_labor_products.v1'])
    expect(body.flows.map(f => f.orderId)).toEqual(['order-1'])
  })

  test('reads a single flow by order id', async () => {
    const response = await Effect.runPromise(
      handleAgenticLaborProductApi(request('?orderId=order-1'), {
        enabled: true,
        store: flowStore(),
      }),
    )
    const body = (await response.json()) as {
      inert: boolean
      flow: { orderId: string } | null
    }
    expect(body.inert).toBe(true)
    expect(body.flow?.orderId).toBe('order-1')
  })

  test('returns null flow for a missing id', async () => {
    const response = await Effect.runPromise(
      handleAgenticLaborProductApi(request('?orderId=missing'), {
        enabled: true,
        store: flowStore(),
      }),
    )
    const body = (await response.json()) as { flow: unknown }
    expect(body.flow).toBeNull()
  })

  test('dereferences a published settlement receipt by receiptRef when armed', async () => {
    const response = await Effect.runPromise(
      handleAgenticLaborProductApi(
        request(`?receiptRef=${encodeURIComponent(receiptRef)}`),
        { enabled: true, receiptStore: receiptStore() },
      ),
    )
    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    const body = (await response.json()) as {
      promiseState: string
      unclearedBlockerRefs: ReadonlyArray<string>
      receipt: { receiptRef: string; settled: boolean; streamKind: string } | null
    }
    expect(body.promiseState).toBe('yellow')
    expect(body.receipt?.receiptRef).toBe(receiptRef)
    expect(body.receipt?.settled).toBe(true)
    expect(body.receipt?.streamKind).toBe('labor')
    expect(body.unclearedBlockerRefs).toEqual([
      'blocker.product_promises.agentic_labor_product_real_sale_receipt_missing',
    ])
  })

  test('returns null receipt for an unknown receiptRef', async () => {
    const response = await Effect.runPromise(
      handleAgenticLaborProductApi(request('?receiptRef=receipt.unknown'), {
        enabled: true,
        receiptStore: receiptStore(),
      }),
    )
    const body = (await response.json()) as { receipt: unknown }
    expect(body.receipt).toBeNull()
  })

  test('is INERT for receiptRef when disabled (empty receipt store)', async () => {
    const response = await Effect.runPromise(
      handleAgenticLaborProductApi(
        request(`?receiptRef=${encodeURIComponent(receiptRef)}`),
        { enabled: false, receiptStore: receiptStore() },
      ),
    )
    const body = (await response.json()) as { receipt: unknown }
    expect(body.receipt).toBeNull()
  })

  test('rejects an unsupported method (DELETE)', async () => {
    const response = await Effect.runPromise(
      handleAgenticLaborProductApi(
        new Request(`https://openagents.com${AgenticLaborProductEndpoint}`, {
          method: 'DELETE',
        }),
        { enabled: false },
      ),
    )
    expect(response.status).toBe(405)
    expect(response.headers.get('allow')).toContain('POST')
  })
})

describe('agentic labor-product self-serve POST', () => {
  const postRequest = (body: unknown) =>
    new Request(`https://openagents.com${AgenticLaborProductEndpoint}`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(body),
    })

  const orderBody = {
    orderId: 'order-self-1',
    buyerRef: 'agent:buyer',
    listing,
  }

  test('POST returns 503 when the flag is disabled (not armed)', async () => {
    const response = await Effect.runPromise(
      handleAgenticLaborProductApi(postRequest(orderBody), { enabled: false }),
    )
    expect(response.status).toBe(503)
    const body = (await response.json()) as {
      error: string
      inert: boolean
      promiseState: string
    }
    expect(body.error).toBe('agentic_labor_products_disabled')
    expect(body.inert).toBe(true)
    expect(body.promiseState).toBe('yellow')
  })

  test('POST self-serve plans an ordered-stage flow when armed, still inert/yellow', async () => {
    const response = await Effect.runPromise(
      handleAgenticLaborProductApi(postRequest(orderBody), { enabled: true }),
    )
    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    const body = (await response.json()) as {
      inert: boolean
      promiseState: string
      promiseIds: ReadonlyArray<string>
      unclearedBlockerRefs: ReadonlyArray<string>
      flow: {
        orderId: string
        stage: string
        workerRef: string | null
        artifactRef: string | null
        settlement: { streamKind: string }
      }
    }
    expect(body.inert).toBe(true)
    expect(body.promiseState).toBe('yellow')
    expect(body.promiseIds).toEqual(['autopilot.agentic_labor_products.v1'])
    expect(body.flow.orderId).toBe('order-self-1')
    expect(body.flow.stage).toBe('ordered')
    expect(body.flow.workerRef).toBeNull()
    expect(body.flow.artifactRef).toBeNull()
    expect(body.flow.settlement.streamKind).toBe('labor')
    // The self-serve path exists now; the real-sale-receipt blocker stays.
    expect(body.unclearedBlockerRefs).toEqual([
      'blocker.product_promises.agentic_labor_product_real_sale_receipt_missing',
    ])
  })

  test('POST rejects an invalid body with 400 when armed', async () => {
    const response = await Effect.runPromise(
      handleAgenticLaborProductApi(
        postRequest({ orderId: '', buyerRef: 'agent:buyer', listing }),
        { enabled: true },
      ),
    )
    expect(response.status).toBe(400)
    const body = (await response.json()) as { error: string; inert: boolean }
    expect(body.error).toBe('invalid_request')
    expect(body.inert).toBe(true)
  })

  test('POST rejects non-JSON with 400 when armed', async () => {
    const response = await Effect.runPromise(
      handleAgenticLaborProductApi(
        new Request(`https://openagents.com${AgenticLaborProductEndpoint}`, {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: 'not json',
        }),
        { enabled: true },
      ),
    )
    expect(response.status).toBe(400)
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('invalid_request')
  })
})
