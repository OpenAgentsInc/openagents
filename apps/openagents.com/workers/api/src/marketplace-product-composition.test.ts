import { describe, expect, test } from 'vitest'

import {
  MARKETPLACE_COMPOSE_AND_LIST_PROMISE,
  MARKETPLACE_PRODUCT_COMPOSITION_SCHEMA,
  MARKETPLACE_PRODUCT_LIFECYCLE_SCHEMA,
  type ComposedProductDefinition,
  assembleComposedProductForListing,
  buildComposedProductDefinition,
  composedProductMonetizableLayers,
  composedProductPrimitives,
  installComposedProduct,
  makeInMemoryComposedProductLifecycleStore,
  listComposedProducts,
  makeInMemoryComposedProductListingStore,
  recordComposedProductUse,
  readComposedProduct,
  selfServeListComposedProduct,
} from './marketplace-product-composition'

const okDefinition = (
  overrides: Partial<Parameters<typeof buildComposedProductDefinition>[0]> = {},
): ComposedProductDefinition => {
  const result = buildComposedProductDefinition({
    productId: 'prod_research_pack',
    definitionVersion: 1,
    builderRef: 'agent:raynor',
    title: 'Research + sandbox pack',
    summary: 'Inference and a sandbox composed into one product.',
    components: [
      { primitive: 'inference', capabilityRef: 'inference.gateway_credits_business.v1' },
      { primitive: 'sandbox', capabilityRef: 'cloud.sandbox_compute_service.v1' },
    ],
    createdAt: '2026-06-19T00:00:00.000Z',
    ...overrides,
  })
  if (!result.ok) {
    throw new Error(`expected ok definition: ${result.error.reason}`)
  }
  return result.definition
}

describe('compose-and-list product definition model (#5515)', () => {
  test('builds a typed definition pinned to the planned promise', () => {
    const definition = okDefinition()
    expect(definition.schema).toBe(MARKETPLACE_PRODUCT_COMPOSITION_SCHEMA)
    expect(definition.promiseId).toBe(MARKETPLACE_COMPOSE_AND_LIST_PROMISE)
    expect(definition.listingState).toBe('draft')
    expect(definition.components).toHaveLength(2)
  })

  test('requires at least one composed primitive', () => {
    const result = buildComposedProductDefinition({
      productId: 'prod_empty',
      definitionVersion: 1,
      builderRef: 'agent:raynor',
      title: 'Empty',
      summary: 'no components',
      components: [],
    })
    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.error.reason).toContain('at least one primitive')
    }
  })

  test('rejects empty productId / builderRef / title and non-positive version', () => {
    for (const overrides of [
      { productId: '  ' },
      { builderRef: '' },
      { title: '' },
      { definitionVersion: 0 },
      { definitionVersion: 1.5 },
    ] as const) {
      const result = buildComposedProductDefinition({
        productId: 'prod',
        definitionVersion: 1,
        builderRef: 'agent:raynor',
        title: 'Title',
        summary: 'summary',
        components: [{ primitive: 'inference', capabilityRef: 'cap' }],
        ...overrides,
      })
      expect(result.ok).toBe(false)
    }
  })

  test('rejects a component with an empty capabilityRef', () => {
    const result = buildComposedProductDefinition({
      productId: 'prod',
      definitionVersion: 1,
      builderRef: 'agent:raynor',
      title: 'Title',
      summary: 'summary',
      components: [{ primitive: 'data', capabilityRef: '   ' }],
    })
    expect(result.ok).toBe(false)
  })

  test('composedProductPrimitives dedupes', () => {
    const definition = okDefinition({
      components: [
        { primitive: 'inference', capabilityRef: 'a' },
        { primitive: 'inference', capabilityRef: 'b' },
        { primitive: 'sandbox', capabilityRef: 'c' },
      ],
    })
    expect(composedProductPrimitives(definition)).toEqual(['inference', 'sandbox'])
  })

  test('composedProductMonetizableLayers pairs each distinct layer with its first capabilityRef (#5518 seam)', () => {
    const definition = okDefinition({
      components: [
        { primitive: 'inference', capabilityRef: 'inference.gateway.v1' },
        { primitive: 'inference', capabilityRef: 'inference.second.v1' },
        { primitive: 'sandbox', capabilityRef: 'cloud.sandbox.v1' },
      ],
    })
    expect(composedProductMonetizableLayers(definition)).toEqual([
      { layer: 'inference', capabilityRef: 'inference.gateway.v1' },
      { layer: 'sandbox', capabilityRef: 'cloud.sandbox.v1' },
    ])
  })
})

describe('compose-and-list runtime core (#6882)', () => {
  test('assembles a builder definition into an inert listed product with attribution', () => {
    const assembled = assembleComposedProductForListing({
      productId: 'prod_research_pack',
      definitionVersion: 1,
      builderRef: 'agent:raynor',
      title: 'Research + sandbox pack',
      summary: 'Inference and a sandbox composed into one product.',
      components: [
        {
          primitive: 'inference',
          capabilityRef: 'inference.gateway_credits_business.v1',
        },
        { primitive: 'sandbox', capabilityRef: 'cloud.sandbox_compute_service.v1' },
      ],
      createdAt: '2026-06-19T00:00:00.000Z',
      assembledAt: '2026-06-20T00:00:00.000Z',
    })
    expect(assembled.ok).toBe(true)
    if (!assembled.ok) {
      throw new Error(assembled.error.reason)
    }
    expect(assembled.result.definition.listingState).toBe('listed')
    expect(assembled.result.assemblyPlan).toMatchObject({
      schema: MARKETPLACE_PRODUCT_COMPOSITION_SCHEMA,
      status: 'assembled',
      inert: true,
      attribution: {
        builderRef: 'agent:raynor',
        attributionRef: 'marketplace.builder:prod_research_pack:1:agent:raynor',
      },
    })
  })

  test('persists self-serve listing, install, and use lifecycle evidence', () => {
    const store = makeInMemoryComposedProductLifecycleStore()
    const listed = selfServeListComposedProduct(store, {
      productId: 'prod_research_pack',
      definitionVersion: 1,
      builderRef: 'agent:raynor',
      title: 'Research + sandbox pack',
      summary: 'Inference and a sandbox composed into one product.',
      components: [
        {
          primitive: 'inference',
          capabilityRef: 'inference.gateway_credits_business.v1',
        },
      ],
      createdAt: '2026-06-19T00:00:00.000Z',
      assembledAt: '2026-06-20T00:00:00.000Z',
    })
    expect(listed.ok).toBe(true)
    expect(listComposedProducts(store).products.map(product => product.productId)).toEqual([
      'prod_research_pack',
    ])

    const installed = installComposedProduct(store, {
      productId: 'prod_research_pack',
      buyerRef: 'buyer:tychus',
      installedAt: '2026-06-21T00:00:00.000Z',
    })
    expect(installed.ok).toBe(true)
    if (!installed.ok) {
      throw new Error(installed.error.reason)
    }
    expect(installed.row).toMatchObject({
      schema: MARKETPLACE_PRODUCT_LIFECYCLE_SCHEMA,
      evidenceKind: 'marketplace_composed_product_install',
      status: 'installed',
      inert: true,
      buyerRef: 'buyer:tychus',
      builderRef: 'agent:raynor',
      attributionRef: 'marketplace.builder:prod_research_pack:1:agent:raynor',
    })

    const used = recordComposedProductUse(store, {
      lifecycleRef: installed.row.lifecycleRef,
      usedAt: '2026-06-22T00:00:00.000Z',
    })
    expect(used.ok).toBe(true)
    if (!used.ok) {
      throw new Error(used.error.reason)
    }
    expect(used.row).toMatchObject({
      schema: MARKETPLACE_PRODUCT_LIFECYCLE_SCHEMA,
      evidenceKind: 'marketplace_composed_product_usage',
      status: 'used',
      inert: true,
      buyerRef: 'buyer:tychus',
      builderRef: 'agent:raynor',
      attributionRef: 'marketplace.builder:prod_research_pack:1:agent:raynor',
    })
    expect(store.listInstallEvidence()).toHaveLength(1)
    expect(store.listUsageEvidence()).toHaveLength(1)
  })

  test('blocks install before listing and use before install', () => {
    const store = makeInMemoryComposedProductLifecycleStore([
      okDefinition({ productId: 'p_draft', listingState: 'draft' }),
    ])
    const install = installComposedProduct(store, {
      productId: 'p_draft',
      buyerRef: 'buyer:tychus',
    })
    expect(install.ok).toBe(false)
    if (!install.ok) {
      expect(install.error.reason).toContain('listed before install')
    }

    const use = recordComposedProductUse(store, {
      lifecycleRef: 'marketplace.install:missing:buyer',
    })
    expect(use.ok).toBe(false)
    if (!use.ok) {
      expect(use.error.reason).toContain('install lifecycle')
    }
  })
})

describe('compose-and-list listing surface (#5515)', () => {
  test('listing reports inert/planned and only surfaces listed products', () => {
    const store = makeInMemoryComposedProductListingStore([
      okDefinition({ productId: 'p_draft', listingState: 'draft' }),
      okDefinition({ productId: 'p_listed', listingState: 'listed' }),
      okDefinition({ productId: 'p_unlisted', listingState: 'unlisted' }),
    ])
    const projection = listComposedProducts(store)
    expect(projection.inert).toBe(true)
    expect(projection.promiseState).toBe('planned')
    expect(projection.products.map(p => p.productId)).toEqual(['p_listed'])
  })

  test('readComposedProduct returns a listed product or null', () => {
    const store = makeInMemoryComposedProductListingStore([
      okDefinition({ productId: 'p_listed', listingState: 'listed' }),
      okDefinition({ productId: 'p_draft', listingState: 'draft' }),
    ])
    expect(readComposedProduct(store, 'p_listed')?.productId).toBe('p_listed')
    expect(readComposedProduct(store, 'p_draft')).toBeNull()
    expect(readComposedProduct(store, 'missing')).toBeNull()
  })
})
