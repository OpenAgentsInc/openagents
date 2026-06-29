// Compose-and-list marketplace MVP — typed product-composition model, bounded
// composition runtime core, install/use lifecycle records, and a pure listing
// surface (EPIC #5510, child #5515; promise
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
//   - the composition runtime only assembles a typed definition into an inert
//     listing row, then records public-safe install/use lifecycle evidence.
// The promise marketplace.compose_and_list_products.v1 STAYS `planned`. Nothing
// here flips it green: this is source-level scaffold evidence only, with no
// public write route, billing, rev-share, sale receipt, or settlement. A green
// flip stays receipt-first and owner-signed per proof.claim_upgrade_receipts.v1.

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

export const MARKETPLACE_PRODUCT_LIFECYCLE_SCHEMA =
  'openagents.marketplace_product_lifecycle.v1' as const

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

export class ComposedProductValidationError extends S.TaggedErrorClass<ComposedProductValidationError>()(
  'ComposedProductValidationError',
  {
    reason: S.String,
  },
) {}

const isNonEmpty = (value: string): boolean => value.trim().length > 0

const publicSafeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/@-]{0,260}$/
const unsafeLifecycleRefPattern =
  /(\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer\s+|cookie|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|preimage|secret)|preimage|private[_-]?key|provider[_-]?(credential|grant|payload|secret|token)|seed[_-]?phrase|sk-[a-z0-9]|wallet[._-]?(key|material|mnemonic|preimage|secret|seed)|xprv)/i

const isPublicSafeRef = (value: string): boolean =>
  publicSafeRefPattern.test(value) && !unsafeLifecycleRefPattern.test(value)

const composedProductBuilderAttributionRef = (
  definition: ComposedProductDefinition,
): string =>
  `marketplace.builder:${definition.productId}:${definition.definitionVersion}:${definition.builderRef}`

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

export type ComposedProductBuilderAttribution = Readonly<{
  builderRef: string
  attributionRef: string
}>

export type ComposedProductAssemblyPlan = Readonly<{
  schema: typeof MARKETPLACE_PRODUCT_COMPOSITION_SCHEMA
  productId: string
  definitionVersion: number
  status: 'assembled'
  promiseId: typeof MARKETPLACE_COMPOSE_AND_LIST_PROMISE
  inert: true
  componentRefs: ReadonlyArray<MarketplaceComponentRef>
  attribution: ComposedProductBuilderAttribution
  assembledAt: string
}>

export type ComposedProductAssemblyResult = Readonly<{
  definition: ComposedProductDefinition
  assemblyPlan: ComposedProductAssemblyPlan
}>

/**
 * Source-level composition runtime core: validate a builder-supplied typed
 * definition, pin it to `listed`, and emit the public-safe builder attribution
 * row future billing/rev-share work can bind to. It does not provision, bill,
 * fulfill, or expose a public write route.
 */
export const assembleComposedProductForListing = (input: {
  productId: string
  definitionVersion: number
  builderRef: string
  title: string
  summary: string
  components: ReadonlyArray<MarketplaceComponentRef>
  createdAt?: string
  assembledAt?: string
}):
  | { ok: true; result: ComposedProductAssemblyResult }
  | { ok: false; error: ComposedProductValidationError } => {
  const definitionResult = buildComposedProductDefinition({
    ...input,
    listingState: 'listed',
  })
  if (!definitionResult.ok) {
    return definitionResult
  }
  if (!isPublicSafeRef(input.productId)) {
    return {
      ok: false,
      error: new ComposedProductValidationError({
        reason: 'productId must be a public-safe product ref',
      }),
    }
  }
  if (!isPublicSafeRef(input.builderRef)) {
    return {
      ok: false,
      error: new ComposedProductValidationError({
        reason: 'builderRef must be a public-safe attribution ref',
      }),
    }
  }

  const definition = definitionResult.definition
  const attributionRef = composedProductBuilderAttributionRef(definition)
  return {
    ok: true,
    result: {
      definition,
      assemblyPlan: {
        schema: MARKETPLACE_PRODUCT_COMPOSITION_SCHEMA,
        productId: definition.productId,
        definitionVersion: definition.definitionVersion,
        status: 'assembled',
        promiseId: MARKETPLACE_COMPOSE_AND_LIST_PROMISE,
        inert: true,
        componentRefs: definition.components,
        attribution: {
          builderRef: definition.builderRef,
          attributionRef,
        },
        assembledAt: input.assembledAt ?? currentIsoTimestamp(),
      },
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

/**
 * A read-only listing store. Injected so the surface stays pure and testable;
 * the live Worker passes an empty store while the marketplace is INERT.
 */
export type ComposedProductListingStore = {
  list: () => ReadonlyArray<ComposedProductDefinition>
}

export type ComposedProductInstallEvidenceRow = Readonly<{
  schema: typeof MARKETPLACE_PRODUCT_LIFECYCLE_SCHEMA
  evidenceKind: 'marketplace_composed_product_install'
  lifecycleRef: string
  productId: string
  definitionVersion: number
  buyerRef: string
  builderRef: string
  attributionRef: string
  status: 'installed'
  inert: true
  installedAt: string
}>

export type ComposedProductUsageEvidenceRow = Readonly<{
  schema: typeof MARKETPLACE_PRODUCT_LIFECYCLE_SCHEMA
  evidenceKind: 'marketplace_composed_product_usage'
  usageRef: string
  lifecycleRef: string
  productId: string
  buyerRef: string
  builderRef: string
  attributionRef: string
  status: 'used'
  inert: true
  usedAt: string
}>

export type ComposedProductLifecycleStore = ComposedProductListingStore & {
  upsertDefinition: (definition: ComposedProductDefinition) => void
  readDefinition: (productId: string) => ComposedProductDefinition | undefined
  writeInstallEvidence: (row: ComposedProductInstallEvidenceRow) => void
  listInstallEvidence: () => ReadonlyArray<ComposedProductInstallEvidenceRow>
  writeUsageEvidence: (row: ComposedProductUsageEvidenceRow) => void
  listUsageEvidence: () => ReadonlyArray<ComposedProductUsageEvidenceRow>
}

export const emptyComposedProductListingStore: ComposedProductListingStore = {
  list: () => [],
}

export const makeInMemoryComposedProductListingStore = (
  definitions: ReadonlyArray<ComposedProductDefinition>,
): ComposedProductListingStore => ({
  list: () => definitions,
})

export const makeInMemoryComposedProductLifecycleStore = (
  initialDefinitions: ReadonlyArray<ComposedProductDefinition> = [],
): ComposedProductLifecycleStore => {
  const definitions = new Map<string, ComposedProductDefinition>(
    initialDefinitions.map(definition => [definition.productId, definition]),
  )
  const installRows: Array<ComposedProductInstallEvidenceRow> = []
  const usageRows: Array<ComposedProductUsageEvidenceRow> = []
  return {
    list: () => [...definitions.values()],
    upsertDefinition: definition => {
      definitions.set(definition.productId, definition)
    },
    readDefinition: productId => definitions.get(productId),
    writeInstallEvidence: row => {
      installRows.push(row)
    },
    listInstallEvidence: () => [...installRows],
    writeUsageEvidence: row => {
      usageRows.push(row)
    },
    listUsageEvidence: () => [...usageRows],
  }
}

export const selfServeListComposedProduct = (
  store: ComposedProductLifecycleStore,
  input: Parameters<typeof assembleComposedProductForListing>[0],
):
  | { ok: true; result: ComposedProductAssemblyResult }
  | { ok: false; error: ComposedProductValidationError } => {
  const assembled = assembleComposedProductForListing(input)
  if (!assembled.ok) {
    return assembled
  }
  store.upsertDefinition(assembled.result.definition)
  return assembled
}

export const installComposedProduct = (
  store: ComposedProductLifecycleStore,
  input: {
    productId: string
    buyerRef: string
    installedAt?: string
  },
):
  | { ok: true; row: ComposedProductInstallEvidenceRow }
  | { ok: false; error: ComposedProductValidationError } => {
  if (!isPublicSafeRef(input.buyerRef)) {
    return {
      ok: false,
      error: new ComposedProductValidationError({
        reason: 'buyerRef must be public-safe',
      }),
    }
  }
  const definition = readComposedProduct(store, input.productId)
  if (definition === null) {
    return {
      ok: false,
      error: new ComposedProductValidationError({
        reason: 'product must be listed before install',
      }),
    }
  }
  const lifecycleRef = `marketplace.install:${definition.productId}:${input.buyerRef}`
  const row: ComposedProductInstallEvidenceRow = {
    schema: MARKETPLACE_PRODUCT_LIFECYCLE_SCHEMA,
    evidenceKind: 'marketplace_composed_product_install',
    lifecycleRef,
    productId: definition.productId,
    definitionVersion: definition.definitionVersion,
    buyerRef: input.buyerRef,
    builderRef: definition.builderRef,
    attributionRef: composedProductBuilderAttributionRef(definition),
    status: 'installed',
    inert: true,
    installedAt: input.installedAt ?? currentIsoTimestamp(),
  }
  store.writeInstallEvidence(row)
  return { ok: true, row }
}

export const recordComposedProductUse = (
  store: ComposedProductLifecycleStore,
  input: {
    lifecycleRef: string
    usedAt?: string
  },
):
  | { ok: true; row: ComposedProductUsageEvidenceRow }
  | { ok: false; error: ComposedProductValidationError } => {
  const installRow = store
    .listInstallEvidence()
    .find(row => row.lifecycleRef === input.lifecycleRef)
  if (installRow === undefined) {
    return {
      ok: false,
      error: new ComposedProductValidationError({
        reason: 'install lifecycle must exist before use',
      }),
    }
  }
  const usageRef = `marketplace.use:${installRow.productId}:${installRow.buyerRef}:${store.listUsageEvidence().length + 1}`
  const row: ComposedProductUsageEvidenceRow = {
    schema: MARKETPLACE_PRODUCT_LIFECYCLE_SCHEMA,
    evidenceKind: 'marketplace_composed_product_usage',
    usageRef,
    lifecycleRef: installRow.lifecycleRef,
    productId: installRow.productId,
    buyerRef: installRow.buyerRef,
    builderRef: installRow.builderRef,
    attributionRef: installRow.attributionRef,
    status: 'used',
    inert: true,
    usedAt: input.usedAt ?? currentIsoTimestamp(),
  }
  store.writeUsageEvidence(row)
  return { ok: true, row }
}

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
