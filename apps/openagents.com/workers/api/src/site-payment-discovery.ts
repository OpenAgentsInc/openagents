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
  OpenAgentsSitePaymentCatalogItemKind,
  OpenAgentsSitePaymentCatalogStatus,
  openAgentsPaidEndpointProductFromSitePaymentCatalogItem,
  projectOpenAgentsSitePaymentCatalog,
} from './site-payment-catalog'
import {
  OpenAgentsSitePaymentEntitlementScope,
  OpenAgentsSitePaymentPublicProjectionState,
  OpenAgentsSitePaymentProvider,
  OpenAgentsSitePaymentSettlementMode,
} from './site-payment-manifest'

export const OpenAgentsSitePaymentDiscoverySurfaceStatus = S.Literals([
  'available',
  'available_contract',
  'fake_provider_only',
  'gated',
  'planned_not_live',
])
export type OpenAgentsSitePaymentDiscoverySurfaceStatus =
  typeof OpenAgentsSitePaymentDiscoverySurfaceStatus.Type

export const OpenAgentsSitePaymentDiscoverySurfaceStates = S.Struct({
  checkoutIntent: OpenAgentsSitePaymentDiscoverySurfaceStatus,
  checkoutReturn: OpenAgentsSitePaymentDiscoverySurfaceStatus,
  commerceReview: OpenAgentsSitePaymentDiscoverySurfaceStatus,
  commerceReviewDecision: OpenAgentsSitePaymentDiscoverySurfaceStatus,
  entitlementProjection: OpenAgentsSitePaymentDiscoverySurfaceStatus,
  l402Challenge: OpenAgentsSitePaymentDiscoverySurfaceStatus,
  l402Redemption: OpenAgentsSitePaymentDiscoverySurfaceStatus,
  mdkAccountBinding: OpenAgentsSitePaymentDiscoverySurfaceStatus,
  mdkAccountBindingReview: OpenAgentsSitePaymentDiscoverySurfaceStatus,
  payoutBridge: OpenAgentsSitePaymentDiscoverySurfaceStatus,
  paymentProof: OpenAgentsSitePaymentDiscoverySurfaceStatus,
  providerEventReconciliation: OpenAgentsSitePaymentDiscoverySurfaceStatus,
  settlement: OpenAgentsSitePaymentDiscoverySurfaceStatus,
  wfpMiddleware: OpenAgentsSitePaymentDiscoverySurfaceStatus,
})
export type OpenAgentsSitePaymentDiscoverySurfaceStates =
  typeof OpenAgentsSitePaymentDiscoverySurfaceStates.Type

export const OpenAgentsSitePaymentDiscoveryEndpoints = S.Struct({
  checkoutIntent: S.String,
  checkoutReturn: S.String,
  commerceReview: S.String,
  commerceReviewDecision: S.String,
  l402Challenge: S.String,
  l402Redemption: S.String,
  mdkAccountBinding: S.String,
  mdkAccountBindingReview: S.String,
  payoutBridge: S.String,
  paymentProof: S.String,
  providerEventReconcile: S.String,
})
export type OpenAgentsSitePaymentDiscoveryEndpoints =
  typeof OpenAgentsSitePaymentDiscoveryEndpoints.Type

export const OpenAgentsSitePaymentDiscoveryItem = S.Struct({
  actionId: S.NullOr(S.String),
  agentReadable: S.Boolean,
  catalogRef: S.String,
  checkoutIntentEndpoint: S.String,
  checkoutPath: S.String,
  customerDataRequirementRefs: S.Array(S.String),
  displayRef: S.String,
  entitlementScope: OpenAgentsSitePaymentEntitlementScope,
  entitlementSemanticsRefs: S.Array(S.String),
  itemKind: OpenAgentsSitePaymentCatalogItemKind,
  l402ChallengeEndpoint: S.NullOr(S.String),
  l402HeaderRef: S.String,
  l402RedemptionEndpoint: S.NullOr(S.String),
  metadataRefs: S.Array(S.String),
  method: S.NullOr(OpenAgentsPaidEndpointMethod),
  path: S.NullOr(S.String),
  price: OpenAgentsPaidEndpointPrice,
  productId: S.NullOr(S.String),
  publicProjectionState: OpenAgentsSitePaymentPublicProjectionState,
  sandbox: S.Boolean,
  settlementMode: OpenAgentsSitePaymentSettlementMode,
  spendCapHintRefs: S.Array(S.String),
  status: OpenAgentsSitePaymentCatalogStatus,
  surfaceStates: OpenAgentsSitePaymentDiscoverySurfaceStates,
})
export type OpenAgentsSitePaymentDiscoveryItem =
  typeof OpenAgentsSitePaymentDiscoveryItem.Type

export const OpenAgentsSitePaymentDiscoveryProjection = S.Struct({
  agentReadable: S.Boolean,
  audience: OpenAgentsPaymentPolicyAudience,
  docsRefs: S.Array(S.String),
  endpoints: OpenAgentsSitePaymentDiscoveryEndpoints,
  items: S.Array(OpenAgentsSitePaymentDiscoveryItem),
  l402HeaderRef: S.String,
  provider: OpenAgentsSitePaymentProvider,
  redactionRefs: S.Array(S.String),
  siteId: S.String,
  siteVersionIds: S.Array(S.String),
  surfaceStates: OpenAgentsSitePaymentDiscoverySurfaceStates,
})
export type OpenAgentsSitePaymentDiscoveryProjection =
  typeof OpenAgentsSitePaymentDiscoveryProjection.Type

export const OpenAgentsSitePaymentDiscoveryInput = S.Struct({
  audience: OpenAgentsPaymentPolicyAudience,
  catalog: OpenAgentsSitePaymentCatalog,
  siteId: S.String,
})
export type OpenAgentsSitePaymentDiscoveryInput =
  typeof OpenAgentsSitePaymentDiscoveryInput.Type

export class OpenAgentsSitePaymentDiscoveryUnsafe extends S.TaggedErrorClass<OpenAgentsSitePaymentDiscoveryUnsafe>()(
  'OpenAgentsSitePaymentDiscoveryUnsafe',
  {
    reason: S.String,
  },
) {}

const stableRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/
const stableIdPattern = /^[a-z][a-z0-9]*(?:[._:-][a-z0-9]+)*$/
const unsafeKeyPattern =
  /(access[_-]?token|bearer[_-]?(credential|secret|token)|callback[_-]?token|checkout[_-]?id|cookie|customer[_-]?(email|name|value)|email[_-]?body|grant|invoice|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|preimage)|preimage|private[_-]?key|provider[_-]?(account|grant|payload|token)|raw[_-]?(invoice|payment|prompt|runner|run[_-]?log)|secret|source[_-]?archive|wallet|webhook)/i
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
        (item !== null && unsafeKeyPattern.test(key)) ||
        valueHasPrivateMaterial(item),
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

const nullableCleanPathIsSafe = (value: string | null): boolean =>
  value === null || cleanPathIsSafe(value)

const safeRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs)].filter(stableRefIsSafe)

const siteCommerceEndpoints = (
  siteId: string,
): OpenAgentsSitePaymentDiscoveryEndpoints => {
  const encodedSiteId = encodeURIComponent(siteId)

  return {
    checkoutIntent: `/api/sites/${encodedSiteId}/commerce/checkout-intents`,
    checkoutReturn:
      `/api/sites/${encodedSiteId}/commerce/checkout-returns/{checkoutIntentRef}/{returnAction}`,
    commerceReview: `/api/sites/${encodedSiteId}/commerce/review`,
    commerceReviewDecision:
      `/api/sites/${encodedSiteId}/commerce/review-decisions`,
    l402Challenge: `/api/sites/${encodedSiteId}/commerce/l402/challenges`,
    l402Redemption: `/api/sites/${encodedSiteId}/commerce/l402/redemptions`,
    mdkAccountBinding:
      `/api/sites/${encodedSiteId}/commerce/mdk-account-binding`,
    mdkAccountBindingReview:
      `/api/sites/${encodedSiteId}/commerce/mdk-account-bindings`,
    payoutBridge: `/api/sites/${encodedSiteId}/commerce/payout-bridges`,
    paymentProof:
      `/api/sites/${encodedSiteId}/commerce/payment-proofs/{checkoutIntentRef}`,
    providerEventReconcile:
      `/api/sites/${encodedSiteId}/commerce/mdk/webhooks`,
  }
}

const currentSurfaceStates: OpenAgentsSitePaymentDiscoverySurfaceStates = {
  checkoutIntent: 'gated',
  checkoutReturn: 'available',
  commerceReview: 'available',
  commerceReviewDecision: 'gated',
  entitlementProjection: 'available',
  l402Challenge: 'available_contract',
  l402Redemption: 'available_contract',
  mdkAccountBinding: 'available',
  mdkAccountBindingReview: 'gated',
  payoutBridge: 'gated',
  paymentProof: 'available',
  providerEventReconciliation: 'gated',
  settlement: 'gated',
  wfpMiddleware: 'available_contract',
}

const itemDiscovery = (
  endpoints: OpenAgentsSitePaymentDiscoveryEndpoints,
  item: ReturnType<typeof projectOpenAgentsSitePaymentCatalog>['items'][number],
  sourceCatalog: OpenAgentsSitePaymentCatalog,
): OpenAgentsSitePaymentDiscoveryItem => {
  const catalogItem = sourceCatalog.items.find(
    candidate => candidate.catalogRef === item.catalogRef,
  )
  const paidProduct = catalogItem === undefined
    ? null
    : openAgentsPaidEndpointProductFromSitePaymentCatalogItem(catalogItem)

  return {
    actionId: item.itemKind === 'paid_action' ? item.actionId : null,
    agentReadable: item.agentReadable,
    catalogRef: item.catalogRef,
    checkoutIntentEndpoint: endpoints.checkoutIntent,
    checkoutPath: item.checkoutPath,
    customerDataRequirementRefs: item.customerDataRequirements
      .map(requirement => requirement.key)
      .filter(stableIdIsSafe),
    displayRef: item.displayRef,
    entitlementScope: item.entitlementScope,
    entitlementSemanticsRefs: paidProduct === null
      ? []
      : safeRefs(paidProduct.entitlement.scopeRefs),
    itemKind: item.itemKind,
    l402ChallengeEndpoint: item.itemKind === 'paid_action'
      ? endpoints.l402Challenge
      : null,
    l402HeaderRef: 'WWW-Authenticate: L402',
    l402RedemptionEndpoint: item.itemKind === 'paid_action'
      ? endpoints.l402Redemption
      : null,
    metadataRefs: safeRefs(item.metadataRefs),
    method: item.itemKind === 'paid_action' ? item.method : null,
    path: item.itemKind === 'paid_action' ? item.path : null,
    price: item.price,
    productId: item.itemKind === 'product' ? item.productId : null,
    publicProjectionState: item.publicProjectionState,
    sandbox: item.sandbox,
    settlementMode: item.settlementMode,
    spendCapHintRefs: paidProduct === null
      ? []
      : safeRefs(paidProduct.spendCapHintRefs),
    status: item.status,
    surfaceStates: currentSurfaceStates,
  }
}

const discoveryItemIsSafe = (
  item: OpenAgentsSitePaymentDiscoveryItem,
): boolean =>
  stableRefIsSafe(item.catalogRef) &&
  nullableStableIdIsSafe(item.productId) &&
  nullableStableIdIsSafe(item.actionId) &&
  stableRefIsSafe(item.displayRef) &&
  cleanPathIsSafe(item.checkoutIntentEndpoint) &&
  cleanPathIsSafe(item.checkoutPath) &&
  nullableCleanPathIsSafe(item.path) &&
  nullableCleanPathIsSafe(item.l402ChallengeEndpoint) &&
  nullableCleanPathIsSafe(item.l402RedemptionEndpoint) &&
  item.customerDataRequirementRefs.every(stableIdIsSafe) &&
  item.entitlementSemanticsRefs.every(stableRefIsSafe) &&
  item.metadataRefs.every(stableRefIsSafe) &&
  item.spendCapHintRefs.every(stableRefIsSafe) &&
  !valueHasPrivateMaterial(item)

const discoveryIsSafe = (
  discovery: OpenAgentsSitePaymentDiscoveryProjection,
): boolean =>
  stableRefIsSafe(discovery.siteId) &&
  discovery.siteVersionIds.every(stableRefIsSafe) &&
  cleanPathIsSafe(discovery.endpoints.checkoutIntent) &&
  cleanPathIsSafe(discovery.endpoints.checkoutReturn) &&
  cleanPathIsSafe(discovery.endpoints.l402Challenge) &&
  cleanPathIsSafe(discovery.endpoints.l402Redemption) &&
  cleanPathIsSafe(discovery.endpoints.providerEventReconcile) &&
  discovery.docsRefs.every(stableRefIsSafe) &&
  discovery.redactionRefs.every(stableRefIsSafe) &&
  discovery.items.every(discoveryItemIsSafe) &&
  !valueHasPrivateMaterial(discovery)

export const openAgentsSitePaymentDiscoveryHasPrivateMaterial =
  valueHasPrivateMaterial

export const decodeOpenAgentsSitePaymentDiscoveryProjection = (
  value: unknown,
): OpenAgentsSitePaymentDiscoveryProjection => {
  if (valueHasPrivateMaterial(value)) {
    throw new OpenAgentsSitePaymentDiscoveryUnsafe({
      reason:
        'Site payment discovery must not contain customer private data, raw payment material, wallet state, MDK credentials, provider grants, payout claims, or secrets.',
    })
  }

  const discovery = S.decodeUnknownSync(
    OpenAgentsSitePaymentDiscoveryProjection,
  )(value)

  if (!discoveryIsSafe(discovery)) {
    throw new OpenAgentsSitePaymentDiscoveryUnsafe({
      reason:
        'Site payment discovery must use stable refs and clean local endpoint paths.',
    })
  }

  return discovery
}

export const projectOpenAgentsSitePaymentDiscovery = (
  input: OpenAgentsSitePaymentDiscoveryInput,
): OpenAgentsSitePaymentDiscoveryProjection => {
  if (valueHasPrivateMaterial(input)) {
    throw new OpenAgentsSitePaymentDiscoveryUnsafe({
      reason:
        'Site payment discovery input must not contain customer private data, raw payment material, wallet state, MDK credentials, provider grants, payout claims, or secrets.',
    })
  }

  const catalog = {
    items: input.catalog.items.filter(item => item.siteId === input.siteId),
  }
  const projectedCatalog = projectOpenAgentsSitePaymentCatalog(
    catalog,
    input.audience,
  )
  const endpoints = siteCommerceEndpoints(input.siteId)
  const siteVersionIds = safeRefs(
    [...new Set(catalog.items.map(item => item.siteVersionId))],
  )

  return decodeOpenAgentsSitePaymentDiscoveryProjection({
    agentReadable: projectedCatalog.items.some(item => item.agentReadable),
    audience: input.audience,
    docsRefs: safeRefs([
      'docs.openagents.site_payment_discovery',
      'docs.openagents.site_checkout_intents',
      'docs.openagents.site_l402',
      'docs.openagents.wfp_site_payment_middleware',
    ]),
    endpoints,
    items: projectedCatalog.items.map(item =>
      itemDiscovery(endpoints, item, catalog),
    ),
    l402HeaderRef: 'WWW-Authenticate: L402',
    provider: 'openagents_hosted_mdk',
    redactionRefs: safeRefs([
      'redaction.customer_private_data_not_exposed',
      'redaction.payment_material_not_exposed',
      'redaction.provider_authority_not_exposed',
      'redaction.checkout_query_state_not_required',
    ]),
    siteId: input.siteId,
    siteVersionIds,
    surfaceStates: currentSurfaceStates,
  })
}
