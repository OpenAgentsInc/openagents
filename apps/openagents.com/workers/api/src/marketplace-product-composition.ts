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
// claim it is live. It is PURE:
//   - it moves no money, runs no fulfillment, reads no wallet, writes no
//     receipt, and provisions no primitive;
//   - it defines a typed, versioned product DEFINITION (a composition of
//     primitive/market capability references), a pure assembly/list/install/use
//     lifecycle over an injected store, and a read-only listing projection.
// The promise marketplace.compose_and_list_products.v1 STAYS `planned`. Nothing
// here flips it green: there is no live durable self-serve write route, no
// billing, rev-share, or settlement. A green flip stays receipt-first and
// owner-signed per proof.claim_upgrade_receipts.v1.

import { Schema as S } from 'effect'

import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

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

export const MarketplaceInstallUseState = S.Literals(['installed', 'used'])
export type MarketplaceInstallUseState = typeof MarketplaceInstallUseState.Type

export const ComposedProductAssembly = S.Struct({
  schema: S.Literal(MARKETPLACE_PRODUCT_COMPOSITION_SCHEMA),
  assemblyId: S.String,
  productId: S.String,
  definitionVersion: S.Number,
  builderRef: S.String,
  builderAttributionRef: S.String,
  componentRefs: S.Array(MarketplaceComponentRef),
  assembledAt: S.String,
  promiseId: S.Literal(MARKETPLACE_COMPOSE_AND_LIST_PROMISE),
  billingAuthority: S.Boolean,
  settlementAuthority: S.Boolean,
})
export type ComposedProductAssembly = typeof ComposedProductAssembly.Type

export const SelfServeComposedProductListing = S.Struct({
  schema: S.Literal(MARKETPLACE_PRODUCT_COMPOSITION_SCHEMA),
  listingId: S.String,
  definition: ComposedProductDefinition,
  assembly: ComposedProductAssembly,
  listedAt: S.String,
  promiseId: S.Literal(MARKETPLACE_COMPOSE_AND_LIST_PROMISE),
})
export type SelfServeComposedProductListing =
  typeof SelfServeComposedProductListing.Type

export const ComposedProductInstallUseRecord = S.Struct({
  schema: S.Literal(MARKETPLACE_PRODUCT_COMPOSITION_SCHEMA),
  installId: S.String,
  listingId: S.String,
  productId: S.String,
  buyerRef: S.String,
  builderRef: S.String,
  builderAttributionRef: S.String,
  state: MarketplaceInstallUseState,
  installedAt: S.String,
  lastUsedAt: S.NullOr(S.String),
  useCount: S.Number,
  promiseId: S.Literal(MARKETPLACE_COMPOSE_AND_LIST_PROMISE),
  billingAuthority: S.Boolean,
  settlementAuthority: S.Boolean,
})
export type ComposedProductInstallUseRecord =
  typeof ComposedProductInstallUseRecord.Type

export class ComposedProductValidationError extends S.TaggedErrorClass<ComposedProductValidationError>()(
  'ComposedProductValidationError',
  {
    reason: S.String,
  },
) {}

const isNonEmpty = (value: string): boolean => value.trim().length > 0

const publicRefSegment = (value: string): string =>
  value.trim().replace(/[^A-Za-z0-9_.:-]+/g, '_').slice(0, 80)

export const composedProductBuilderAttributionRef = (
  definition: ComposedProductDefinition,
): string =>
  `attribution.marketplace.composed_product.${publicRefSegment(definition.builderRef)}.${publicRefSegment(definition.productId)}.v${definition.definitionVersion}`

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
  options: { assemblyId?: string; assembledAt?: string } = {},
): ComposedProductAssembly => ({
  schema: MARKETPLACE_PRODUCT_COMPOSITION_SCHEMA,
  assemblyId: options.assemblyId ?? compactRandomId('assembly'),
  productId: definition.productId,
  definitionVersion: definition.definitionVersion,
  builderRef: definition.builderRef,
  builderAttributionRef: composedProductBuilderAttributionRef(definition),
  componentRefs: definition.components,
  assembledAt: options.assembledAt ?? currentIsoTimestamp(),
  promiseId: MARKETPLACE_COMPOSE_AND_LIST_PROMISE,
  billingAuthority: false,
  settlementAuthority: false,
})

export type MarketplaceCompositionLifecycleStore = {
  listListings: () => ReadonlyArray<SelfServeComposedProductListing>
  listInstalls: () => ReadonlyArray<ComposedProductInstallUseRecord>
  saveListing: (listing: SelfServeComposedProductListing) => void
  saveInstall: (install: ComposedProductInstallUseRecord) => void
}

export const makeInMemoryMarketplaceCompositionLifecycleStore = (
  seed: {
    listings?: ReadonlyArray<SelfServeComposedProductListing>
    installs?: ReadonlyArray<ComposedProductInstallUseRecord>
  } = {},
): MarketplaceCompositionLifecycleStore => {
  const listings = [...(seed.listings ?? [])]
  const installs = [...(seed.installs ?? [])]

  return {
    listListings: () => listings,
    listInstalls: () => installs,
    saveListing: listing => {
      const existingIndex = listings.findIndex(
        existing => existing.listingId === listing.listingId,
      )
      if (existingIndex >= 0) {
        listings[existingIndex] = listing
      } else {
        listings.push(listing)
      }
    },
    saveInstall: install => {
      const existingIndex = installs.findIndex(
        existing => existing.installId === install.installId,
      )
      if (existingIndex >= 0) {
        installs[existingIndex] = install
      } else {
        installs.push(install)
      }
    },
  }
}

export const selfServeListComposedProduct = (
  store: MarketplaceCompositionLifecycleStore,
  input: {
    definition: ComposedProductDefinition
    listingId?: string
    listedAt?: string
    assemblyId?: string
  },
): SelfServeComposedProductListing => {
  const definition = { ...input.definition, listingState: 'listed' as const }
  const listing = {
    schema: MARKETPLACE_PRODUCT_COMPOSITION_SCHEMA,
    listingId: input.listingId ?? compactRandomId('listing'),
    definition,
    assembly: assembleComposedProduct(definition, {
      ...(input.assemblyId === undefined ? {} : { assemblyId: input.assemblyId }),
      ...(input.listedAt === undefined ? {} : { assembledAt: input.listedAt }),
    }),
    listedAt: input.listedAt ?? currentIsoTimestamp(),
    promiseId: MARKETPLACE_COMPOSE_AND_LIST_PROMISE,
  }

  store.saveListing(listing)
  return listing
}

export const installComposedProduct = (
  store: MarketplaceCompositionLifecycleStore,
  input: {
    listingId: string
    buyerRef: string
    installId?: string
    installedAt?: string
  },
):
  | { ok: true; install: ComposedProductInstallUseRecord }
  | { ok: false; error: ComposedProductValidationError } => {
  if (!isNonEmpty(input.buyerRef)) {
    return {
      ok: false,
      error: new ComposedProductValidationError({
        reason: 'buyerRef must be non-empty',
      }),
    }
  }

  const listing = store
    .listListings()
    .find(candidate => candidate.listingId === input.listingId)

  if (listing === undefined) {
    return {
      ok: false,
      error: new ComposedProductValidationError({
        reason: 'listingId does not reference a self-serve listed product',
      }),
    }
  }

  const install = {
    schema: MARKETPLACE_PRODUCT_COMPOSITION_SCHEMA,
    installId: input.installId ?? compactRandomId('install'),
    listingId: listing.listingId,
    productId: listing.definition.productId,
    buyerRef: input.buyerRef,
    builderRef: listing.definition.builderRef,
    builderAttributionRef: listing.assembly.builderAttributionRef,
    state: 'installed' as const,
    installedAt: input.installedAt ?? currentIsoTimestamp(),
    lastUsedAt: null,
    useCount: 0,
    promiseId: MARKETPLACE_COMPOSE_AND_LIST_PROMISE,
    billingAuthority: false,
    settlementAuthority: false,
  }

  store.saveInstall(install)
  return { ok: true, install }
}

export const recordComposedProductUse = (
  store: MarketplaceCompositionLifecycleStore,
  input: { installId: string; usedAt?: string },
):
  | { ok: true; install: ComposedProductInstallUseRecord }
  | { ok: false; error: ComposedProductValidationError } => {
  const install = store
    .listInstalls()
    .find(candidate => candidate.installId === input.installId)

  if (install === undefined) {
    return {
      ok: false,
      error: new ComposedProductValidationError({
        reason: 'installId does not reference an installed composed product',
      }),
    }
  }

  const updated = {
    ...install,
    state: 'used' as const,
    lastUsedAt: input.usedAt ?? currentIsoTimestamp(),
    useCount: install.useCount + 1,
  }

  store.saveInstall(updated)
  return { ok: true, install: updated }
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
