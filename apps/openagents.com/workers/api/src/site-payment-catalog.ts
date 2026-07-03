import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import { OpenAgentsHostedMdkCheckoutRequest } from './hosted-mdk-client'
import {
  OpenAgentsPaidEndpointEntitlement,
  OpenAgentsPaidEndpointProductCatalog,
  OpenAgentsPaidEndpointProductRecord,
} from './paid-endpoint-product-catalog'
import { OpenAgentsPaymentPolicyAudience } from './payment-limit-policy'
import { openAgentsRunnerGatewayPayloadHasPrivateMaterial } from './runner-gateway'
import {
  OpenAgentsSitePaymentCustomerDataRequirement,
  OpenAgentsSitePaymentRecurringBilling,
  OpenAgentsSitePaymentEntitlementScope,
  OpenAgentsSitePaymentManifest,
  OpenAgentsSitePaymentPaidAction,
  OpenAgentsSitePaymentProduct,
  OpenAgentsSitePaymentPublicProjectionState,
  OpenAgentsSitePaymentSettlementMode,
  decodeOpenAgentsSitePaymentManifest,
} from './site-payment-manifest'

export const OpenAgentsSitePaymentCatalogItemKind = S.Literals([
  'paid_action',
  'product',
])
export type OpenAgentsSitePaymentCatalogItemKind =
  typeof OpenAgentsSitePaymentCatalogItemKind.Type

export const OpenAgentsSitePaymentCatalogStatus = S.Literals([
  'active',
  'archived',
  'draft',
  'retired',
])
export type OpenAgentsSitePaymentCatalogStatus =
  typeof OpenAgentsSitePaymentCatalogStatus.Type

export const OpenAgentsSitePaymentCatalogLinkage = S.Struct({
  deploymentId: S.NullOr(S.String),
  manifestRef: S.NullOr(S.String),
  orderRef: S.NullOr(S.String),
  siteId: S.String,
  siteVersionId: S.String,
  sourceManifestDigest: S.NullOr(S.String),
  workroomRef: S.NullOr(S.String),
})
export type OpenAgentsSitePaymentCatalogLinkage =
  typeof OpenAgentsSitePaymentCatalogLinkage.Type

const OpenAgentsSitePaymentCatalogCommonRecord = S.Struct({
  agentReadable: S.Boolean,
  archivedAt: S.NullOr(S.String),
  catalogRef: S.String,
  checkoutPath: S.String,
  createdAt: S.String,
  customerDataRequirements: S.Array(
    OpenAgentsSitePaymentCustomerDataRequirement,
  ),
  deploymentId: S.NullOr(S.String),
  displayRef: S.String,
  entitlementScope: OpenAgentsSitePaymentEntitlementScope,
  itemKind: OpenAgentsSitePaymentCatalogItemKind,
  manifestRef: S.NullOr(S.String),
  metadataRefs: S.Array(S.String),
  orderRef: S.NullOr(S.String),
  price: OpenAgentsPaidEndpointProductRecord.fields.price,
  publicProjectionState: OpenAgentsSitePaymentPublicProjectionState,
  recurringBilling: S.NullOr(OpenAgentsSitePaymentRecurringBilling),
  sandbox: S.Boolean,
  settlementMode: OpenAgentsSitePaymentSettlementMode,
  siteId: S.String,
  siteVersionId: S.String,
  sourceManifestDigest: S.NullOr(S.String),
  status: OpenAgentsSitePaymentCatalogStatus,
  updatedAt: S.String,
  workroomRef: S.NullOr(S.String),
})

export const OpenAgentsSitePaymentProductCatalogRecord = S.Struct({
  ...OpenAgentsSitePaymentCatalogCommonRecord.fields,
  itemKind: S.Literal('product'),
  productId: S.String,
})
export type OpenAgentsSitePaymentProductCatalogRecord =
  typeof OpenAgentsSitePaymentProductCatalogRecord.Type

export const OpenAgentsSitePaymentPaidActionCatalogRecord = S.Struct({
  ...OpenAgentsSitePaymentCatalogCommonRecord.fields,
  actionId: S.String,
  actionRef: S.String,
  itemKind: S.Literal('paid_action'),
  method: OpenAgentsPaidEndpointProductRecord.fields.binding.fields.method,
  path: S.String,
})
export type OpenAgentsSitePaymentPaidActionCatalogRecord =
  typeof OpenAgentsSitePaymentPaidActionCatalogRecord.Type

export const OpenAgentsSitePaymentCatalogRecord = S.Union([
  OpenAgentsSitePaymentProductCatalogRecord,
  OpenAgentsSitePaymentPaidActionCatalogRecord,
])
export type OpenAgentsSitePaymentCatalogRecord =
  typeof OpenAgentsSitePaymentCatalogRecord.Type

export const OpenAgentsSitePaymentCatalog = S.Struct({
  items: S.Array(OpenAgentsSitePaymentCatalogRecord),
})
export type OpenAgentsSitePaymentCatalog =
  typeof OpenAgentsSitePaymentCatalog.Type

export const OpenAgentsSitePaymentCatalogFromManifestInput = S.Struct({
  createdAt: S.String,
  deploymentId: S.NullOr(S.String),
  manifest: OpenAgentsSitePaymentManifest,
  manifestRef: S.NullOr(S.String),
  orderRef: S.NullOr(S.String),
  siteId: S.String,
  siteVersionId: S.String,
  sourceManifestDigest: S.NullOr(S.String),
  status: OpenAgentsSitePaymentCatalogStatus,
  updatedAt: S.String,
  workroomRef: S.NullOr(S.String),
})
export type OpenAgentsSitePaymentCatalogFromManifestInput =
  typeof OpenAgentsSitePaymentCatalogFromManifestInput.Type

export const OpenAgentsSitePaymentCatalogItemProjection = S.Struct({
  agentReadable: S.Boolean,
  audience: OpenAgentsPaymentPolicyAudience,
  catalogRef: S.String,
  checkoutPath: S.String,
  customerDataRequirements: S.Array(
    OpenAgentsSitePaymentCustomerDataRequirement,
  ),
  displayRef: S.String,
  entitlementScope: OpenAgentsSitePaymentEntitlementScope,
  itemKind: OpenAgentsSitePaymentCatalogItemKind,
  metadataRefs: S.Array(S.String),
  operatorRefs: S.Array(S.String),
  price: OpenAgentsPaidEndpointProductRecord.fields.price,
  publicProjectionState: OpenAgentsSitePaymentPublicProjectionState,
  recurringBilling: S.NullOr(OpenAgentsSitePaymentRecurringBilling),
  sandbox: S.Boolean,
  settlementMode: OpenAgentsSitePaymentSettlementMode,
  siteId: S.String,
  siteVersionId: S.String,
  status: OpenAgentsSitePaymentCatalogStatus,
})
export type OpenAgentsSitePaymentCatalogItemProjection =
  typeof OpenAgentsSitePaymentCatalogItemProjection.Type

export const OpenAgentsSitePaymentProductCatalogProjection = S.Struct({
  ...OpenAgentsSitePaymentCatalogItemProjection.fields,
  itemKind: S.Literal('product'),
  productId: S.String,
})
export type OpenAgentsSitePaymentProductCatalogProjection =
  typeof OpenAgentsSitePaymentProductCatalogProjection.Type

export const OpenAgentsSitePaymentPaidActionCatalogProjection = S.Struct({
  ...OpenAgentsSitePaymentCatalogItemProjection.fields,
  actionId: S.String,
  actionRef: S.String,
  itemKind: S.Literal('paid_action'),
  method: OpenAgentsPaidEndpointProductRecord.fields.binding.fields.method,
  path: S.String,
})
export type OpenAgentsSitePaymentPaidActionCatalogProjection =
  typeof OpenAgentsSitePaymentPaidActionCatalogProjection.Type

export const OpenAgentsSitePaymentCatalogProjection = S.Struct({
  audience: OpenAgentsPaymentPolicyAudience,
  items: S.Array(
    S.Union([
      OpenAgentsSitePaymentProductCatalogProjection,
      OpenAgentsSitePaymentPaidActionCatalogProjection,
    ]),
  ),
})
export type OpenAgentsSitePaymentCatalogProjection =
  typeof OpenAgentsSitePaymentCatalogProjection.Type

export const OpenAgentsSitePaymentCatalogHostedCheckoutPlan = S.Struct({
  catalogRecord: OpenAgentsSitePaymentCatalogRecord,
  hostedRequest: OpenAgentsHostedMdkCheckoutRequest,
})
export type OpenAgentsSitePaymentCatalogHostedCheckoutPlan =
  typeof OpenAgentsSitePaymentCatalogHostedCheckoutPlan.Type

export class OpenAgentsSitePaymentCatalogUnsafe extends S.TaggedErrorClass<OpenAgentsSitePaymentCatalogUnsafe>()(
  'OpenAgentsSitePaymentCatalogUnsafe',
  {
    reason: S.String,
  },
) {}

const stableCatalogIdPattern = /^[a-z][a-z0-9]*(?:[._:-][a-z0-9]+)*$/
const stableCatalogRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,180}$/
const unsafeCatalogKeyPattern =
  /(access[_-]?token|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|value)|email[_-]?body|grant|invoice|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|preimage)|preimage|private[_-]?key|provider[_-]?(account|grant|payload|token)|raw[_-]?(invoice|payment|prompt|runner|run[_-]?log)|secret|source[_-]?archive|wallet|webhook)/i
const unsafeCatalogValuePattern =
  /(bearer\s+|checkout_id=|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github[_-]?pat_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment_hash=|payment_preimage=|preimage=[A-Za-z0-9_-]+|provider[_-]?token|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log)|secret|sk-[a-z0-9]|\S+@\S+|wallet[_-]?state)/i

const valueHasPrivateMaterial = (value: unknown): boolean => {
  if (typeof value === 'string') {
    return containsProviderSecretMaterial(value) ||
      unsafeCatalogValuePattern.test(value) ||
      openAgentsRunnerGatewayPayloadHasPrivateMaterial(value)
  }

  if (Array.isArray(value)) {
    return value.some(valueHasPrivateMaterial)
  }

  if (value !== null && typeof value === 'object') {
    return openAgentsRunnerGatewayPayloadHasPrivateMaterial(value) ||
      Object.entries(value).some(([key, item]) =>
        unsafeCatalogKeyPattern.test(key) || valueHasPrivateMaterial(item),
      )
  }

  return false
}

const stableRefIsSafe = (value: string): boolean =>
  value.trim() !== '' &&
  stableCatalogRefPattern.test(value) &&
  !valueHasPrivateMaterial(value)

const stableIdIsSafe = (value: string): boolean =>
  stableCatalogIdPattern.test(value) && !valueHasPrivateMaterial(value)

const safeRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs)].filter(stableRefIsSafe)

const safeRefOrFallback = (ref: string, fallback: string): string =>
  stableRefIsSafe(ref) ? ref : fallback

const checkoutPathIsSafe = (value: string): boolean =>
  value.startsWith('/') &&
  !value.includes('?') &&
  !value.includes('#') &&
  !value.includes('://') &&
  !value.includes('//') &&
  !valueHasPrivateMaterial(value)

const optionalRefIsSafe = (value: string | null): boolean =>
  value === null || stableRefIsSafe(value)

const priceIsSupported = (
  price: OpenAgentsSitePaymentCatalogRecord['price'],
): boolean =>
  Number.isInteger(price.amountMinorUnits) &&
  price.amountMinorUnits > 0 &&
  (
    (price.asset === 'usd' && price.denomination === 'usd_cent') ||
    (price.asset === 'bitcoin' &&
      price.denomination === 'bitcoin_millisatoshi') ||
    (price.asset === 'credits' && price.denomination === 'credit')
  )

const recurringBillingIsSafe = (
  recurringBilling: OpenAgentsSitePaymentRecurringBilling | null,
): boolean =>
  recurringBilling === null ||
  (
    recurringBilling.entitlementRenewalMode === 'renew_on_payment_receipt' &&
    recurringBilling.renewalReceiptScopeRefs.length > 0 &&
    recurringBilling.renewalReceiptScopeRefs.every(stableRefIsSafe)
  )

const catalogIdSegment = (value: string, fallback: string): string => {
  const normalized = value
    .toLowerCase()
    .replaceAll(/[^a-z0-9]+/g, '_')
    .replaceAll(/^_+|_+$/g, '')
    .slice(0, 64)
  const candidate = normalized === '' ? fallback : normalized

  return /^[a-z]/u.test(candidate) ? candidate : `${fallback}_${candidate}`
}

const catalogRef = (
  linkage: OpenAgentsSitePaymentCatalogLinkage,
  itemKind: OpenAgentsSitePaymentCatalogItemKind,
  itemId: string,
): string =>
  [
    'site_payment',
    catalogIdSegment(linkage.siteId, 'site'),
    catalogIdSegment(linkage.siteVersionId, 'version'),
    itemKind,
    itemId,
  ].join(':')

const linkageFromInput = (
  input: OpenAgentsSitePaymentCatalogFromManifestInput,
): OpenAgentsSitePaymentCatalogLinkage => ({
  deploymentId: input.deploymentId,
  manifestRef: input.manifestRef,
  orderRef: input.orderRef,
  siteId: input.siteId,
  siteVersionId: input.siteVersionId,
  sourceManifestDigest: input.sourceManifestDigest,
  workroomRef: input.workroomRef,
})

const commonRecord = (
  input: OpenAgentsSitePaymentCatalogFromManifestInput,
  item: OpenAgentsSitePaymentProduct | OpenAgentsSitePaymentPaidAction,
  itemKind: OpenAgentsSitePaymentCatalogItemKind,
): typeof OpenAgentsSitePaymentCatalogCommonRecord.Type => {
  const linkage = linkageFromInput(input)
  const recurringBilling =
    itemKind === 'product' && 'recurringBilling' in item
      ? item.recurringBilling ?? null
      : null

  return {
    agentReadable: item.agentReadable,
    archivedAt: input.status === 'archived' ? input.updatedAt : null,
    catalogRef: catalogRef(linkage, itemKind, item.id),
    checkoutPath: item.checkoutPath,
    createdAt: input.createdAt,
    customerDataRequirements: item.customerDataRequirements,
    deploymentId: input.deploymentId,
    displayRef: item.displayRef,
    entitlementScope: item.entitlementScope,
    itemKind,
    manifestRef: input.manifestRef,
    metadataRefs: safeRefs([
      ...input.manifest.payments.metadataRefs,
      ...item.metadataRefs,
    ]),
    orderRef: input.orderRef,
    price: item.price,
    publicProjectionState: item.publicProjectionState,
    recurringBilling,
    sandbox: item.sandbox || input.manifest.payments.sandboxDefault,
    settlementMode: item.settlementMode,
    siteId: input.siteId,
    siteVersionId: input.siteVersionId,
    sourceManifestDigest: input.sourceManifestDigest,
    status: input.status,
    updatedAt: input.updatedAt,
    workroomRef: input.workroomRef,
  }
}

const productRecordFromManifest = (
  input: OpenAgentsSitePaymentCatalogFromManifestInput,
  product: OpenAgentsSitePaymentProduct,
): OpenAgentsSitePaymentProductCatalogRecord => ({
  ...commonRecord(input, product, 'product'),
  itemKind: 'product',
  productId: product.id,
})

const paidActionRecordFromManifest = (
  input: OpenAgentsSitePaymentCatalogFromManifestInput,
  action: OpenAgentsSitePaymentPaidAction,
): OpenAgentsSitePaymentPaidActionCatalogRecord => ({
  ...commonRecord(input, action, 'paid_action'),
  actionId: action.id,
  actionRef: action.actionRef,
  itemKind: 'paid_action',
  method: action.method,
  path: action.path,
})

const itemVisibleToAudience = (
  item: OpenAgentsSitePaymentCatalogRecord,
  audience: typeof OpenAgentsPaymentPolicyAudience.Type,
): boolean => {
  if (audience === 'operator') {
    return true
  }

  if (item.status === 'archived') {
    return false
  }

  if (audience === 'public') {
    return item.status === 'active' &&
      item.publicProjectionState !== 'hidden'
  }

  if (audience === 'agent') {
    return item.agentReadable
  }

  return true
}

const operatorRefsForItem = (
  item: OpenAgentsSitePaymentCatalogRecord,
  audience: typeof OpenAgentsPaymentPolicyAudience.Type,
): ReadonlyArray<string> =>
  audience === 'operator'
    ? safeRefs([
      item.deploymentId ?? '',
      item.manifestRef ?? '',
      item.orderRef ?? '',
      item.sourceManifestDigest ?? '',
      item.workroomRef ?? '',
    ])
    : []

const metadataRefsForItem = (
  item: OpenAgentsSitePaymentCatalogRecord,
  audience: typeof OpenAgentsPaymentPolicyAudience.Type,
): ReadonlyArray<string> =>
  audience === 'operator' || (audience === 'agent' && item.agentReadable)
    ? safeRefs(item.metadataRefs)
    : []

const commonProjection = (
  item: OpenAgentsSitePaymentCatalogRecord,
  audience: typeof OpenAgentsPaymentPolicyAudience.Type,
): typeof OpenAgentsSitePaymentCatalogItemProjection.Type => ({
  agentReadable: item.agentReadable,
  audience,
  catalogRef: safeRefOrFallback(item.catalogRef, 'site_payment:redacted'),
  checkoutPath: checkoutPathIsSafe(item.checkoutPath)
    ? item.checkoutPath
    : '/checkout/redacted',
  customerDataRequirements: item.customerDataRequirements.filter(
    requirement =>
      stableIdIsSafe(requirement.key) &&
      stableRefIsSafe(requirement.labelRef),
  ),
  displayRef: safeRefOrFallback(item.displayRef, 'display.redacted'),
  entitlementScope: item.entitlementScope,
  itemKind: item.itemKind,
  metadataRefs: metadataRefsForItem(item, audience),
  operatorRefs: operatorRefsForItem(item, audience),
  price: item.price,
  publicProjectionState: item.publicProjectionState,
  recurringBilling: item.recurringBilling,
  sandbox: item.sandbox,
  settlementMode: item.settlementMode,
  siteId: safeRefOrFallback(item.siteId, 'site.redacted'),
  siteVersionId: safeRefOrFallback(item.siteVersionId, 'version.redacted'),
  status: item.status,
})

const projectCatalogItem = (
  item: OpenAgentsSitePaymentCatalogRecord,
  audience: typeof OpenAgentsPaymentPolicyAudience.Type,
): OpenAgentsSitePaymentCatalogProjection['items'][number] =>
  item.itemKind === 'product'
    ? {
      ...commonProjection(item, audience),
      itemKind: 'product',
      productId: safeRefOrFallback(item.productId, 'product.redacted'),
    }
    : {
      ...commonProjection(item, audience),
      actionId: safeRefOrFallback(item.actionId, 'action.redacted'),
      actionRef: safeRefOrFallback(item.actionRef, 'action.redacted'),
      itemKind: 'paid_action',
      method: item.method,
      path: checkoutPathIsSafe(item.path) ? item.path : '/api/redacted',
    }

const projectionPolicyForItem = (
  item: OpenAgentsSitePaymentCatalogRecord,
): OpenAgentsPaidEndpointProductRecord['projectionPolicy'] => {
  if (item.publicProjectionState === 'hidden') {
    return 'operator_only'
  }

  if (item.publicProjectionState === 'redacted') {
    return 'customer_visible'
  }

  return 'public_visible'
}

const paidEndpointStatusForItem = (
  item: OpenAgentsSitePaymentCatalogRecord,
): OpenAgentsPaidEndpointProductRecord['status'] =>
  item.status === 'archived' ? 'retired' : item.status

const entitlementForItem = (
  item: OpenAgentsSitePaymentCatalogRecord,
): OpenAgentsPaidEndpointEntitlement => ({
  durationSeconds: item.recurringBilling?.interval === 'month'
    ? 2_678_400
    : item.recurringBilling?.interval === 'year'
      ? 31_536_000
      : null,
  kind: 'resource',
  quotaUnits: null,
  scopeRefs: [
    [
      'entitlement',
      'site_payment',
      catalogIdSegment(item.siteId, 'site'),
      catalogIdSegment(item.siteVersionId, 'version'),
      item.entitlementScope,
      catalogIdSegment(
        item.itemKind === 'product' ? item.productId : item.actionId,
        item.itemKind,
      ),
    ].join('.'),
    ...(item.recurringBilling === null
      ? []
      : [
          `entitlement_renewal.site_payment.${catalogIdSegment(item.siteId, 'site')}.${catalogIdSegment(item.siteVersionId, 'version')}.${item.recurringBilling.billingKind}.${catalogIdSegment(item.itemKind === 'product' ? item.productId : item.actionId, item.itemKind)}`,
          ...item.recurringBilling.renewalReceiptScopeRefs,
        ]),
  ],
})

const paidEndpointBindingForItem = (
  item: OpenAgentsSitePaymentCatalogRecord,
): OpenAgentsPaidEndpointProductRecord['binding'] =>
  item.itemKind === 'product'
    ? {
      actionRef: item.productId,
      kind: 'site_checkout',
      method: null,
      pathTemplate: item.checkoutPath,
      resourceRef: item.catalogRef,
    }
    : {
      actionRef: item.actionRef,
      kind: 'site_paid_action',
      method: item.method,
      pathTemplate: item.path,
      resourceRef: item.catalogRef,
    }

export const openAgentsSitePaymentCatalogHasPrivateMaterial =
  valueHasPrivateMaterial

export const decodeOpenAgentsSitePaymentCatalog = (
  value: unknown,
): OpenAgentsSitePaymentCatalog => {
  if (valueHasPrivateMaterial(value)) {
    throw new OpenAgentsSitePaymentCatalogUnsafe({
      reason:
        'Site payment catalog contains customer, payment, provider, wallet, runner, or secret material.',
    })
  }

  const catalog = S.decodeUnknownSync(OpenAgentsSitePaymentCatalog)(value)
  const catalogRefs = new Set(catalog.items.map(item => item.catalogRef))

  if (catalogRefs.size !== catalog.items.length) {
    throw new OpenAgentsSitePaymentCatalogUnsafe({
      reason: 'Site payment catalog item refs must be unique.',
    })
  }

  const unsafeItem = catalog.items.find(item =>
    !stableRefIsSafe(item.siteId) ||
    !stableRefIsSafe(item.siteVersionId) ||
    !stableIdIsSafe(item.itemKind === 'product' ? item.productId : item.actionId) ||
    !stableRefIsSafe(item.catalogRef) ||
    !stableRefIsSafe(item.displayRef) ||
    !checkoutPathIsSafe(item.checkoutPath) ||
    !optionalRefIsSafe(item.deploymentId) ||
    !optionalRefIsSafe(item.manifestRef) ||
    !optionalRefIsSafe(item.orderRef) ||
    !optionalRefIsSafe(item.sourceManifestDigest) ||
    !optionalRefIsSafe(item.workroomRef) ||
    !priceIsSupported(item.price) ||
    !recurringBillingIsSafe(item.recurringBilling) ||
    item.metadataRefs.some(ref => !stableRefIsSafe(ref)) ||
    item.customerDataRequirements.some(
      requirement =>
        !stableIdIsSafe(requirement.key) ||
        !stableRefIsSafe(requirement.labelRef),
    ) ||
    (
      item.itemKind === 'paid_action' &&
      (
        !stableRefIsSafe(item.actionRef) ||
        !checkoutPathIsSafe(item.path)
      )
    ),
  )

  if (unsafeItem !== undefined) {
    throw new OpenAgentsSitePaymentCatalogUnsafe({
      reason:
        'Site payment catalog items must use stable refs, supported prices, and clean local paths.',
    })
  }

  return catalog
}

export const sitePaymentCatalogFromManifest = (
  input: OpenAgentsSitePaymentCatalogFromManifestInput,
): OpenAgentsSitePaymentCatalog =>
  decodeOpenAgentsSitePaymentCatalog({
    items: [
      ...decodeOpenAgentsSitePaymentManifest(input.manifest).payments.products
        .map(product => productRecordFromManifest(input, product)),
      ...decodeOpenAgentsSitePaymentManifest(input.manifest).payments.paidActions
        .map(action => paidActionRecordFromManifest(input, action)),
    ],
  })

export const projectOpenAgentsSitePaymentCatalog = (
  catalog: OpenAgentsSitePaymentCatalog,
  audience: typeof OpenAgentsPaymentPolicyAudience.Type,
): OpenAgentsSitePaymentCatalogProjection => ({
  audience,
  items: catalog.items
    .filter(item => itemVisibleToAudience(item, audience))
    .map(item => projectCatalogItem(item, audience)),
})

export const openAgentsPaidEndpointProductFromSitePaymentCatalogItem = (
  item: OpenAgentsSitePaymentCatalogRecord,
): OpenAgentsPaidEndpointProductRecord => ({
  binding: paidEndpointBindingForItem(item),
  displayName: item.displayRef,
  entitlement: entitlementForItem(item),
  internalEconomicsRefs: [
    `internal_economics.site_payment.${catalogIdSegment(item.siteId, 'site')}`,
  ],
  operatorNoteRefs: [
    `operator_note.site_payment.${item.settlementMode}`,
  ],
  price: item.price,
  productId: item.catalogRef,
  projectionPolicy: projectionPolicyForItem(item),
  providerBindingRefs: ['provider_binding.openagents.hosted_mdk'],
  publicAgentDocRefs: item.agentReadable
    ? ['docs.openagents.site_payments']
    : [],
  publicSummaryRef: `summary.${item.catalogRef}`,
  spendCapHintRefs: [
    `spend_cap.${item.price.asset}.${item.entitlementScope}`,
  ],
  status: paidEndpointStatusForItem(item),
  surface: 'site_checkout',
})

export const paidEndpointCatalogFromSitePaymentCatalog = (
  catalog: OpenAgentsSitePaymentCatalog,
): typeof OpenAgentsPaidEndpointProductCatalog.Type => ({
  products: catalog.items.map(openAgentsPaidEndpointProductFromSitePaymentCatalogItem),
})
