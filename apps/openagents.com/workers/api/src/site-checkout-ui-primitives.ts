import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  OpenAgentsPaidEndpointMethod,
  OpenAgentsPaidEndpointPrice,
} from './paid-endpoint-product-catalog'
import { OpenAgentsPaymentPolicyAudience } from './payment-limit-policy'
import { openAgentsRunnerGatewayPayloadHasPrivateMaterial } from './runner-gateway'
import {
  OpenAgentsSitePaymentCatalog,
  OpenAgentsSitePaymentCatalogRecord,
} from './site-payment-catalog'
import {
  OpenAgentsSitePaymentEntitlementScope,
  OpenAgentsSitePaymentPublicProjectionState,
} from './site-payment-manifest'

export const OpenAgentsSiteCheckoutUiRuntimeTarget = S.Literals([
  'static',
  'workers_for_platforms',
])
export type OpenAgentsSiteCheckoutUiRuntimeTarget =
  typeof OpenAgentsSiteCheckoutUiRuntimeTarget.Type

export const OpenAgentsSiteCheckoutUiPrimitiveKind = S.Literals([
  'cancel_state',
  'checkout_button',
  'checkout_form',
  'deposit_affordance',
  'entitlement_state',
  'paid_action_prompt',
  'product_card',
  'subscription_affordance',
  'success_state',
  'tip_affordance',
])
export type OpenAgentsSiteCheckoutUiPrimitiveKind =
  typeof OpenAgentsSiteCheckoutUiPrimitiveKind.Type

export const OpenAgentsSiteCheckoutUiSourceSurface = S.Literals([
  'generated_html',
  'generated_react',
  'generated_worker',
])
export type OpenAgentsSiteCheckoutUiSourceSurface =
  typeof OpenAgentsSiteCheckoutUiSourceSurface.Type

export const OpenAgentsSiteCheckoutUiPrimitive = S.Struct({
  actionId: S.NullOr(S.String),
  agentMetadataRefs: S.Array(S.String),
  agentReadable: S.Boolean,
  cancelPath: S.String,
  catalogRef: S.NullOr(S.String),
  checkoutIntentEndpoint: S.String,
  checkoutPath: S.NullOr(S.String),
  customerDataRequirementRefs: S.Array(S.String),
  displayRef: S.String,
  entitlementScope: S.NullOr(OpenAgentsSitePaymentEntitlementScope),
  id: S.String,
  itemKind: S.NullOr(S.Literals(['paid_action', 'product'])),
  method: S.NullOr(OpenAgentsPaidEndpointMethod),
  path: S.NullOr(S.String),
  price: S.NullOr(OpenAgentsPaidEndpointPrice),
  primitiveKind: OpenAgentsSiteCheckoutUiPrimitiveKind,
  productId: S.NullOr(S.String),
  publicProjectionState: OpenAgentsSitePaymentPublicProjectionState,
  runtimeTarget: OpenAgentsSiteCheckoutUiRuntimeTarget,
  sandbox: S.Boolean,
  sourceSafe: S.Literal(true),
  sourceSurface: OpenAgentsSiteCheckoutUiSourceSurface,
  successPath: S.String,
})
export type OpenAgentsSiteCheckoutUiPrimitive =
  typeof OpenAgentsSiteCheckoutUiPrimitive.Type

export const OpenAgentsSiteCheckoutUiPrimitiveContract = S.Struct({
  primitives: S.Array(OpenAgentsSiteCheckoutUiPrimitive),
  runtimeTarget: OpenAgentsSiteCheckoutUiRuntimeTarget,
  siteId: S.String,
  siteVersionId: S.String,
})
export type OpenAgentsSiteCheckoutUiPrimitiveContract =
  typeof OpenAgentsSiteCheckoutUiPrimitiveContract.Type

export const OpenAgentsSiteCheckoutUiPrimitiveProjection = S.Struct({
  audience: OpenAgentsPaymentPolicyAudience,
  primitives: S.Array(OpenAgentsSiteCheckoutUiPrimitive),
  runtimeTarget: OpenAgentsSiteCheckoutUiRuntimeTarget,
  siteId: S.String,
  siteVersionId: S.String,
})
export type OpenAgentsSiteCheckoutUiPrimitiveProjection =
  typeof OpenAgentsSiteCheckoutUiPrimitiveProjection.Type

export const OpenAgentsSiteCheckoutUiPrimitiveGenerationInput = S.Struct({
  cancelPath: S.String,
  catalog: OpenAgentsSitePaymentCatalog,
  runtimeTarget: OpenAgentsSiteCheckoutUiRuntimeTarget,
  sourceSurface: OpenAgentsSiteCheckoutUiSourceSurface,
  successPath: S.String,
})
export type OpenAgentsSiteCheckoutUiPrimitiveGenerationInput =
  typeof OpenAgentsSiteCheckoutUiPrimitiveGenerationInput.Type

export class OpenAgentsSiteCheckoutUiPrimitiveUnsafe extends S.TaggedErrorClass<OpenAgentsSiteCheckoutUiPrimitiveUnsafe>()(
  'OpenAgentsSiteCheckoutUiPrimitiveUnsafe',
  {
    reason: S.String,
  },
) {}

const stableIdPattern = /^[a-z][a-z0-9]*(?:[._:-][a-z0-9]+)*$/
const stableRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,180}$/
const unsafeKeyPattern =
  /(access[_-]?token|bearer|callback[_-]?token|checkout[_-]?id|cookie|customer[_-]?(email|name|value)|email[_-]?body|grant|invoice|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|preimage)|preimage|private[_-]?key|provider[_-]?(account|grant|payload|token)|raw[_-]?(invoice|payment|prompt|runner|run[_-]?log)|secret|source[_-]?archive|wallet|webhook)/i
const unsafeValuePattern =
  /(bearer\s+|checkout_id=|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github[_-]?pat_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment_hash=|payment_preimage=|preimage=[A-Za-z0-9_-]+|provider[_-]?token|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log)|secret|sk-[a-z0-9]|\S+@\S+|wallet[_-]?state)/i

const valueHasPrivateMaterial = (value: unknown): boolean => {
  if (typeof value === 'string') {
    return containsProviderSecretMaterial(value) ||
      unsafeValuePattern.test(value) ||
      openAgentsRunnerGatewayPayloadHasPrivateMaterial(value)
  }

  if (Array.isArray(value)) {
    return value.some(valueHasPrivateMaterial)
  }

  if (value !== null && typeof value === 'object') {
    return openAgentsRunnerGatewayPayloadHasPrivateMaterial(value) ||
      Object.entries(value).some(([key, item]) =>
        unsafeKeyPattern.test(key) || valueHasPrivateMaterial(item),
      )
  }

  return false
}

const stableRefIsSafe = (value: string): boolean =>
  value.trim() !== '' &&
  stableRefPattern.test(value) &&
  !valueHasPrivateMaterial(value)

const stableIdIsSafe = (value: string): boolean =>
  stableIdPattern.test(value) && !valueHasPrivateMaterial(value)

const cleanPathIsSafe = (value: string): boolean =>
  value.startsWith('/') &&
  !value.includes('?') &&
  !value.includes('#') &&
  !value.includes('://') &&
  !value.includes('//') &&
  !valueHasPrivateMaterial(value)

const nullableStableIdIsSafe = (value: string | null): boolean =>
  value === null || stableIdIsSafe(value)

const nullableStableRefIsSafe = (value: string | null): boolean =>
  value === null || stableRefIsSafe(value)

const nullableCleanPathIsSafe = (value: string | null): boolean =>
  value === null || cleanPathIsSafe(value)

const safeRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs)].filter(stableRefIsSafe)

const catalogIdSegment = (value: string, fallback: string): string => {
  const normalized = value
    .toLowerCase()
    .replaceAll(/[^a-z0-9]+/g, '_')
    .replaceAll(/^_+|_+$/g, '')
    .slice(0, 64)
  const candidate = normalized === '' ? fallback : normalized

  return /^[a-z]/u.test(candidate) ? candidate : `${fallback}_${candidate}`
}

const checkoutIntentEndpointForSite = (siteId: string): string =>
  `/api/sites/${encodeURIComponent(siteId)}/commerce/checkout-intents`

const primitiveId = (
  primitiveKind: OpenAgentsSiteCheckoutUiPrimitiveKind,
  catalogItem: OpenAgentsSitePaymentCatalogRecord,
): string =>
  [
    'site_checkout_ui',
    catalogIdSegment(catalogItem.siteId, 'site'),
    catalogIdSegment(catalogItem.siteVersionId, 'version'),
    primitiveKind,
    catalogIdSegment(
      catalogItem.itemKind === 'product'
        ? catalogItem.productId
        : catalogItem.actionId,
      'item',
    ),
  ].join(':')

const requirementRefs = (
  catalogItem: OpenAgentsSitePaymentCatalogRecord,
): ReadonlyArray<string> =>
  catalogItem.customerDataRequirements.map(requirement => requirement.key)

const commonPrimitive = (
  input: OpenAgentsSiteCheckoutUiPrimitiveGenerationInput,
  catalogItem: OpenAgentsSitePaymentCatalogRecord,
  primitiveKind: OpenAgentsSiteCheckoutUiPrimitiveKind,
): OpenAgentsSiteCheckoutUiPrimitive => ({
  actionId: catalogItem.itemKind === 'paid_action'
    ? catalogItem.actionId
    : null,
  agentMetadataRefs: catalogItem.agentReadable
    ? safeRefs(catalogItem.metadataRefs)
    : [],
  agentReadable: catalogItem.agentReadable,
  cancelPath: input.cancelPath,
  catalogRef: catalogItem.catalogRef,
  checkoutIntentEndpoint: checkoutIntentEndpointForSite(catalogItem.siteId),
  checkoutPath: catalogItem.checkoutPath,
  customerDataRequirementRefs: requirementRefs(catalogItem),
  displayRef: catalogItem.displayRef,
  entitlementScope: catalogItem.entitlementScope,
  id: primitiveId(primitiveKind, catalogItem),
  itemKind: catalogItem.itemKind,
  method: catalogItem.itemKind === 'paid_action'
    ? catalogItem.method
    : null,
  path: catalogItem.itemKind === 'paid_action' ? catalogItem.path : null,
  price: catalogItem.price,
  primitiveKind,
  productId: catalogItem.itemKind === 'product'
    ? catalogItem.productId
    : null,
  publicProjectionState: catalogItem.publicProjectionState,
  runtimeTarget: input.runtimeTarget,
  sandbox: catalogItem.sandbox,
  sourceSafe: true,
  sourceSurface: input.sourceSurface,
  successPath: input.successPath,
})

const catalogItemPrimitives = (
  input: OpenAgentsSiteCheckoutUiPrimitiveGenerationInput,
  catalogItem: OpenAgentsSitePaymentCatalogRecord,
): ReadonlyArray<OpenAgentsSiteCheckoutUiPrimitive> =>
  catalogItem.itemKind === 'product'
    ? [
      commonPrimitive(input, catalogItem, 'product_card'),
      commonPrimitive(input, catalogItem, 'checkout_button'),
      commonPrimitive(input, catalogItem, 'checkout_form'),
    ]
    : [
      commonPrimitive(input, catalogItem, 'paid_action_prompt'),
      commonPrimitive(input, catalogItem, 'checkout_button'),
    ]

const statePrimitive = (
  input: OpenAgentsSiteCheckoutUiPrimitiveGenerationInput,
  catalog: OpenAgentsSitePaymentCatalog,
  primitiveKind:
    | 'cancel_state'
    | 'entitlement_state'
    | 'success_state',
): OpenAgentsSiteCheckoutUiPrimitive => {
  const firstItem = catalog.items[0]
  const siteId = firstItem?.siteId ?? 'site.generated'
  const siteVersionId = firstItem?.siteVersionId ?? 'version.generated'

  return {
    actionId: null,
    agentMetadataRefs: [],
    agentReadable: true,
    cancelPath: input.cancelPath,
    catalogRef: null,
    checkoutIntentEndpoint: checkoutIntentEndpointForSite(siteId),
    checkoutPath: null,
    customerDataRequirementRefs: [],
    displayRef: `display.${primitiveKind}`,
    entitlementScope: null,
    id: [
      'site_checkout_ui',
      catalogIdSegment(siteId, 'site'),
      catalogIdSegment(siteVersionId, 'version'),
      primitiveKind,
    ].join(':'),
    itemKind: null,
    method: null,
    path: null,
    price: null,
    primitiveKind,
    productId: null,
    publicProjectionState: 'listed',
    runtimeTarget: input.runtimeTarget,
    sandbox: firstItem?.sandbox ?? true,
    sourceSafe: true,
    sourceSurface: input.sourceSurface,
    successPath: input.successPath,
  }
}

const primitiveIsSafe = (
  primitive: OpenAgentsSiteCheckoutUiPrimitive,
): boolean =>
  stableIdIsSafe(primitive.id) &&
  nullableStableRefIsSafe(primitive.catalogRef) &&
  nullableStableIdIsSafe(primitive.productId) &&
  nullableStableIdIsSafe(primitive.actionId) &&
  stableRefIsSafe(primitive.displayRef) &&
  cleanPathIsSafe(primitive.checkoutIntentEndpoint) &&
  cleanPathIsSafe(primitive.successPath) &&
  cleanPathIsSafe(primitive.cancelPath) &&
  nullableCleanPathIsSafe(primitive.checkoutPath) &&
  nullableCleanPathIsSafe(primitive.path) &&
  primitive.customerDataRequirementRefs.every(stableIdIsSafe) &&
  primitive.agentMetadataRefs.every(stableRefIsSafe) &&
  !valueHasPrivateMaterial(primitive)

export const openAgentsSiteCheckoutUiPrimitiveHasPrivateMaterial =
  valueHasPrivateMaterial

export const decodeOpenAgentsSiteCheckoutUiPrimitiveContract = (
  value: unknown,
): OpenAgentsSiteCheckoutUiPrimitiveContract => {
  if (valueHasPrivateMaterial(value)) {
    throw new OpenAgentsSiteCheckoutUiPrimitiveUnsafe({
      reason:
        'Site checkout UI primitives must not contain customer private values, raw payment material, wallet state, provider grants, or secrets.',
    })
  }

  const contract = S.decodeUnknownSync(
    OpenAgentsSiteCheckoutUiPrimitiveContract,
  )(value)
  const unsafePrimitive = contract.primitives.find(
    primitive => !primitiveIsSafe(primitive),
  )

  if (
    !stableRefIsSafe(contract.siteId) ||
    !stableRefIsSafe(contract.siteVersionId) ||
    unsafePrimitive !== undefined
  ) {
    throw new OpenAgentsSiteCheckoutUiPrimitiveUnsafe({
      reason:
        'Site checkout UI primitives must use stable refs and clean local paths.',
    })
  }

  return contract
}

export const siteCheckoutUiPrimitivesFromCatalog = (
  input: OpenAgentsSiteCheckoutUiPrimitiveGenerationInput,
): OpenAgentsSiteCheckoutUiPrimitiveContract => {
  const primitives = [
    ...input.catalog.items.flatMap(catalogItem =>
      catalogItemPrimitives(input, catalogItem),
    ),
    statePrimitive(input, input.catalog, 'success_state'),
    statePrimitive(input, input.catalog, 'cancel_state'),
    statePrimitive(input, input.catalog, 'entitlement_state'),
  ]
  const firstItem = input.catalog.items[0]

  return decodeOpenAgentsSiteCheckoutUiPrimitiveContract({
    primitives,
    runtimeTarget: input.runtimeTarget,
    siteId: firstItem?.siteId ?? 'site.generated',
    siteVersionId: firstItem?.siteVersionId ?? 'version.generated',
  })
}

export const projectOpenAgentsSiteCheckoutUiPrimitives = (
  contract: OpenAgentsSiteCheckoutUiPrimitiveContract,
  audience: typeof OpenAgentsPaymentPolicyAudience.Type,
): OpenAgentsSiteCheckoutUiPrimitiveProjection => ({
  audience,
  primitives: contract.primitives
    .filter(primitive =>
      audience === 'operator' ||
      primitive.publicProjectionState !== 'hidden',
    )
    .map(primitive => ({
      ...primitive,
      agentMetadataRefs:
        audience === 'agent' || audience === 'operator'
          ? primitive.agentMetadataRefs
          : [],
    })),
  runtimeTarget: contract.runtimeTarget,
  siteId: contract.siteId,
  siteVersionId: contract.siteVersionId,
})
