// Public read-only listing surface for the compose-and-list marketplace MVP
// (EPIC #5510, child #5515; promise marketplace.compose_and_list_products.v1).
//
// INERT by default. The route is wired into the live Worker but reads from an
// injected listing store that the Worker leaves EMPTY unless the marketplace
// flag is explicitly armed (MARKETPLACE_COMPOSE_AND_LIST_ENABLED). Either way
// the response is honest: `inert: true` and `promiseState: 'planned'`, with NO
// billing, fulfillment, or live-product claim. Read-only (GET only).

import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  ComposedProductListingStaleness,
  type ComposedProductListingStore,
  emptyComposedProductListingStore,
  listComposedProducts,
  readComposedProduct,
} from './marketplace-product-composition'
import { currentIsoTimestamp } from './runtime-primitives'

export const MarketplaceComposeListEndpoint =
  '/api/public/marketplace/composed-products'

// Parse the MARKETPLACE_COMPOSE_AND_LIST_ENABLED flag. Default OFF: anything
// other than an explicit truthy token leaves the surface inert (empty store).
export const isMarketplaceComposeAndListEnabled = (
  value: string | undefined,
): boolean => {
  if (value === undefined) {
    return false
  }
  return ['1', 'true', 'yes', 'on'].includes(value.trim().toLowerCase())
}

export type MarketplaceCompositionDeps = Readonly<{
  // Whether the marketplace surface is armed. When false (default) the Worker
  // passes the empty store, so the listing is inert.
  enabled: boolean
  // The listing store. The Worker passes the empty store while INERT.
  store?: ComposedProductListingStore
}>

const resolveStore = (
  deps: MarketplaceCompositionDeps,
): ComposedProductListingStore =>
  deps.enabled && deps.store !== undefined
    ? deps.store
    : emptyComposedProductListingStore

/**
 * GET the composed-products listing. Read-only. Optional `?productId=` reads a
 * single listed product (returns `product: null` when absent/unlisted).
 */
export const handleMarketplaceCompositionApi = (
  request: Request,
  deps: MarketplaceCompositionDeps,
): Effect.Effect<Response> => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  const store = resolveStore(deps)
  const url = new URL(request.url)
  const productId = url.searchParams.get('productId')

  if (productId !== null && productId.trim().length > 0) {
    return Effect.succeed(
      noStoreJsonResponse({
        schema: 'openagents.marketplace_product_composition.v1',
        promiseId: 'marketplace.compose_and_list_products.v1',
        promiseState: 'planned',
        inert: true,
        generatedAt: currentIsoTimestamp(),
        maxStalenessSeconds: ComposedProductListingStaleness.maxStalenessSeconds,
        staleness: ComposedProductListingStaleness,
        product: readComposedProduct(store, productId),
      }),
    )
  }

  return Effect.succeed(noStoreJsonResponse(listComposedProducts(store)))
}
