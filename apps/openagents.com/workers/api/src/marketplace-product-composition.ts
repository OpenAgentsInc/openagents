// Compose-and-list marketplace MVP — typed product-composition model + a pure
// listing surface (EPIC #5510, child #5515; promise
// marketplace.compose_and_list_products.v1).
//
// Episode 239 ("Let's Make Money", docs/transcripts/239.md): agents and their
// humans COMPOSE the OpenAgents primitives (inference / fine-tuning / training /
// agentic-work / sandbox / data) and the open markets into products they list
// for sale.
//
// SCOPE / HONESTY: this is an INERT scaffold toward that marketplace, NOT a
// paid marketplace claim. It is PURE:
//   - it moves no money, runs no fulfillment, reads no wallet, writes no
//     receipt, and provisions no primitive;
//   - it defines a typed, versioned product definition plus bounded no-spend
//     assemble/list/install-use lifecycle receipts over public-safe refs.
// The promise marketplace.compose_and_list_products.v1 STAYS `planned`. Nothing
// here flips it green: there is no billing, paid sale receipt, rev-share
// settlement, or live primitive provisioning. A green flip stays receipt-first
// and owner-signed per proof.claim_upgrade_receipts.v1.

import { Schema as S } from 'effect'

import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

export const MARKETPLACE_PRODUCT_COMPOSITION_SCHEMA =
  'openagents.marketplace_product_composition.v1' as const

export const MARKETPLACE_COMPOSE_AND_LIST_PROMISE =
  'marketplace.compose_and_list_products.v1' as const

/**
 * The OpenAgents primitives + open-market capabilities a product can be
 * composed from. These are the Episode 239 layers; each is gated by its own
 * promise record and is NOT made live by being nameable here.
 */
export const MarketplaceComposablePrimitive = S.Literals([
  'inference',
  'fine_tuning',
  'training',
  'agentic_work',
  'sandbox',
  'data',
  'market_compute',
  'market_data',
  'market_labor',
  'market_liquidity',
  'market_risk',
  'market_verification',
])
export type MarketplaceComposablePrimitive =
  typeof MarketplaceComposablePrimitive.Type

/** Listing lifecycle state. INERT: only `draft`/`listed` are reachable here. */
export const MarketplaceListingState = S.Literals([
  'draft',
  'listed',
  'unlisted',
])
export type MarketplaceListingState = typeof MarketplaceListingState.Type

/**
 * One primitive/market capability the product composes, with the bounded,
 * neutral capability ref it points at (e.g. a promise id or capability id).
 */
export const MarketplaceComponentRef = S.Struct({
  primitive: MarketplaceComposablePrimitive,
  capabilityRef: S.String,
})
export type MarketplaceComponentRef = typeof MarketplaceComponentRef.Type

/**
 * A versioned product definition: a composition of ≥1 primitive/market
 * capability that a builder lists for sale. Definition only — no fulfillment.
 */
export const ComposedProductDefinition = S.Struct({
  schema: S.Literal(MARKETPLACE_PRODUCT_COMPOSITION_SCHEMA),
  /** Stable product id. */
  productId: S.String,
  /** Monotonic definition version for this product id. */
  definitionVersion: S.Number,
  /** Neutral builder ref (agent/user ref); no name is required. */
  builderRef: S.String,
  title: S.String,
  summary: S.String,
  components: S.Array(MarketplaceComponentRef),
  listingState: MarketplaceListingState,
  /** Always the planned promise — the listing makes NO live-product claim. */
  promiseId: S.Literal(MARKETPLACE_COMPOSE_AND_LIST_PROMISE),
  createdAt: S.String,
})
export type ComposedProductDefinition = typeof ComposedProductDefinition.Type

export const MarketplaceBuilderAttribution = S.Struct({
  builderRef: S.String,
  attributionRef: S.String,
  revSharePolicyRef: S.String,
})
export type MarketplaceBuilderAttribution =
  typeof MarketplaceBuilderAttribution.Type

export const ComposedProductAssemblyReceipt = S.Struct({
  schema: S.Literal(MARKETPLACE_PRODUCT_COMPOSITION_SCHEMA),
  promiseId: S.Literal(MARKETPLACE_COMPOSE_AND_LIST_PROMISE),
  assemblyRef: S.String,
  productId: S.String,
  definitionVersion: S.Number,
  builderAttribution: MarketplaceBuilderAttribution,
  primitiveRefs: S.Array(S.String),
  assembledAt: S.String,
  fulfillmentMode: S.Literal('no_spend_public_fixture'),
  billingState: S.Literal('not_configured'),
})
export type ComposedProductAssemblyReceipt =
  typeof ComposedProductAssemblyReceipt.Type

export const ComposedProductListingWriteReceipt = S.Struct({
  schema: S.Literal(MARKETPLACE_PRODUCT_COMPOSITION_SCHEMA),
  promiseId: S.Literal(MARKETPLACE_COMPOSE_AND_LIST_PROMISE),
  listingRef: S.String,
  assemblyRef: S.String,
  productId: S.String,
  builderAttribution: MarketplaceBuilderAttribution,
  listedAt: S.String,
  listingState: S.Literal('listed'),
  selfServe: S.Literal(true),
  billingState: S.Literal('not_configured'),
})
export type ComposedProductListingWriteReceipt =
  typeof ComposedProductListingWriteReceipt.Type

export const ComposedProductBuyerLifecycleReceipt = S.Struct({
  schema: S.Literal(MARKETPLACE_PRODUCT_COMPOSITION_SCHEMA),
  promiseId: S.Literal(MARKETPLACE_COMPOSE_AND_LIST_PROMISE),
  lifecycleRef: S.String,
  listingRef: S.String,
  productId: S.String,
  buyerRef: S.String,
  builderAttribution: MarketplaceBuilderAttribution,
  installedAt: S.String,
  usedAt: S.String,
  installState: S.Literal('used'),
  billingState: S.Literal('not_configured'),
  settlementState: S.Literal('not_applicable'),
})
export type ComposedProductBuyerLifecycleReceipt =
  typeof ComposedProductBuyerLifecycleReceipt.Type

export class ComposedProductValidationError extends S.TaggedErrorClass<ComposedProductValidationError>()(
  'ComposedProductValidationError',
  {
    reason: S.String,
  },
) {}

const isNonEmpty = (value: string): boolean => value.trim().length > 0

const defaultAttribution = (
  definition: ComposedProductDefinition,
): MarketplaceBuilderAttribution => ({
  builderRef: definition.builderRef,
  attributionRef: `attribution.public.marketplace_composed_product.${definition.productId}.${definition.definitionVersion}`,
  revSharePolicyRef: 'revshare.policy.public.marketplace_composed_product.pending_billing',
})

/**
 * Build a typed product definition from raw input. PURE and validating:
 * - requires ≥1 component (the composition invariant);
 * - requires non-empty product id / builder / title;
 * - requires a positive definition version;
 * - pins the promise id to the planned promise so no listing can over-claim.
 */
export const buildComposedProductDefinition = (input: {
  productId: string
  definitionVersion: number
  builderRef: string
  title: string
  summary: string
  components: ReadonlyArray<MarketplaceComponentRef>
  listingState?: MarketplaceListingState
  createdAt?: string
}):
  | { ok: true; definition: ComposedProductDefinition }
  | { ok: false; error: ComposedProductValidationError } => {
  if (!isNonEmpty(input.productId)) {
    return {
      ok: false,
      error: new ComposedProductValidationError({
        reason: 'productId must be non-empty',
      }),
    }
  }
  if (!isNonEmpty(input.builderRef)) {
    return {
      ok: false,
      error: new ComposedProductValidationError({
        reason: 'builderRef must be non-empty',
      }),
    }
  }
  if (!isNonEmpty(input.title)) {
    return {
      ok: false,
      error: new ComposedProductValidationError({
        reason: 'title must be non-empty',
      }),
    }
  }
  if (!Number.isInteger(input.definitionVersion) || input.definitionVersion < 1) {
    return {
      ok: false,
      error: new ComposedProductValidationError({
        reason: 'definitionVersion must be a positive integer',
      }),
    }
  }
  if (input.components.length < 1) {
    return {
      ok: false,
      error: new ComposedProductValidationError({
        reason: 'a composed product must reference at least one primitive',
      }),
    }
  }
  for (const component of input.components) {
    if (!isNonEmpty(component.capabilityRef)) {
      return {
        ok: false,
        error: new ComposedProductValidationError({
          reason: `component for ${component.primitive} must have a non-empty capabilityRef`,
        }),
      }
    }
  }

  return {
    ok: true,
    definition: {
      schema: MARKETPLACE_PRODUCT_COMPOSITION_SCHEMA,
      productId: input.productId,
      definitionVersion: input.definitionVersion,
      builderRef: input.builderRef,
      title: input.title,
      summary: input.summary,
      components: input.components,
      listingState: input.listingState ?? 'draft',
      promiseId: MARKETPLACE_COMPOSE_AND_LIST_PROMISE,
      createdAt: input.createdAt ?? currentIsoTimestamp(),
    },
  }
}

/**
 * The set of distinct primitives a definition composes — useful for a listing
 * surface and for the monetize-any-layer seam (#5518).
 */
export const composedProductPrimitives = (
  definition: ComposedProductDefinition,
): ReadonlyArray<MarketplaceComposablePrimitive> => [
  ...new Set(definition.components.map(component => component.primitive)),
]

/**
 * One monetizable layer derived from a composed product: a (layer,
 * capabilityRef) pair the builder could attach a monetize-any-layer offer to.
 * INERT scaffolding — this is the seam (#5518) between a composed product and
 * the per-layer monetization offers, NOT an offer itself (it carries no price,
 * no referral split, and authorizes nothing). The monetize-any-layer module
 * (`marketplace-monetize-any-layer.ts`) owns building/validating real offers and
 * runs the no-resale guards; this only enumerates which layers a product exposes.
 */
export type ComposedProductMonetizableLayer = {
  layer: MarketplaceComposablePrimitive
  capabilityRef: string
}

/**
 * Enumerate the monetizable layers of a composed product: its distinct
 * primitives, each paired with the FIRST capabilityRef the product binds for
 * that primitive (a deterministic, dedup-by-primitive projection). This is the
 * compose-and-list -> monetize-any-layer bridge: a builder lists a composed
 * product, then attaches a per-layer offer to one of these layers. PURE / INERT;
 * it computes no price and authorizes no resale. The promises stay planned.
 */
export const composedProductMonetizableLayers = (
  definition: ComposedProductDefinition,
): ReadonlyArray<ComposedProductMonetizableLayer> => {
  const byLayer = new Map<MarketplaceComposablePrimitive, string>()
  for (const component of definition.components) {
    if (!byLayer.has(component.primitive)) {
      byLayer.set(component.primitive, component.capabilityRef)
    }
  }
  return [...byLayer].map(([layer, capabilityRef]) => ({ layer, capabilityRef }))
}

export const assembleComposedProduct = (
  definition: ComposedProductDefinition,
  input: {
    assemblyRef?: string
    builderAttribution?: MarketplaceBuilderAttribution
    assembledAt?: string
  } = {},
): ComposedProductAssemblyReceipt => ({
  schema: MARKETPLACE_PRODUCT_COMPOSITION_SCHEMA,
  promiseId: MARKETPLACE_COMPOSE_AND_LIST_PROMISE,
  assemblyRef:
    input.assemblyRef ??
    `assembly.public.marketplace_composed_product.${definition.productId}.${definition.definitionVersion}`,
  productId: definition.productId,
  definitionVersion: definition.definitionVersion,
  builderAttribution: input.builderAttribution ?? defaultAttribution(definition),
  primitiveRefs: composedProductPrimitives(definition).map(
    primitive => `primitive.public.${primitive}`,
  ),
  assembledAt: input.assembledAt ?? currentIsoTimestamp(),
  fulfillmentMode: 'no_spend_public_fixture',
  billingState: 'not_configured',
})

export const selfServeListComposedProduct = (
  definition: ComposedProductDefinition,
  assembly: ComposedProductAssemblyReceipt,
  input: {
    listingRef?: string
    listedAt?: string
  } = {},
): {
  definition: ComposedProductDefinition
  listingReceipt: ComposedProductListingWriteReceipt
} => ({
  definition: {
    ...definition,
    listingState: 'listed',
  },
  listingReceipt: {
    schema: MARKETPLACE_PRODUCT_COMPOSITION_SCHEMA,
    promiseId: MARKETPLACE_COMPOSE_AND_LIST_PROMISE,
    listingRef:
      input.listingRef ??
      `listing.public.marketplace_composed_product.${definition.productId}.${definition.definitionVersion}`,
    assemblyRef: assembly.assemblyRef,
    productId: definition.productId,
    builderAttribution: assembly.builderAttribution,
    listedAt: input.listedAt ?? currentIsoTimestamp(),
    listingState: 'listed',
    selfServe: true,
    billingState: 'not_configured',
  },
})

export const recordComposedProductInstallUse = (
  listing: ComposedProductListingWriteReceipt,
  input: {
    buyerRef: string
    lifecycleRef?: string
    installedAt?: string
    usedAt?: string
  },
):
  | { ok: true; lifecycleReceipt: ComposedProductBuyerLifecycleReceipt }
  | { ok: false; error: ComposedProductValidationError } => {
  if (!isNonEmpty(input.buyerRef)) {
    return {
      ok: false,
      error: new ComposedProductValidationError({
        reason: 'buyerRef must be non-empty',
      }),
    }
  }

  const installedAt = input.installedAt ?? currentIsoTimestamp()
  return {
    ok: true,
    lifecycleReceipt: {
      schema: MARKETPLACE_PRODUCT_COMPOSITION_SCHEMA,
      promiseId: MARKETPLACE_COMPOSE_AND_LIST_PROMISE,
      lifecycleRef:
        input.lifecycleRef ??
        `lifecycle.public.marketplace_composed_product.${listing.productId}.${input.buyerRef}`,
      listingRef: listing.listingRef,
      productId: listing.productId,
      buyerRef: input.buyerRef,
      builderAttribution: listing.builderAttribution,
      installedAt,
      usedAt: input.usedAt ?? installedAt,
      installState: 'used',
      billingState: 'not_configured',
      settlementState: 'not_applicable',
    },
  }
}

/**
 * A read-only listing store. Injected so the surface stays pure and testable;
 * the live Worker passes an empty store while the marketplace is INERT.
 */
export type ComposedProductListingStore = {
  list: () => ReadonlyArray<ComposedProductDefinition>
}

export const emptyComposedProductListingStore: ComposedProductListingStore = {
  list: () => [],
}

export const makeInMemoryComposedProductListingStore = (
  definitions: ReadonlyArray<ComposedProductDefinition>,
): ComposedProductListingStore => ({
  list: () => definitions,
})

/**
 * Staleness contract for the listing projection. It is built fresh from the
 * injected store on every request, so it is `live_at_read` (maxStaleness 0).
 */
export const ComposedProductListingStaleness: PublicProjectionStalenessContract =
  liveAtReadStaleness(['marketplace_composed_product_listing_changed'])

/** Public-safe listing projection: only `listed` products are discoverable. */
export const listComposedProducts = (
  store: ComposedProductListingStore,
): {
  schema: typeof MARKETPLACE_PRODUCT_COMPOSITION_SCHEMA
  promiseId: typeof MARKETPLACE_COMPOSE_AND_LIST_PROMISE
  promiseState: 'planned'
  inert: true
  generatedAt: string
  maxStalenessSeconds: number
  staleness: PublicProjectionStalenessContract
  products: ReadonlyArray<ComposedProductDefinition>
} => ({
  schema: MARKETPLACE_PRODUCT_COMPOSITION_SCHEMA,
  promiseId: MARKETPLACE_COMPOSE_AND_LIST_PROMISE,
  // Honest: the surface is a scaffold; the promise stays planned and inert.
  promiseState: 'planned',
  inert: true,
  generatedAt: currentIsoTimestamp(),
  maxStalenessSeconds: ComposedProductListingStaleness.maxStalenessSeconds,
  staleness: ComposedProductListingStaleness,
  products: store.list().filter(product => product.listingState === 'listed'),
})

/** Read one listed product by id, or null when absent/unlisted. */
export const readComposedProduct = (
  store: ComposedProductListingStore,
  productId: string,
): ComposedProductDefinition | null =>
  listComposedProducts(store).products.find(
    product => product.productId === productId,
  ) ?? null
