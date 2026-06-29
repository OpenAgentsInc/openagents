import { describe, expect, test } from 'vitest'

import {
  MARKETPLACE_COMPOSE_AND_LIST_PROMISE,
  MARKETPLACE_PRODUCT_COMPOSITION_SCHEMA,
  assembleComposedProduct,
  type ComposedProductDefinition,
  buildComposedProductDefinition,
  composedProductMonetizableLayers,
  composedProductPrimitives,
  listComposedProducts,
  makeInMemoryComposedProductListingStore,
  recordComposedProductInstallUse,
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

  test('assembles, self-serve lists, and records buyer install/use with builder attribution (#6882)', () => {
    const definition = okDefinition()
    const assembly = assembleComposedProduct(definition, {
      assembledAt: '2026-06-29T00:00:00.000Z',
    })
    expect(assembly.productId).toBe(definition.productId)
    expect(assembly.fulfillmentMode).toBe('no_spend_public_fixture')
    expect(assembly.billingState).toBe('not_configured')
    expect(assembly.primitiveRefs).toEqual([
      'primitive.public.inference',
      'primitive.public.sandbox',
    ])
    expect(assembly.builderAttribution).toEqual({
      builderRef: 'agent:raynor',
      attributionRef:
        'attribution.public.marketplace_composed_product.prod_research_pack.1',
      revSharePolicyRef:
        'revshare.policy.public.marketplace_composed_product.pending_billing',
    })

    const listed = selfServeListComposedProduct(definition, assembly, {
      listedAt: '2026-06-29T00:01:00.000Z',
    })
    expect(listed.ok).toBe(true)
    if (!listed.ok) {
      throw new Error(`expected listed product: ${listed.error.reason}`)
    }
    expect(listed.definition.listingState).toBe('listed')
    expect(listed.listingReceipt.selfServe).toBe(true)
    expect(listed.listingReceipt.builderAttribution).toEqual(
      assembly.builderAttribution,
    )

    const lifecycle = recordComposedProductInstallUse(
      listed.listingReceipt,
      {
        buyerRef: 'buyer:public-fixture',
        installedAt: '2026-06-29T00:02:00.000Z',
        usedAt: '2026-06-29T00:03:00.000Z',
      },
    )
    expect(lifecycle.ok).toBe(true)
    if (lifecycle.ok) {
      expect(lifecycle.lifecycleReceipt.installState).toBe('used')
      expect(lifecycle.lifecycleReceipt.billingState).toBe('not_configured')
      expect(lifecycle.lifecycleReceipt.settlementState).toBe('not_applicable')
      expect(lifecycle.lifecycleReceipt.builderAttribution).toEqual(
        assembly.builderAttribution,
      )
    }
  })

  test('install/use lifecycle requires a buyer ref', () => {
    const definition = okDefinition()
    const assembly = assembleComposedProduct(definition)
    const listed = selfServeListComposedProduct(definition, assembly)
    expect(listed.ok).toBe(true)
    if (!listed.ok) {
      throw new Error(`expected listed product: ${listed.error.reason}`)
    }

    const lifecycle = recordComposedProductInstallUse(listed.listingReceipt, {
      buyerRef: ' ',
    })
    expect(lifecycle.ok).toBe(false)
    if (!lifecycle.ok) {
      expect(lifecycle.error.reason).toContain('buyerRef')
    }
  })

  test('self-serve listing requires the assembly receipt to match the definition', () => {
    const definition = okDefinition()
    const otherDefinition = okDefinition({
      productId: 'prod_other_pack',
      definitionVersion: 2,
    })
    const otherAssembly = assembleComposedProduct(otherDefinition)

    const listed = selfServeListComposedProduct(definition, otherAssembly)
    expect(listed.ok).toBe(false)
    if (!listed.ok) {
      expect(listed.error.reason).toContain('assembly receipt')
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
