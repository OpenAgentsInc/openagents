import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  BuyerPaymentEntitlementRecord,
  BuyerPaymentLedgerProjection,
  BuyerPaymentReceiptRecord,
  BuyerPaymentRedemptionRecord,
  projectBuyerPaymentLedgerRecord,
} from './buyer-payment-ledger'
import {
  OpenAgentsPaidEndpointProductRecord,
  projectOpenAgentsPaidEndpointProduct,
} from './paid-endpoint-product-catalog'
import {
  OpenAgentsPaymentPolicyAudience,
  OpenAgentsPaymentPolicySurface,
} from './payment-limit-policy'
import { openAgentsRunnerGatewayPayloadHasPrivateMaterial } from './runner-gateway'

export const OpenAgentsBuyerPaymentEntitlementShape = S.Literals([
  'actor_bound',
  'hybrid',
  'one_shot',
  'quota',
  'resource_bound',
  'route_bound',
  'site_bound',
  'time_window',
])
export type OpenAgentsBuyerPaymentEntitlementShape =
  typeof OpenAgentsBuyerPaymentEntitlementShape.Type

export const OpenAgentsBuyerPaymentEntitlementDecisionStatus = S.Literals([
  'allow_existing',
  'blocked',
  'consume_one_shot',
  'create_entitlement',
  'decrement_quota',
  'duplicate_replay',
  'exhausted',
  'expired',
  'mismatched_actor',
  'mismatched_resource',
  'mismatched_route',
  'mismatched_scope',
  'mismatched_site',
  'payment_required',
  'renew_entitlement',
])
export type OpenAgentsBuyerPaymentEntitlementDecisionStatus =
  typeof OpenAgentsBuyerPaymentEntitlementDecisionStatus.Type

export const OpenAgentsBuyerPaymentEntitlementNextAction = S.Literals([
  'check_external_authority',
  'pay',
  'retry_with_correct_scope',
  'review_product_policy',
  'stop',
  'use_entitlement',
])
export type OpenAgentsBuyerPaymentEntitlementNextAction =
  typeof OpenAgentsBuyerPaymentEntitlementNextAction.Type

export const OpenAgentsBuyerPaymentEntitlementExternalAuthority = S.Struct({
  authorizationSatisfied: S.Boolean,
  authorizationRequired: S.Boolean,
  moderationSatisfied: S.Boolean,
  moderationRequired: S.Boolean,
  confidentialDataSatisfied: S.Boolean,
  confidentialDataRequired: S.Boolean,
  ownerWriteSatisfied: S.Boolean,
  ownerWriteRequired: S.Boolean,
  siteDeploySatisfied: S.Boolean,
  siteDeployRequired: S.Boolean,
  payoutSatisfied: S.Boolean,
  payoutRequired: S.Boolean,
})
export type OpenAgentsBuyerPaymentEntitlementExternalAuthority =
  typeof OpenAgentsBuyerPaymentEntitlementExternalAuthority.Type

export const OpenAgentsBuyerPaymentEntitlementAuthorityEffects = S.Struct({
  authorizesConfidentialData: S.Boolean,
  authorizesModerationBypass: S.Boolean,
  authorizesOwnerWrite: S.Boolean,
  authorizesPayout: S.Boolean,
  authorizesSiteDeploy: S.Boolean,
  authorizesUserAccess: S.Boolean,
})
export type OpenAgentsBuyerPaymentEntitlementAuthorityEffects =
  typeof OpenAgentsBuyerPaymentEntitlementAuthorityEffects.Type

export const OpenAgentsBuyerPaymentEntitlementPolicy = S.Struct({
  actorRef: S.NullOr(S.String),
  durationSeconds: S.NullOr(S.Number),
  metadataRefs: S.Array(S.String),
  policyRef: S.String,
  productId: S.String,
  quotaUnits: S.NullOr(S.Number),
  resourceRef: S.NullOr(S.String),
  routeRef: S.NullOr(S.String),
  scopeRefs: S.Array(S.String),
  shape: OpenAgentsBuyerPaymentEntitlementShape,
  siteRef: S.NullOr(S.String),
  surface: OpenAgentsPaymentPolicySurface,
})
export type OpenAgentsBuyerPaymentEntitlementPolicy =
  typeof OpenAgentsBuyerPaymentEntitlementPolicy.Type

export const OpenAgentsBuyerPaymentEntitlementPolicyInput = S.Struct({
  actorRef: S.String,
  audience: OpenAgentsPaymentPolicyAudience,
  entitlement: S.NullOr(BuyerPaymentEntitlementRecord),
  externalAuthority: OpenAgentsBuyerPaymentEntitlementExternalAuthority,
  idempotencyKeyHash: S.String,
  nowIso: S.String,
  policy: OpenAgentsBuyerPaymentEntitlementPolicy,
  priorIdempotencyKeyHashes: S.Array(S.String),
  product: OpenAgentsPaidEndpointProductRecord,
  receipt: S.NullOr(BuyerPaymentReceiptRecord),
  redemption: S.NullOr(BuyerPaymentRedemptionRecord),
  requestedResourceRef: S.String,
  requestedRouteRef: S.NullOr(S.String),
  requestedScopeRefs: S.Array(S.String),
  requestedSiteRef: S.NullOr(S.String),
  usageCount: S.Number,
})
export type OpenAgentsBuyerPaymentEntitlementPolicyInput =
  typeof OpenAgentsBuyerPaymentEntitlementPolicyInput.Type

export const OpenAgentsBuyerPaymentEntitlementPolicyDecision = S.Struct({
  actorRef: S.String,
  createEntitlement: S.Boolean,
  decisionRef: S.String,
  decrementQuota: S.Boolean,
  entitlementRef: S.NullOr(S.String),
  idempotencyKeyHash: S.String,
  metadataRefs: S.Array(S.String),
  nextAction: OpenAgentsBuyerPaymentEntitlementNextAction,
  productId: S.String,
  reasonRefs: S.Array(S.String),
  receiptRef: S.NullOr(S.String),
  redemptionRef: S.NullOr(S.String),
  remainingQuotaUnits: S.NullOr(S.Number),
  renewEntitlement: S.Boolean,
  scopeRefs: S.Array(S.String),
  status: OpenAgentsBuyerPaymentEntitlementDecisionStatus,
  surface: OpenAgentsPaymentPolicySurface,
  useEntitlement: S.Boolean,
})
export type OpenAgentsBuyerPaymentEntitlementPolicyDecision =
  typeof OpenAgentsBuyerPaymentEntitlementPolicyDecision.Type

export const OpenAgentsBuyerPaymentEntitlementPolicyProjection = S.Struct({
  actorRef: S.NullOr(S.String),
  audience: OpenAgentsPaymentPolicyAudience,
  authorityEffects: OpenAgentsBuyerPaymentEntitlementAuthorityEffects,
  createEntitlement: S.Boolean,
  decisionRef: S.String,
  decrementQuota: S.Boolean,
  entitlement: S.NullOr(BuyerPaymentLedgerProjection),
  entitlementRef: S.NullOr(S.String),
  expiryLabelRef: S.String,
  nextAction: OpenAgentsBuyerPaymentEntitlementNextAction,
  product: S.Struct({
    displayName: S.String,
    productId: S.String,
    status: S.String,
  }),
  quotaUnits: S.NullOr(S.Number),
  reasonRefs: S.Array(S.String),
  receipt: S.NullOr(BuyerPaymentLedgerProjection),
  receiptRef: S.NullOr(S.String),
  redemption: S.NullOr(BuyerPaymentLedgerProjection),
  redemptionRef: S.NullOr(S.String),
  remainingQuotaUnits: S.NullOr(S.Number),
  renewEntitlement: S.Boolean,
  resourceRef: S.String,
  routeRef: S.NullOr(S.String),
  safeBody: S.Record(S.String, S.Unknown),
  scopeRefs: S.Array(S.String),
  shape: OpenAgentsBuyerPaymentEntitlementShape,
  siteRef: S.NullOr(S.String),
  status: OpenAgentsBuyerPaymentEntitlementDecisionStatus,
  statusCode: S.Number,
  surface: OpenAgentsPaymentPolicySurface,
  useEntitlement: S.Boolean,
})
export type OpenAgentsBuyerPaymentEntitlementPolicyProjection =
  typeof OpenAgentsBuyerPaymentEntitlementPolicyProjection.Type

export class OpenAgentsBuyerPaymentEntitlementPolicyUnsafe extends S.TaggedErrorClass<OpenAgentsBuyerPaymentEntitlementPolicyUnsafe>()(
  'OpenAgentsBuyerPaymentEntitlementPolicyUnsafe',
  {
    reason: S.String,
  },
) {}

const stableRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/
const timestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const timestampKeys = new Set([
  'archivedAt',
  'consumedAt',
  'createdAt',
  'expiresAt',
  'nowIso',
  'updatedAt',
])
const unsafeKeyPattern =
  /(access[_-]?token|bearer|callback[_-]?token|checkout[_-]?id|cookie|customer[_-]?(email|name)|email[_-]?body|invoice|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|preimage|proof)|preimage|private[_-]?key|provider[_-]?(account|grant|payload|token)|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log)|secret|source[_-]?archive|wallet|webhook)/i
const unsafeValuePattern =
  /(bearer\s+|checkout_id=|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github[_-]?pat_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|preimage|proof)|preimage=[A-Za-z0-9_-]+|provider[_-]?token|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log)|secret|sk-[a-z0-9]|\S+@\S+|wallet[_-]?state)/i

const scanForUnsafeEntitlementPolicyMaterial = (
  value: unknown,
  path: ReadonlyArray<string> = [],
): string | undefined => {
  if (typeof value === 'string') {
    const key = path.at(-1)

    if (
      key !== undefined &&
      timestampKeys.has(key) &&
      timestampPattern.test(value)
    ) {
      return undefined
    }

    return containsProviderSecretMaterial(value) ||
      unsafeValuePattern.test(value) ||
      openAgentsRunnerGatewayPayloadHasPrivateMaterial(value)
      ? path.join('.') || '<root>'
      : undefined
  }

  if (Array.isArray(value)) {
    return value
      .map((item, index) =>
        scanForUnsafeEntitlementPolicyMaterial(item, [
          ...path,
          String(index),
        ]),
      )
      .find((unsafePath): unsafePath is string => unsafePath !== undefined)
  }

  if (value === null || typeof value !== 'object') {
    return undefined
  }

  if (openAgentsRunnerGatewayPayloadHasPrivateMaterial(value)) {
    return path.join('.') || '<root>'
  }

  return Object.entries(value)
    .map(([key, item]) =>
      unsafeKeyPattern.test(key)
        ? [...path, key].join('.')
        : scanForUnsafeEntitlementPolicyMaterial(item, [...path, key]),
    )
    .find((unsafePath): unsafePath is string => unsafePath !== undefined)
}

const assertSafeEntitlementPolicyValue = (
  label: string,
  value: unknown,
): void => {
  const unsafePath = scanForUnsafeEntitlementPolicyMaterial(value)

  if (unsafePath !== undefined) {
    throw new OpenAgentsBuyerPaymentEntitlementPolicyUnsafe({
      reason:
        `${label} contains private or payment-secret material at ${unsafePath}.`,
    })
  }
}

const safeRef = (value: string): string | undefined =>
  value.trim() !== '' &&
  stableRefPattern.test(value) &&
  scanForUnsafeEntitlementPolicyMaterial(value) === undefined
    ? value
    : undefined

const nullableSafeRef = (
  value: string | null | undefined,
): string | null =>
  value === null || value === undefined ? null : safeRef(value) ?? null

const safeRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs)]
    .map(ref => safeRef(ref))
    .filter((ref): ref is string => ref !== undefined)

const authorityEffects: OpenAgentsBuyerPaymentEntitlementAuthorityEffects = {
  authorizesConfidentialData: false,
  authorizesModerationBypass: false,
  authorizesOwnerWrite: false,
  authorizesPayout: false,
  authorizesSiteDeploy: false,
  authorizesUserAccess: false,
}

const inferShapeFromProduct = (
  product: OpenAgentsPaidEndpointProductRecord,
): OpenAgentsBuyerPaymentEntitlementShape =>
  product.entitlement.kind === 'duration'
    ? 'time_window'
    : product.entitlement.kind === 'quota' &&
      product.entitlement.quotaUnits === 1
      ? 'one_shot'
      : product.entitlement.kind === 'quota' ||
        product.entitlement.kind === 'duration_quota'
        ? 'quota'
        : 'resource_bound'

export const openAgentsBuyerPaymentEntitlementPolicyFromProduct = (
  product: OpenAgentsPaidEndpointProductRecord,
  overrides: Partial<Pick<
    OpenAgentsBuyerPaymentEntitlementPolicy,
    | 'actorRef'
    | 'metadataRefs'
    | 'policyRef'
    | 'resourceRef'
    | 'routeRef'
    | 'shape'
    | 'siteRef'
  >> = {},
): OpenAgentsBuyerPaymentEntitlementPolicy => {
  assertSafeEntitlementPolicyValue(
    'OpenAgents buyer payment entitlement product',
    product,
  )

  return {
    actorRef: overrides.actorRef ?? null,
    durationSeconds: product.entitlement.durationSeconds,
    metadataRefs: safeRefs(overrides.metadataRefs ?? []),
    policyRef: overrides.policyRef ??
      `policy.buyer_payment_entitlement.${product.productId}`,
    productId: product.productId,
    quotaUnits: product.entitlement.quotaUnits,
    resourceRef: overrides.resourceRef ?? product.binding.resourceRef,
    routeRef: overrides.routeRef ?? product.binding.pathTemplate,
    scopeRefs: product.entitlement.scopeRefs,
    shape: overrides.shape ?? inferShapeFromProduct(product),
    siteRef: overrides.siteRef ?? null,
    surface: product.surface,
  }
}

const positiveQuotaUnits = (
  policy: OpenAgentsBuyerPaymentEntitlementPolicy,
): number | null =>
  policy.quotaUnits !== null && Number.isInteger(policy.quotaUnits) &&
  policy.quotaUnits > 0
    ? policy.quotaUnits
    : null

const requestedScopeMatches = (
  input: OpenAgentsBuyerPaymentEntitlementPolicyInput,
): boolean =>
  input.requestedScopeRefs.some(scopeRef =>
    input.policy.scopeRefs.includes(scopeRef),
  ) &&
  input.requestedScopeRefs.some(scopeRef =>
    input.product.entitlement.scopeRefs.includes(scopeRef),
  )

const productIsUsable = (
  product: OpenAgentsPaidEndpointProductRecord,
): boolean => product.status === 'active'

const redemptionIsUsable = (
  redemption: BuyerPaymentRedemptionRecord | null,
): boolean => redemption !== null && redemption.status === 'redeemed'

const entitlementExpired = (
  input: OpenAgentsBuyerPaymentEntitlementPolicyInput,
): boolean =>
  input.entitlement !== null &&
  input.entitlement.expiresAt !== null &&
  input.entitlement.expiresAt <= input.nowIso

const entitlementRecordScopeMatches = (
  input: OpenAgentsBuyerPaymentEntitlementPolicyInput,
): boolean =>
  input.entitlement !== null &&
  input.entitlement.productId === input.product.productId &&
  input.entitlement.scopeRefs.some(scopeRef =>
    input.policy.scopeRefs.includes(scopeRef),
  )

const externalAuthorityBlockedReason = (
  authority: OpenAgentsBuyerPaymentEntitlementExternalAuthority,
): string | undefined =>
  authority.authorizationRequired && !authority.authorizationSatisfied
    ? 'reason.buyer_payment_entitlement.authorization_required'
    : authority.moderationRequired && !authority.moderationSatisfied
      ? 'reason.buyer_payment_entitlement.moderation_required'
      : authority.confidentialDataRequired && !authority.confidentialDataSatisfied
        ? 'reason.buyer_payment_entitlement.confidential_data_required'
        : authority.ownerWriteRequired && !authority.ownerWriteSatisfied
          ? 'reason.buyer_payment_entitlement.owner_write_required'
          : authority.siteDeployRequired && !authority.siteDeploySatisfied
            ? 'reason.buyer_payment_entitlement.site_deploy_required'
            : authority.payoutRequired && !authority.payoutSatisfied
              ? 'reason.buyer_payment_entitlement.payout_policy_required'
              : undefined

const scopeBoundShapes = new Set<OpenAgentsBuyerPaymentEntitlementShape>([
  'hybrid',
  'resource_bound',
])
const routeBoundShapes = new Set<OpenAgentsBuyerPaymentEntitlementShape>([
  'hybrid',
  'route_bound',
])
const siteBoundShapes = new Set<OpenAgentsBuyerPaymentEntitlementShape>([
  'hybrid',
  'site_bound',
])
const actorBoundShapes = new Set<OpenAgentsBuyerPaymentEntitlementShape>([
  'actor_bound',
  'hybrid',
])

const scopeMismatchStatus = (
  input: OpenAgentsBuyerPaymentEntitlementPolicyInput,
): OpenAgentsBuyerPaymentEntitlementDecisionStatus | undefined =>
  !requestedScopeMatches(input)
    ? 'mismatched_scope'
    : scopeBoundShapes.has(input.policy.shape) &&
      input.policy.resourceRef !== null &&
      input.requestedResourceRef !== input.policy.resourceRef
      ? 'mismatched_resource'
      : routeBoundShapes.has(input.policy.shape) &&
        input.policy.routeRef !== null &&
        input.requestedRouteRef !== input.policy.routeRef
        ? 'mismatched_route'
        : siteBoundShapes.has(input.policy.shape) &&
          input.policy.siteRef !== null &&
          input.requestedSiteRef !== input.policy.siteRef
          ? 'mismatched_site'
          : actorBoundShapes.has(input.policy.shape) &&
            input.policy.actorRef !== null &&
            input.actorRef !== input.policy.actorRef
            ? 'mismatched_actor'
            : undefined

const existingEntitlementStatus = (
  input: OpenAgentsBuyerPaymentEntitlementPolicyInput,
): OpenAgentsBuyerPaymentEntitlementDecisionStatus | undefined => {
  if (input.entitlement === null) {
    return undefined
  }

  if (
    input.entitlement.productId !== input.product.productId ||
    !entitlementRecordScopeMatches(input)
  ) {
    return 'mismatched_scope'
  }

  if (input.entitlement.status === 'consumed') {
    return 'exhausted'
  }

  if (
    input.entitlement.status === 'revoked' ||
    input.entitlement.status === 'expired' ||
    entitlementExpired(input)
  ) {
    return redemptionIsUsable(input.redemption)
      ? 'renew_entitlement'
      : 'expired'
  }

  if (input.policy.shape === 'one_shot') {
    return input.entitlement.consumedAt === null
      ? 'consume_one_shot'
      : 'exhausted'
  }

  if (input.policy.shape === 'quota' || input.policy.shape === 'hybrid') {
    const quotaUnits = positiveQuotaUnits(input.policy)

    if (quotaUnits !== null) {
      return input.usageCount < quotaUnits
        ? 'decrement_quota'
        : 'exhausted'
    }
  }

  return 'allow_existing'
}

const statusForInput = (
  input: OpenAgentsBuyerPaymentEntitlementPolicyInput,
): OpenAgentsBuyerPaymentEntitlementDecisionStatus => {
  const externalBlockedReason =
    externalAuthorityBlockedReason(input.externalAuthority)
  const mismatchStatus = scopeMismatchStatus(input)
  const existingStatus = existingEntitlementStatus(input)

  return input.priorIdempotencyKeyHashes.includes(input.idempotencyKeyHash) ||
    input.redemption?.status === 'replayed' ||
    input.redemption?.replayed === 1
    ? 'duplicate_replay'
    : !productIsUsable(input.product) ||
      input.product.productId !== input.policy.productId ||
      input.product.surface !== input.policy.surface ||
      externalBlockedReason !== undefined
      ? 'blocked'
      : mismatchStatus !== undefined
        ? mismatchStatus
        : existingStatus !== undefined
          ? existingStatus
          : !redemptionIsUsable(input.redemption)
            ? 'payment_required'
            : input.policy.shape === 'one_shot'
              ? 'consume_one_shot'
              : 'create_entitlement'
}

const nextActionForStatus = (
  status: OpenAgentsBuyerPaymentEntitlementDecisionStatus,
): OpenAgentsBuyerPaymentEntitlementNextAction =>
  status === 'allow_existing' ||
  status === 'consume_one_shot' ||
  status === 'create_entitlement' ||
  status === 'decrement_quota' ||
  status === 'renew_entitlement'
    ? 'use_entitlement'
    : status === 'payment_required'
      ? 'pay'
      : status === 'mismatched_actor' ||
        status === 'mismatched_resource' ||
        status === 'mismatched_route' ||
        status === 'mismatched_scope' ||
        status === 'mismatched_site'
        ? 'retry_with_correct_scope'
        : status === 'blocked'
          ? 'check_external_authority'
          : status === 'expired'
            ? 'pay'
            : status === 'duplicate_replay'
              ? 'stop'
              : 'stop'

const statusCodeForStatus = (
  status: OpenAgentsBuyerPaymentEntitlementDecisionStatus,
): number =>
  status === 'payment_required' || status === 'expired'
    ? 402
    : status === 'blocked' ||
      status === 'mismatched_actor' ||
      status === 'mismatched_resource' ||
      status === 'mismatched_route' ||
      status === 'mismatched_scope' ||
      status === 'mismatched_site'
      ? 403
      : status === 'exhausted'
        ? 409
        : 200

const reasonForStatus = (
  input: OpenAgentsBuyerPaymentEntitlementPolicyInput,
  status: OpenAgentsBuyerPaymentEntitlementDecisionStatus,
): string => {
  const externalBlockedReason =
    externalAuthorityBlockedReason(input.externalAuthority)

  return status === 'blocked' && externalBlockedReason !== undefined
    ? externalBlockedReason
    : `reason.buyer_payment_entitlement.${status}`
}

const decisionRefForInput = (
  input: OpenAgentsBuyerPaymentEntitlementPolicyInput,
): string =>
  safeRef([
    'decision',
    'buyer_payment_entitlement',
    input.product.productId,
    input.idempotencyKeyHash,
  ].join('.')) ?? 'decision.buyer_payment_entitlement.redacted'

const entitlementRefForInput = (
  input: OpenAgentsBuyerPaymentEntitlementPolicyInput,
  status: OpenAgentsBuyerPaymentEntitlementDecisionStatus,
): string | null =>
  input.entitlement?.entitlementRef ??
  input.receipt?.entitlementRef ??
  input.redemption?.entitlementRef ??
  (status === 'create_entitlement' ||
    status === 'consume_one_shot' ||
    status === 'renew_entitlement'
    ? safeRef([
      'entitlement',
      'buyer_payment',
      input.product.productId,
      input.idempotencyKeyHash,
    ].join('.')) ?? 'entitlement.buyer_payment.redacted'
    : null)

const receiptRefForInput = (
  input: OpenAgentsBuyerPaymentEntitlementPolicyInput,
): string | null =>
  input.receipt?.receiptRef ?? input.redemption?.receiptRef ?? null

const redemptionRefForInput = (
  input: OpenAgentsBuyerPaymentEntitlementPolicyInput,
): string | null => input.redemption?.redemptionRef ?? null

const remainingQuotaUnitsForInput = (
  input: OpenAgentsBuyerPaymentEntitlementPolicyInput,
  status: OpenAgentsBuyerPaymentEntitlementDecisionStatus,
): number | null => {
  const quotaUnits = positiveQuotaUnits(input.policy)

  if (quotaUnits === null) {
    return null
  }

  if (status === 'consume_one_shot') {
    return 0
  }

  if (status === 'decrement_quota') {
    return Math.max(0, quotaUnits - input.usageCount - 1)
  }

  if (status === 'create_entitlement' || status === 'renew_entitlement') {
    return quotaUnits
  }

  return Math.max(0, quotaUnits - input.usageCount)
}

const expiryLabelRefForInput = (
  input: OpenAgentsBuyerPaymentEntitlementPolicyInput,
): string =>
  input.policy.durationSeconds !== null
    ? `expiry.buyer_payment_entitlement.duration_seconds.${input.policy.durationSeconds}`
    : input.entitlement?.expiresAt !== null &&
      input.entitlement?.expiresAt !== undefined &&
      input.entitlement.expiresAt <= input.nowIso
      ? 'expiry.buyer_payment_entitlement.expired'
      : 'expiry.buyer_payment_entitlement.not_time_limited'

const decisionForInput = (
  input: OpenAgentsBuyerPaymentEntitlementPolicyInput,
): OpenAgentsBuyerPaymentEntitlementPolicyDecision => {
  const status = statusForInput(input)
  const entitlementRef = entitlementRefForInput(input, status)
  const remainingQuotaUnits = remainingQuotaUnitsForInput(input, status)
  const reasonRefs = safeRefs([
    reasonForStatus(input, status),
    ...input.policy.metadataRefs,
  ])

  return {
    actorRef: nullableSafeRef(input.actorRef) ??
      'actor.buyer_payment_entitlement.redacted',
    createEntitlement:
      status === 'create_entitlement' ||
      (status === 'consume_one_shot' && input.entitlement === null),
    decisionRef: decisionRefForInput(input),
    decrementQuota: status === 'decrement_quota',
    entitlementRef,
    idempotencyKeyHash: input.idempotencyKeyHash,
    metadataRefs: safeRefs(input.policy.metadataRefs),
    nextAction: nextActionForStatus(status),
    productId: nullableSafeRef(input.product.productId) ??
      'product.buyer_payment_entitlement.redacted',
    reasonRefs,
    receiptRef: receiptRefForInput(input),
    redemptionRef: redemptionRefForInput(input),
    remainingQuotaUnits,
    renewEntitlement: status === 'renew_entitlement',
    scopeRefs: safeRefs(input.requestedScopeRefs),
    status,
    surface: input.policy.surface,
    useEntitlement:
      status === 'allow_existing' ||
      status === 'consume_one_shot' ||
      status === 'create_entitlement' ||
      status === 'decrement_quota' ||
      status === 'renew_entitlement',
  }
}

const projectionForDecision = (
  decision: OpenAgentsBuyerPaymentEntitlementPolicyDecision,
  input: OpenAgentsBuyerPaymentEntitlementPolicyInput,
): OpenAgentsBuyerPaymentEntitlementPolicyProjection => {
  const productProjection =
    projectOpenAgentsPaidEndpointProduct(input.product, input.audience)
  const actorRef =
    input.audience === 'operator' || input.audience === 'agent'
      ? nullableSafeRef(decision.actorRef)
      : null
  const receipt =
    input.receipt === null
      ? null
      : projectBuyerPaymentLedgerRecord('receipt', input.receipt, input.audience)
  const entitlement =
    input.entitlement === null
      ? null
      : projectBuyerPaymentLedgerRecord(
        'entitlement',
        input.entitlement,
        input.audience,
      )
  const redemption =
    input.redemption === null
      ? null
      : projectBuyerPaymentLedgerRecord(
        'redemption',
        input.redemption,
        input.audience,
      )

  return {
    actorRef,
    audience: input.audience,
    authorityEffects,
    createEntitlement: decision.createEntitlement,
    decisionRef: decision.decisionRef,
    decrementQuota: decision.decrementQuota,
    entitlement,
    entitlementRef: decision.entitlementRef,
    expiryLabelRef: expiryLabelRefForInput(input),
    nextAction: decision.nextAction,
    product: {
      displayName: productProjection.displayName,
      productId: productProjection.productId,
      status: productProjection.status,
    },
    quotaUnits: positiveQuotaUnits(input.policy),
    reasonRefs: decision.reasonRefs,
    receipt,
    receiptRef: decision.receiptRef,
    redemption,
    redemptionRef: decision.redemptionRef,
    remainingQuotaUnits: decision.remainingQuotaUnits,
    renewEntitlement: decision.renewEntitlement,
    resourceRef: nullableSafeRef(input.policy.resourceRef) ??
      'resource.buyer_payment_entitlement.redacted',
    routeRef: nullableSafeRef(input.policy.routeRef),
    safeBody: {
      action: 'buyer_payment_entitlement_policy',
      decisionRef: decision.decisionRef,
      entitlementRef: decision.entitlementRef,
      expiryLabelRef: expiryLabelRefForInput(input),
      nextAction: decision.nextAction,
      productId: decision.productId,
      quotaUnits: positiveQuotaUnits(input.policy),
      receiptRef: decision.receiptRef,
      redemptionRef: decision.redemptionRef,
      remainingQuotaUnits: decision.remainingQuotaUnits,
      status: decision.status,
    },
    scopeRefs: decision.scopeRefs,
    shape: input.policy.shape,
    siteRef: nullableSafeRef(input.policy.siteRef),
    status: decision.status,
    statusCode: statusCodeForStatus(decision.status),
    surface: decision.surface,
    useEntitlement: decision.useEntitlement,
  }
}

const projectionIsSafe = (
  projection: OpenAgentsBuyerPaymentEntitlementPolicyProjection,
): boolean =>
  scanForUnsafeEntitlementPolicyMaterial(projection) === undefined

export const openAgentsBuyerPaymentEntitlementPolicyHasPrivateMaterial = (
  value: unknown,
): boolean => scanForUnsafeEntitlementPolicyMaterial(value) !== undefined

export const evaluateOpenAgentsBuyerPaymentEntitlementPolicy = (
  input: OpenAgentsBuyerPaymentEntitlementPolicyInput,
): OpenAgentsBuyerPaymentEntitlementPolicyProjection => {
  assertSafeEntitlementPolicyValue(
    'OpenAgents buyer payment entitlement policy input',
    input,
  )

  const decision = decisionForInput(input)
  const projection = projectionForDecision(decision, input)

  if (!projectionIsSafe(projection)) {
    throw new OpenAgentsBuyerPaymentEntitlementPolicyUnsafe({
      reason:
        'OpenAgents buyer payment entitlement policy projection is not public-safe.',
    })
  }

  return projection
}
