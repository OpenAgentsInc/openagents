import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  BuyerPaymentChallengeRecord,
  BuyerPaymentEntitlementRecord,
  BuyerPaymentLedgerProjection,
  projectBuyerPaymentLedgerRecord,
} from './buyer-payment-ledger'
import { OpenAgentsHostedMdkCheckoutProjection } from './hosted-mdk-client'
import { OpenAgentsL402ResponseContract } from './l402-response-contract'
import {
  formatOpenAgentsL402WwwAuthenticate,
  OpenAgentsPaymentHeaderParseResult,
  OpenAgentsPaymentHeaderProjection,
  projectOpenAgentsPaymentHeaderResult,
} from './l402-payment-headers'
import {
  OpenAgentsPaidEndpointMethod,
  OpenAgentsPaidEndpointPrice,
} from './paid-endpoint-product-catalog'
import {
  classifyOpenAgentsPaymentLimitPolicy,
  OpenAgentsPaymentLimitPolicyProjection,
  OpenAgentsPaymentPolicyAudience,
  projectOpenAgentsPaymentLimitPolicyDecision,
} from './payment-limit-policy'
import { openAgentsRunnerGatewayPayloadHasPrivateMaterial } from './runner-gateway'
import {
  OpenAgentsSitePaymentCatalogRecord,
  openAgentsPaidEndpointProductFromSitePaymentCatalogItem,
} from './site-payment-catalog'
import {
  OpenAgentsSitePaymentEntitlementScope,
  OpenAgentsSitePaymentPublicProjectionState,
  OpenAgentsSitePaymentSettlementMode,
} from './site-payment-manifest'
import { paymentRequiredL402Response } from './l402-response-contract'

export const OpenAgentsSitePaymentMiddlewareDecisionStatus = S.Literals([
  'allow',
  'blocked',
  'entitlement_required',
  'payment_required',
])
export type OpenAgentsSitePaymentMiddlewareDecisionStatus =
  typeof OpenAgentsSitePaymentMiddlewareDecisionStatus.Type

export const OpenAgentsSitePaymentMiddlewareProtectedRoute = S.Struct({
  actionId: S.String,
  catalogRef: S.String,
  entitlementScope: OpenAgentsSitePaymentEntitlementScope,
  metadataRefs: S.Array(S.String),
  method: OpenAgentsPaidEndpointMethod,
  path: S.String,
  price: OpenAgentsPaidEndpointPrice,
  publicProjectionState: OpenAgentsSitePaymentPublicProjectionState,
  sandbox: S.Boolean,
  settlementMode: OpenAgentsSitePaymentSettlementMode,
  siteId: S.String,
  siteVersionId: S.String,
})
export type OpenAgentsSitePaymentMiddlewareProtectedRoute =
  typeof OpenAgentsSitePaymentMiddlewareProtectedRoute.Type

export const OpenAgentsSitePaymentMiddlewareInput = S.Struct({
  audience: OpenAgentsPaymentPolicyAudience,
  buyerPaymentChallenge: BuyerPaymentChallengeRecord,
  catalogItem: OpenAgentsSitePaymentCatalogRecord,
  entitlement: S.NullOr(BuyerPaymentEntitlementRecord),
  hostedCheckout: S.NullOr(OpenAgentsHostedMdkCheckoutProjection),
  paymentHeader: OpenAgentsPaymentHeaderParseResult,
  protectedRoute: OpenAgentsSitePaymentMiddlewareProtectedRoute,
})
export type OpenAgentsSitePaymentMiddlewareInput =
  typeof OpenAgentsSitePaymentMiddlewareInput.Type

export const OpenAgentsSitePaymentMiddlewareProjection = S.Struct({
  audience: OpenAgentsPaymentPolicyAudience,
  buyerPaymentChallenge: S.NullOr(BuyerPaymentLedgerProjection),
  decisionStatus: OpenAgentsSitePaymentMiddlewareDecisionStatus,
  entitlement: S.NullOr(BuyerPaymentLedgerProjection),
  hostedCheckoutUrlRef: S.NullOr(S.String),
  l402Response: S.NullOr(OpenAgentsL402ResponseContract),
  paymentHeader: OpenAgentsPaymentHeaderProjection,
  policyDecision: OpenAgentsPaymentLimitPolicyProjection,
  protectedRoute: OpenAgentsSitePaymentMiddlewareProtectedRoute,
  reasonRefs: S.Array(S.String),
  safeBody: S.Record(S.String, S.Unknown),
  statusCode: S.Number,
  wwwAuthenticate: S.NullOr(S.String),
})
export type OpenAgentsSitePaymentMiddlewareProjection =
  typeof OpenAgentsSitePaymentMiddlewareProjection.Type

export class OpenAgentsSitePaymentMiddlewareUnsafe extends S.TaggedErrorClass<OpenAgentsSitePaymentMiddlewareUnsafe>()(
  'OpenAgentsSitePaymentMiddlewareUnsafe',
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

const safeRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs)].filter(stableRefIsSafe)

const endpointRefForRoute = (
  protectedRoute: OpenAgentsSitePaymentMiddlewareProtectedRoute,
): string =>
  [
    'endpoint',
    'site_payment',
    protectedRoute.siteId,
    protectedRoute.siteVersionId,
    protectedRoute.actionId,
  ].join('.')

const routeMatchesCatalog = (
  protectedRoute: OpenAgentsSitePaymentMiddlewareProtectedRoute,
  catalogItem: OpenAgentsSitePaymentCatalogRecord,
): boolean =>
  catalogItem.itemKind === 'paid_action' &&
  catalogItem.status === 'active' &&
  protectedRoute.actionId === catalogItem.actionId &&
  protectedRoute.catalogRef === catalogItem.catalogRef &&
  protectedRoute.entitlementScope === catalogItem.entitlementScope &&
  protectedRoute.method === catalogItem.method &&
  protectedRoute.path === catalogItem.path &&
  protectedRoute.siteId === catalogItem.siteId &&
  protectedRoute.siteVersionId === catalogItem.siteVersionId &&
  protectedRoute.price.amountMinorUnits === catalogItem.price.amountMinorUnits &&
  protectedRoute.price.asset === catalogItem.price.asset &&
  protectedRoute.price.denomination === catalogItem.price.denomination

const routeIsSafe = (
  protectedRoute: OpenAgentsSitePaymentMiddlewareProtectedRoute,
): boolean =>
  stableRefIsSafe(protectedRoute.siteId) &&
  stableRefIsSafe(protectedRoute.siteVersionId) &&
  stableRefIsSafe(protectedRoute.catalogRef) &&
  stableIdIsSafe(protectedRoute.actionId) &&
  cleanPathIsSafe(protectedRoute.path) &&
  protectedRoute.metadataRefs.every(stableRefIsSafe) &&
  !valueHasPrivateMaterial(protectedRoute)

const entitlementAllows = (
  input: OpenAgentsSitePaymentMiddlewareInput,
): boolean => {
  const entitlement = input.entitlement
  const paidProduct =
    openAgentsPaidEndpointProductFromSitePaymentCatalogItem(input.catalogItem)

  return entitlement !== null &&
    entitlement.status === 'active' &&
    entitlement.productId === paidProduct.productId &&
    entitlement.scopeRefs.some(scopeRef =>
      paidProduct.entitlement.scopeRefs.includes(scopeRef),
    )
}

const basePolicyDecision = (
  input: OpenAgentsSitePaymentMiddlewareInput,
) =>
  classifyOpenAgentsPaymentLimitPolicy({
    entitlementScopeRefs: [
      `entitlement.site_payment.${input.protectedRoute.actionId}`,
    ],
    l402MdkAvailable: true,
    limitClass: 'economic_usage',
    publicSummaryRef: 'summary.site_payment_middleware.payment_required',
    requiredEndpointRefs: [input.protectedRoute.path],
    requiredProductRefs: [input.protectedRoute.catalogRef],
    surface: 'site_checkout',
  })

const blockedPolicyDecision = (
  input: OpenAgentsSitePaymentMiddlewareInput,
) =>
  classifyOpenAgentsPaymentLimitPolicy({
    limitClass: 'private_authority',
    reasonRefs: ['reason.site_payment_middleware.route_catalog_mismatch'],
    requiredEndpointRefs: [input.protectedRoute.path],
    surface: 'site_checkout',
  })

const paymentRequiredProjection = (
  input: OpenAgentsSitePaymentMiddlewareInput,
): OpenAgentsSitePaymentMiddlewareProjection => {
  const paidProduct =
    openAgentsPaidEndpointProductFromSitePaymentCatalogItem(input.catalogItem)
  const policyDecision = basePolicyDecision(input)
  const endpointRef = endpointRefForRoute(input.protectedRoute)
  const l402Response = paymentRequiredL402Response({
    audience: input.audience,
    challenge: input.buyerPaymentChallenge,
    endpointRef,
    policyDecision,
    product: paidProduct,
  })
  const wwwAuthenticate = formatOpenAgentsL402WwwAuthenticate({
    amount: input.buyerPaymentChallenge.price,
    challengeRef: input.buyerPaymentChallenge.challengeRef,
    docsRef: 'docs.openagents.site_payments',
    endpointRef,
    expiresAt: input.buyerPaymentChallenge.expiresAt,
    productId: paidProduct.productId,
  })

  return {
    audience: input.audience,
    buyerPaymentChallenge: projectBuyerPaymentLedgerRecord(
      'challenge',
      input.buyerPaymentChallenge,
      input.audience,
    ),
    decisionStatus: 'payment_required',
    entitlement: null,
    hostedCheckoutUrlRef: input.hostedCheckout?.checkoutUrlRef ?? null,
    l402Response,
    paymentHeader: projectOpenAgentsPaymentHeaderResult(
      input.paymentHeader,
      input.audience,
    ),
    policyDecision: projectOpenAgentsPaymentLimitPolicyDecision(
      policyDecision,
      input.audience,
    ),
    protectedRoute: input.protectedRoute,
    reasonRefs: safeRefs(['reason.site_payment_middleware.payment_required']),
    safeBody: {
      action: 'site_payment_required',
      challengeRef: input.buyerPaymentChallenge.challengeRef,
      checkoutUrlRef: input.hostedCheckout?.checkoutUrlRef ?? null,
      productId: paidProduct.productId,
    },
    statusCode: 402,
    wwwAuthenticate,
  }
}

const allowProjection = (
  input: OpenAgentsSitePaymentMiddlewareInput,
): OpenAgentsSitePaymentMiddlewareProjection => {
  const policyDecision = classifyOpenAgentsPaymentLimitPolicy({
    entitlementScopeRefs: [
      `entitlement.site_payment.${input.protectedRoute.actionId}`,
    ],
    freeBetaAvailable: true,
    limitClass: 'free_beta_allowance',
    publicSummaryRef: 'summary.site_payment_middleware.allowed',
    requiredEndpointRefs: [input.protectedRoute.path],
    surface: 'site_checkout',
  })

  return {
    audience: input.audience,
    buyerPaymentChallenge: null,
    decisionStatus: 'allow',
    entitlement: input.entitlement === null
      ? null
      : projectBuyerPaymentLedgerRecord(
        'entitlement',
        input.entitlement,
        input.audience,
      ),
    hostedCheckoutUrlRef: input.hostedCheckout?.checkoutUrlRef ?? null,
    l402Response: null,
    paymentHeader: projectOpenAgentsPaymentHeaderResult(
      input.paymentHeader,
      input.audience,
    ),
    policyDecision: projectOpenAgentsPaymentLimitPolicyDecision(
      policyDecision,
      input.audience,
    ),
    protectedRoute: input.protectedRoute,
    reasonRefs: safeRefs(['reason.site_payment_middleware.entitlement_active']),
    safeBody: {
      action: 'site_payment_allowed',
      entitlementRef: input.entitlement?.entitlementRef ?? null,
    },
    statusCode: 200,
    wwwAuthenticate: null,
  }
}

const entitlementRequiredProjection = (
  input: OpenAgentsSitePaymentMiddlewareInput,
): OpenAgentsSitePaymentMiddlewareProjection => {
  const policyDecision = basePolicyDecision(input)

  return {
    audience: input.audience,
    buyerPaymentChallenge: projectBuyerPaymentLedgerRecord(
      'challenge',
      input.buyerPaymentChallenge,
      input.audience,
    ),
    decisionStatus: 'entitlement_required',
    entitlement: null,
    hostedCheckoutUrlRef: input.hostedCheckout?.checkoutUrlRef ?? null,
    l402Response: null,
    paymentHeader: projectOpenAgentsPaymentHeaderResult(
      input.paymentHeader,
      input.audience,
    ),
    policyDecision: projectOpenAgentsPaymentLimitPolicyDecision(
      policyDecision,
      input.audience,
    ),
    protectedRoute: input.protectedRoute,
    reasonRefs: safeRefs([
      'reason.site_payment_middleware.payment_seen_entitlement_missing',
    ]),
    safeBody: {
      action: 'site_entitlement_required',
      challengeRef: input.buyerPaymentChallenge.challengeRef,
    },
    statusCode: 403,
    wwwAuthenticate: null,
  }
}

const blockedProjection = (
  input: OpenAgentsSitePaymentMiddlewareInput,
): OpenAgentsSitePaymentMiddlewareProjection => {
  const policyDecision = blockedPolicyDecision(input)

  return {
    audience: input.audience,
    buyerPaymentChallenge: null,
    decisionStatus: 'blocked',
    entitlement: null,
    hostedCheckoutUrlRef: null,
    l402Response: null,
    paymentHeader: projectOpenAgentsPaymentHeaderResult(
      input.paymentHeader,
      input.audience,
    ),
    policyDecision: projectOpenAgentsPaymentLimitPolicyDecision(
      policyDecision,
      input.audience,
    ),
    protectedRoute: input.protectedRoute,
    reasonRefs: safeRefs(['reason.site_payment_middleware.blocked']),
    safeBody: {
      action: 'site_payment_blocked',
      reasonRef: 'reason.site_payment_middleware.blocked',
    },
    statusCode: 403,
    wwwAuthenticate: null,
  }
}

const projectionIsSafe = (
  projection: OpenAgentsSitePaymentMiddlewareProjection,
): boolean =>
  !valueHasPrivateMaterial(projection) &&
  routeIsSafe(projection.protectedRoute)

export const openAgentsSitePaymentMiddlewareHasPrivateMaterial =
  valueHasPrivateMaterial

export const evaluateOpenAgentsSitePaymentMiddleware = (
  input: OpenAgentsSitePaymentMiddlewareInput,
): OpenAgentsSitePaymentMiddlewareProjection => {
  if (valueHasPrivateMaterial(input)) {
    throw new OpenAgentsSitePaymentMiddlewareUnsafe({
      reason:
        'Site payment middleware input must not contain customer private data, raw payment material, wallet state, provider grants, or secrets.',
    })
  }

  const projection =
    !routeIsSafe(input.protectedRoute) ||
    !routeMatchesCatalog(input.protectedRoute, input.catalogItem)
      ? blockedProjection(input)
      : entitlementAllows(input)
        ? allowProjection(input)
        : input.paymentHeader.status === 'l402_authorization' ||
          input.paymentHeader.status === 'x_openagents_l402' ||
          input.paymentHeader.status === 'lsat_authorization'
          ? entitlementRequiredProjection(input)
          : paymentRequiredProjection(input)

  if (!projectionIsSafe(projection)) {
    throw new OpenAgentsSitePaymentMiddlewareUnsafe({
      reason: 'Site payment middleware projection is not public-safe.',
    })
  }

  return projection
}
