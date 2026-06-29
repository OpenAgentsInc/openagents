import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import { decodeUnknownWithSchema } from './json-boundary'
import {
  OpenAgentsPaymentPolicyAudience,
  OpenAgentsPaymentPolicySurface,
} from './payment-limit-policy'
import { openAgentsRunnerGatewayPayloadHasPrivateMaterial } from './runner-gateway'

export const OpenAgentsPaidEndpointBindingKind = S.Literals([
  'agent_api_endpoint',
  'forum_paid_action',
  'runner_recovery',
  'site_checkout',
  'site_paid_action',
])
export type OpenAgentsPaidEndpointBindingKind =
  typeof OpenAgentsPaidEndpointBindingKind.Type

export const OpenAgentsPaidEndpointMethod = S.Literals([
  'GET',
  'POST',
  'PUT',
  'PATCH',
  'DELETE',
])
export type OpenAgentsPaidEndpointMethod =
  typeof OpenAgentsPaidEndpointMethod.Type

export const OpenAgentsPaidEndpointAsset = S.Literals([
  'bitcoin',
  'credits',
  'usd',
])
export type OpenAgentsPaidEndpointAsset =
  typeof OpenAgentsPaidEndpointAsset.Type

export const OpenAgentsPaidEndpointDenomination = S.Literals([
  'bitcoin_millisatoshi',
  'credit',
  'usd_cent',
])
export type OpenAgentsPaidEndpointDenomination =
  typeof OpenAgentsPaidEndpointDenomination.Type

export const OpenAgentsPaidEndpointEntitlementKind = S.Literals([
  'duration',
  'duration_quota',
  'quota',
  'resource',
])
export type OpenAgentsPaidEndpointEntitlementKind =
  typeof OpenAgentsPaidEndpointEntitlementKind.Type

export const OpenAgentsPaidEndpointProductStatus = S.Literals([
  'active',
  'draft',
  'paused',
  'retired',
])
export type OpenAgentsPaidEndpointProductStatus =
  typeof OpenAgentsPaidEndpointProductStatus.Type

export const OpenAgentsPaidEndpointProjectionPolicy = S.Literals([
  'agent_visible',
  'customer_visible',
  'operator_only',
  'public_visible',
])
export type OpenAgentsPaidEndpointProjectionPolicy =
  typeof OpenAgentsPaidEndpointProjectionPolicy.Type

export const OpenAgentsPaidEndpointPrice = S.Struct({
  amountMinorUnits: S.Number,
  asset: OpenAgentsPaidEndpointAsset,
  denomination: OpenAgentsPaidEndpointDenomination,
})
export type OpenAgentsPaidEndpointPrice =
  typeof OpenAgentsPaidEndpointPrice.Type

export const OpenAgentsPaidEndpointBinding = S.Struct({
  actionRef: S.NullOr(S.String),
  kind: OpenAgentsPaidEndpointBindingKind,
  method: S.NullOr(OpenAgentsPaidEndpointMethod),
  pathTemplate: S.NullOr(S.String),
  resourceRef: S.String,
})
export type OpenAgentsPaidEndpointBinding =
  typeof OpenAgentsPaidEndpointBinding.Type

export const OpenAgentsPaidEndpointEntitlement = S.Struct({
  durationSeconds: S.NullOr(S.Number),
  kind: OpenAgentsPaidEndpointEntitlementKind,
  quotaUnits: S.NullOr(S.Number),
  scopeRefs: S.Array(S.String),
})
export type OpenAgentsPaidEndpointEntitlement =
  typeof OpenAgentsPaidEndpointEntitlement.Type

export const OpenAgentsPaidEndpointProductRecord = S.Struct({
  binding: OpenAgentsPaidEndpointBinding,
  displayName: S.String,
  entitlement: OpenAgentsPaidEndpointEntitlement,
  internalEconomicsRefs: S.Array(S.String),
  operatorNoteRefs: S.Array(S.String),
  price: OpenAgentsPaidEndpointPrice,
  productId: S.String,
  projectionPolicy: OpenAgentsPaidEndpointProjectionPolicy,
  providerBindingRefs: S.Array(S.String),
  publicAgentDocRefs: S.Array(S.String),
  publicSummaryRef: S.String,
  spendCapHintRefs: S.Array(S.String),
  status: OpenAgentsPaidEndpointProductStatus,
  surface: OpenAgentsPaymentPolicySurface,
})
export type OpenAgentsPaidEndpointProductRecord =
  typeof OpenAgentsPaidEndpointProductRecord.Type

export const OpenAgentsPaidEndpointProductProjection = S.Struct({
  audience: OpenAgentsPaymentPolicyAudience,
  binding: OpenAgentsPaidEndpointBinding,
  displayName: S.String,
  entitlement: OpenAgentsPaidEndpointEntitlement,
  internalEconomicsRefs: S.Array(S.String),
  operatorNoteRefs: S.Array(S.String),
  price: OpenAgentsPaidEndpointPrice,
  productId: S.String,
  projectionPolicy: OpenAgentsPaidEndpointProjectionPolicy,
  providerBindingRefs: S.Array(S.String),
  publicAgentDocRefs: S.Array(S.String),
  publicSummaryRef: S.String,
  spendCapHintRefs: S.Array(S.String),
  status: OpenAgentsPaidEndpointProductStatus,
  surface: OpenAgentsPaymentPolicySurface,
})
export type OpenAgentsPaidEndpointProductProjection =
  typeof OpenAgentsPaidEndpointProductProjection.Type

export const OpenAgentsPaidEndpointProductCatalog = S.Struct({
  products: S.Array(OpenAgentsPaidEndpointProductRecord),
})
export type OpenAgentsPaidEndpointProductCatalog =
  typeof OpenAgentsPaidEndpointProductCatalog.Type

export class OpenAgentsPaidEndpointProductCatalogUnsafe extends S.TaggedErrorClass<OpenAgentsPaidEndpointProductCatalogUnsafe>()(
  'OpenAgentsPaidEndpointProductCatalogUnsafe',
  {
    reason: S.String,
  },
) {}

const stableCatalogIdPattern = /^[a-z][a-z0-9]*(?:[._:-][a-z0-9]+)*$/

const unsafeCatalogKeyPattern =
  /(access[_-]?token|bearer|callback[_-]?token|cookie|customer[_-]?(email|name)|email[_-]?body|grant|invoice|mdk|mnemonic|oauth|payment[_-]?(hash|preimage)|preimage|private[_-]?key|provider[_-]?(account|grant|payload|token)|raw[_-]?(invoice|payment|prompt|runner|run[_-]?log)|secret|source[_-]?archive|wallet|webhook)/i

const unsafeCatalogValuePattern =
  /(bearer\s+|checkout_id=|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github[_-]?pat_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|mdk_access_token|mnemonic|payment_hash=|payment_preimage=|preimage|provider[_-]?token|raw[_-]?invoice|raw[_-]?payment|raw[_-]?prompt|raw[_-]?runner|raw[_-]?run[_-]?log|secret|sk-[a-z0-9]|\S+@\S+)/i

const scanForUnsafeCatalogMaterial = (
  value: unknown,
  path: ReadonlyArray<string> = [],
): string | undefined => {
  if (typeof value === 'string') {
    return containsProviderSecretMaterial(value) ||
      unsafeCatalogValuePattern.test(value) ||
      openAgentsRunnerGatewayPayloadHasPrivateMaterial(value)
      ? path.join('.') || '<root>'
      : undefined
  }

  if (Array.isArray(value)) {
    for (const [index, item] of value.entries()) {
      const unsafePath = scanForUnsafeCatalogMaterial(item, [
        ...path,
        String(index),
      ])

      if (unsafePath !== undefined) {
        return unsafePath
      }
    }

    return undefined
  }

  if (value === null || typeof value !== 'object') {
    return undefined
  }

  if (openAgentsRunnerGatewayPayloadHasPrivateMaterial(value)) {
    return path.join('.') || '<root>'
  }

  for (const [key, item] of Object.entries(value)) {
    if (unsafeCatalogKeyPattern.test(key)) {
      return [...path, key].join('.')
    }

    const unsafePath = scanForUnsafeCatalogMaterial(item, [...path, key])

    if (unsafePath !== undefined) {
      return unsafePath
    }
  }

  return undefined
}

const safeCatalogRef = (ref: string): string | undefined =>
  scanForUnsafeCatalogMaterial(ref) === undefined && ref.trim() !== ''
    ? ref
    : undefined

const safeCatalogRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs)]
    .map(ref => safeCatalogRef(ref))
    .filter((ref): ref is string => ref !== undefined)

const assertCondition = (
  condition: boolean,
  reason: string,
): void => {
  if (!condition) {
    throw new OpenAgentsPaidEndpointProductCatalogUnsafe({ reason })
  }
}

const assertStableId = (value: string, fieldPath: string): void =>
  assertCondition(
    stableCatalogIdPattern.test(value),
    `${fieldPath} must be a stable lowercase catalog id.`,
  )

const assertPositiveInteger = (value: number, fieldPath: string): void =>
  assertCondition(
    Number.isInteger(value) && value > 0,
    `${fieldPath} must be a positive integer.`,
  )

const assertPrice = (
  price: OpenAgentsPaidEndpointPrice,
  fieldPath: string,
): void => {
  assertPositiveInteger(price.amountMinorUnits, `${fieldPath}.amountMinorUnits`)

  const expectedDenomination =
    price.asset === 'usd'
      ? 'usd_cent'
      : price.asset === 'bitcoin'
        ? 'bitcoin_millisatoshi'
        : 'credit'

  assertCondition(
    price.denomination === expectedDenomination,
    `${fieldPath}.denomination must match ${price.asset}.`,
  )
}

const assertPathTemplate = (
  pathTemplate: string | null,
  fieldPath: string,
): void => {
  if (pathTemplate === null) {
    return
  }

  assertCondition(
    pathTemplate.startsWith('/'),
    `${fieldPath} must be a site-local or API-local absolute path.`,
  )
  assertCondition(
    !pathTemplate.includes('?') && !pathTemplate.includes('#'),
    `${fieldPath} must not include query strings or fragments.`,
  )
}

const assertBinding = (
  binding: OpenAgentsPaidEndpointBinding,
  fieldPath: string,
): void => {
  assertStableId(binding.resourceRef, `${fieldPath}.resourceRef`)
  assertPathTemplate(binding.pathTemplate, `${fieldPath}.pathTemplate`)

  if (
    binding.kind === 'agent_api_endpoint' ||
    binding.kind === 'site_paid_action'
  ) {
    assertCondition(
      binding.method !== null && binding.pathTemplate !== null,
      `${fieldPath} requires method and pathTemplate for endpoint bindings.`,
    )
  }

  if (
    binding.kind === 'forum_paid_action' ||
    binding.kind === 'runner_recovery' ||
    binding.kind === 'site_checkout'
  ) {
    assertCondition(
      binding.actionRef !== null,
      `${fieldPath} requires actionRef for action bindings.`,
    )
  }

  if (binding.actionRef !== null) {
    assertStableId(binding.actionRef, `${fieldPath}.actionRef`)
  }
}

const assertEntitlement = (
  entitlement: OpenAgentsPaidEndpointEntitlement,
  fieldPath: string,
): void => {
  assertCondition(
    entitlement.scopeRefs.length > 0,
    `${fieldPath}.scopeRefs must include at least one entitlement scope.`,
  )
  for (const [index, scopeRef] of entitlement.scopeRefs.entries()) {
    assertStableId(scopeRef, `${fieldPath}.scopeRefs.${index}`)
  }

  if (
    entitlement.kind === 'duration' ||
    entitlement.kind === 'duration_quota'
  ) {
    const durationSeconds = entitlement.durationSeconds

    if (durationSeconds === null) {
      throw new OpenAgentsPaidEndpointProductCatalogUnsafe({
        reason:
          `${fieldPath}.durationSeconds is required for duration entitlements.`,
      })
    }

    assertPositiveInteger(durationSeconds, `${fieldPath}.durationSeconds`)
  }

  if (entitlement.kind === 'quota' || entitlement.kind === 'duration_quota') {
    const quotaUnits = entitlement.quotaUnits

    if (quotaUnits === null) {
      throw new OpenAgentsPaidEndpointProductCatalogUnsafe({
        reason: `${fieldPath}.quotaUnits is required for quota entitlements.`,
      })
    }

    assertPositiveInteger(quotaUnits, `${fieldPath}.quotaUnits`)
  }
}

const assertProductRecord = (
  product: OpenAgentsPaidEndpointProductRecord,
  index: number,
): void => {
  const fieldPath = `products.${index}`

  assertStableId(product.productId, `${fieldPath}.productId`)
  assertCondition(
    product.displayName.trim() !== '',
    `${fieldPath}.displayName is required.`,
  )
  assertPrice(product.price, `${fieldPath}.price`)
  assertBinding(product.binding, `${fieldPath}.binding`)
  assertEntitlement(product.entitlement, `${fieldPath}.entitlement`)
}

const sanitizeBinding = (
  binding: OpenAgentsPaidEndpointBinding,
): OpenAgentsPaidEndpointBinding => ({
  actionRef:
    binding.actionRef === null ? null : safeCatalogRef(binding.actionRef) ?? null,
  kind: binding.kind,
  method: binding.method,
  pathTemplate:
    binding.pathTemplate === null
      ? null
      : safeCatalogRef(binding.pathTemplate) ?? null,
  resourceRef: safeCatalogRef(binding.resourceRef) ?? 'redacted_resource',
})

const sanitizeEntitlement = (
  entitlement: OpenAgentsPaidEndpointEntitlement,
): OpenAgentsPaidEndpointEntitlement => ({
  durationSeconds: entitlement.durationSeconds,
  kind: entitlement.kind,
  quotaUnits: entitlement.quotaUnits,
  scopeRefs: safeCatalogRefs(entitlement.scopeRefs),
})

export const decodeOpenAgentsPaidEndpointProductCatalog = (
  value: unknown,
): OpenAgentsPaidEndpointProductCatalog => {
  const unsafePath = scanForUnsafeCatalogMaterial(value)

  if (unsafePath !== undefined) {
    throw new OpenAgentsPaidEndpointProductCatalogUnsafe({
      reason: `Paid endpoint product catalog contains private or payment-secret material at ${unsafePath}.`,
    })
  }

  const catalog = decodeUnknownWithSchema(
    OpenAgentsPaidEndpointProductCatalog,
    value,
  )
  const productIds = new Set<string>()

  for (const [index, product] of catalog.products.entries()) {
    assertProductRecord(product, index)
    assertCondition(
      !productIds.has(product.productId),
      `products.${index}.productId duplicates an existing product id.`,
    )
    productIds.add(product.productId)
  }

  return catalog
}

export const projectOpenAgentsPaidEndpointProduct = (
  product: OpenAgentsPaidEndpointProductRecord,
  audience: typeof OpenAgentsPaymentPolicyAudience.Type,
): OpenAgentsPaidEndpointProductProjection => {
  const operator = audience === 'operator'

  return {
    audience,
    binding: sanitizeBinding(product.binding),
    displayName: product.displayName,
    entitlement: sanitizeEntitlement(product.entitlement),
    internalEconomicsRefs: operator
      ? safeCatalogRefs(product.internalEconomicsRefs)
      : [],
    operatorNoteRefs: operator ? safeCatalogRefs(product.operatorNoteRefs) : [],
    price: product.price,
    productId: product.productId,
    projectionPolicy: product.projectionPolicy,
    providerBindingRefs: operator
      ? safeCatalogRefs(product.providerBindingRefs)
      : [],
    publicAgentDocRefs: safeCatalogRefs(product.publicAgentDocRefs),
    publicSummaryRef:
      safeCatalogRef(product.publicSummaryRef) ??
      'summary.paid_endpoint_product.redacted',
    spendCapHintRefs: safeCatalogRefs(product.spendCapHintRefs),
    status: product.status,
    surface: product.surface,
  }
}

export const projectOpenAgentsPaidEndpointCatalog = (
  catalog: OpenAgentsPaidEndpointProductCatalog,
  audience: typeof OpenAgentsPaymentPolicyAudience.Type,
): ReadonlyArray<OpenAgentsPaidEndpointProductProjection> =>
  catalog.products
    .filter(product => {
      if (audience === 'operator') {
        return true
      }

      return product.projectionPolicy !== 'operator_only'
    })
    .map(product => projectOpenAgentsPaidEndpointProduct(product, audience))

export const openAgentsPaidEndpointProductProjectionHasPrivateMaterial = (
  projection: OpenAgentsPaidEndpointProductProjection,
): boolean => scanForUnsafeCatalogMaterial(projection) !== undefined
