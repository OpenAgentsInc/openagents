import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  buildLaborProductFlowPlan,
  makeInMemoryLaborProductFlowStore,
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

  test('rejects non-GET', async () => {
    const response = await Effect.runPromise(
      handleAgenticLaborProductApi(
        new Request(`https://openagents.com${AgenticLaborProductEndpoint}`, {
          method: 'POST',
        }),
        { enabled: false },
      ),
    )
    expect(response.status).toBe(405)
  })
})
