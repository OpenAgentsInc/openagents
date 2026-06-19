import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  MarketplaceComposeListEndpoint,
  handleMarketplaceCompositionApi,
  isMarketplaceComposeAndListEnabled,
} from './marketplace-composition-routes'
import {
  buildComposedProductDefinition,
  makeInMemoryComposedProductListingStore,
} from './marketplace-product-composition'

const listedStore = () => {
  const result = buildComposedProductDefinition({
    productId: 'p_listed',
    definitionVersion: 1,
    builderRef: 'agent:raynor',
    title: 'Listed product',
    summary: 'a listed composed product',
    components: [{ primitive: 'inference', capabilityRef: 'cap' }],
    listingState: 'listed',
  })
  if (!result.ok) {
    throw new Error(result.error.reason)
  }
  return makeInMemoryComposedProductListingStore([result.definition])
}

const request = (suffix = '') =>
  new Request(`https://openagents.com${MarketplaceComposeListEndpoint}${suffix}`)

describe('marketplace compose-and-list flag (#5515)', () => {
  test('flag defaults OFF', () => {
    expect(isMarketplaceComposeAndListEnabled(undefined)).toBe(false)
    expect(isMarketplaceComposeAndListEnabled('false')).toBe(false)
    expect(isMarketplaceComposeAndListEnabled('0')).toBe(false)
    expect(isMarketplaceComposeAndListEnabled('on')).toBe(true)
    expect(isMarketplaceComposeAndListEnabled('TRUE')).toBe(true)
  })
})

describe('marketplace compose-and-list route (#5515)', () => {
  test('is INERT (empty list) when disabled, even with a populated store', async () => {
    const response = await Effect.runPromise(
      handleMarketplaceCompositionApi(request(), {
        enabled: false,
        store: listedStore(),
      }),
    )
    const body = (await response.json()) as {
      inert: boolean
      promiseState: string
      products: ReadonlyArray<unknown>
    }
    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.inert).toBe(true)
    expect(body.promiseState).toBe('planned')
    expect(body.products).toHaveLength(0)
  })

  test('lists products when armed, still reporting inert/planned', async () => {
    const response = await Effect.runPromise(
      handleMarketplaceCompositionApi(request(), {
        enabled: true,
        store: listedStore(),
      }),
    )
    const body = (await response.json()) as {
      inert: boolean
      promiseState: string
      products: ReadonlyArray<{ productId: string }>
    }
    expect(body.inert).toBe(true)
    expect(body.promiseState).toBe('planned')
    expect(body.products.map(p => p.productId)).toEqual(['p_listed'])
  })

  test('reads a single product by id', async () => {
    const response = await Effect.runPromise(
      handleMarketplaceCompositionApi(request('?productId=p_listed'), {
        enabled: true,
        store: listedStore(),
      }),
    )
    const body = (await response.json()) as {
      product: { productId: string } | null
    }
    expect(body.product?.productId).toBe('p_listed')
  })

  test('returns null product for a missing id', async () => {
    const response = await Effect.runPromise(
      handleMarketplaceCompositionApi(request('?productId=missing'), {
        enabled: true,
        store: listedStore(),
      }),
    )
    const body = (await response.json()) as { product: unknown }
    expect(body.product).toBeNull()
  })

  test('rejects non-GET', async () => {
    const response = await Effect.runPromise(
      handleMarketplaceCompositionApi(
        new Request(`https://openagents.com${MarketplaceComposeListEndpoint}`, {
          method: 'POST',
        }),
        { enabled: false },
      ),
    )
    expect(response.status).toBe(405)
  })
})
