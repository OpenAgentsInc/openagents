import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  OpenAgentsPaidEndpointDenomination,
  OpenAgentsPaidEndpointPrice,
  OpenAgentsPaidEndpointProductRecord,
} from './paid-endpoint-product-catalog'
import {
  OpenAgentsPaymentPolicyAudience,
  OpenAgentsPaymentPolicySurface,
} from './payment-limit-policy'
import {
  OpenAgentsUnifiedPaymentDecisionProjection,
  OpenAgentsUnifiedPaymentNextAction,
} from './unified-payment-decision'
import { openAgentsRunnerGatewayPayloadHasPrivateMaterial } from './runner-gateway'

export const OpenAgentsSpendCapPaymentRail = S.Literals([
  'bitcoin_l402_mdk',
  'credits',
  'existing_entitlement',
  'free_beta',
])
export type OpenAgentsSpendCapPaymentRail =
  typeof OpenAgentsSpendCapPaymentRail.Type

export const OpenAgentsSpendCapSettlementMode = S.Literals([
  'deferred_until_artifact_receipt',
  'deferred_until_response_closeout',
  'deferred_until_success',
  'immediate',
  'manual_operator_review',
])
export type OpenAgentsSpendCapSettlementMode =
  typeof OpenAgentsSpendCapSettlementMode.Type

export const OpenAgentsSpendCapPreviewStatus = S.Literals([
  'blocked',
  'catalog_missing',
  'exact_cap',
  'malformed_amount',
  'over_cap',
  'owner_grant_required',
  'private_route',
  'stale_catalog_entry',
  'unauthenticated_agent',
  'under_cap',
  'unsupported_rail',
  'wrong_currency',
])
export type OpenAgentsSpendCapPreviewStatus =
  typeof OpenAgentsSpendCapPreviewStatus.Type

export const OpenAgentsSpendCapPreviewNextAction = S.Literals([
  'add_credits',
  'ask_owner_for_grant',
  'fix_catalog',
  'fix_currency',
  'lower_spend_or_raise_cap',
  'pay_l402_mdk',
  'provide_agent_token',
  'request_manual_review',
  'spend_internal_credits',
  'stop',
  'use_entitlement',
  'use_free_beta',
])
export type OpenAgentsSpendCapPreviewNextAction =
  typeof OpenAgentsSpendCapPreviewNextAction.Type

export const OpenAgentsSpendCapPreviewRoute = S.Struct({
  method: S.NullOr(S.String),
  ownerGrantOnly: S.Boolean,
  path: S.NullOr(S.String),
  privateRoute: S.Boolean,
  routeRef: S.String,
})
export type OpenAgentsSpendCapPreviewRoute =
  typeof OpenAgentsSpendCapPreviewRoute.Type

export const OpenAgentsSpendCapPreviewSideEffectSummary = S.Struct({
  callsMdk: S.Boolean,
  createsEntitlement: S.Boolean,
  createsPaymentArtifact: S.Boolean,
  debitsCredits: S.Boolean,
  mutatesPayout: S.Boolean,
  redeemsCredentials: S.Boolean,
})
export type OpenAgentsSpendCapPreviewSideEffectSummary =
  typeof OpenAgentsSpendCapPreviewSideEffectSummary.Type

export const OpenAgentsSpendCapPreviewInput = S.Struct({
  actionRef: S.NullOr(S.String),
  actorRef: S.NullOr(S.String),
  agentAuthenticated: S.Boolean,
  audience: OpenAgentsPaymentPolicyAudience,
  availableCreditAllowanceMinorUnits: S.Number,
  freeAllowanceUses: S.NullOr(S.Number),
  idempotencyKeyHintRef: S.NullOr(S.String),
  idempotencyKeyRequired: S.Boolean,
  l402MdkRecoveryAvailable: S.Boolean,
  maxPerCall: OpenAgentsPaidEndpointPrice,
  maxPerWindow: OpenAgentsPaidEndpointPrice,
  nowIso: S.String,
  paymentDecision: OpenAgentsUnifiedPaymentDecisionProjection,
  price: OpenAgentsPaidEndpointPrice,
  product: S.NullOr(OpenAgentsPaidEndpointProductRecord),
  requestedRail: OpenAgentsSpendCapPaymentRail,
  retryBehaviorRefs: S.Array(S.String),
  route: OpenAgentsSpendCapPreviewRoute,
  settlementMode: OpenAgentsSpendCapSettlementMode,
  supportedRails: S.Array(OpenAgentsSpendCapPaymentRail),
  surface: OpenAgentsPaymentPolicySurface,
  windowSpent: OpenAgentsPaidEndpointPrice,
})
export type OpenAgentsSpendCapPreviewInput =
  typeof OpenAgentsSpendCapPreviewInput.Type

export const OpenAgentsSpendCapPreviewProjection = S.Struct({
  actionRef: S.NullOr(S.String),
  actorRef: S.NullOr(S.String),
  audience: OpenAgentsPaymentPolicyAudience,
  availableCreditAllowanceMinorUnits: S.Number,
  dryRun: S.Boolean,
  entitlementScopeRefs: S.Array(S.String),
  freeAllowanceUses: S.NullOr(S.Number),
  idempotencyGuidanceRefs: S.Array(S.String),
  l402MdkRecoveryAvailable: S.Boolean,
  maxPerCall: OpenAgentsPaidEndpointPrice,
  maxPerWindow: OpenAgentsPaidEndpointPrice,
  nextActions: S.Array(OpenAgentsSpendCapPreviewNextAction),
  paymentDecisionStatus: S.String,
  previewRef: S.String,
  price: OpenAgentsPaidEndpointPrice,
  productRef: S.NullOr(S.String),
  reasonRefs: S.Array(S.String),
  requestedRail: OpenAgentsSpendCapPaymentRail,
  retryBehaviorRefs: S.Array(S.String),
  routeRef: S.String,
  safeBody: S.Record(S.String, S.Unknown),
  settlementMode: OpenAgentsSpendCapSettlementMode,
  sideEffectSummary: OpenAgentsSpendCapPreviewSideEffectSummary,
  status: OpenAgentsSpendCapPreviewStatus,
  statusCode: S.Number,
  surface: OpenAgentsPaymentPolicySurface,
  windowRemainingMinorUnits: S.Number,
  windowSpent: OpenAgentsPaidEndpointPrice,
})
export type OpenAgentsSpendCapPreviewProjection =
  typeof OpenAgentsSpendCapPreviewProjection.Type

export class OpenAgentsSpendCapPreviewUnsafe extends S.TaggedErrorClass<OpenAgentsSpendCapPreviewUnsafe>()(
  'OpenAgentsSpendCapPreviewUnsafe',
  {
    reason: S.String,
  },
) {}

const stableRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/
const timestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const timestampKeys = new Set(['createdAt', 'expiresAt', 'nowIso', 'updatedAt'])
const publicLiteralValues = new Set([
  'add_credits',
  'agent',
  'allow',
  'ask_owner_for_grant',
  'bitcoin',
  'bitcoin_l402_mdk',
  'bitcoin_millisatoshi',
  'blocked',
  'credit',
  'credit_balance',
  'credits',
  'deferred_until_artifact_receipt',
  'deferred_until_response_closeout',
  'deferred_until_success',
  'existing_entitlement',
  'exhausted',
  'fix_catalog',
  'fix_currency',
  'free_beta',
  'hard_blocked',
  'immediate',
  'l402_mdk',
  'lower_spend_or_raise_cap',
  'manual_operator_review',
  'manual_review_required',
  'none',
  'operator',
  'pay_l402_mdk',
  'product_entitlement',
  'provide_agent_token',
  'provider_unavailable',
  'recoverable',
  'recoverable_by_l402_mdk',
  'request_manual_review',
  'site_checkout',
  'spend_internal_credits',
  'stop',
  'usd_cent',
  'use_entitlement',
  'use_free_beta',
])
const unsafeKeyPattern =
  /(access[_-]?token|bearer|callback[_-]?token|checkout[_-]?id|cookie|customer[_-]?(email|name|id)|email[_-]?body|invoice|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|method|preimage|proof)|preimage|private[_-]?key|provider[_-]?(account|grant|payload|token)|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log|webhook)|secret|source[_-]?archive|stripe[_-]?(customer|invoice|payment|secret|webhook)|wallet|webhook)/i
const unsafeValuePattern =
  /(bearer\s+|checkout_id=|cus_[A-Za-z0-9]+|evt_[A-Za-z0-9]+|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github[_-]?pat_[a-z0-9_]+|in_[A-Za-z0-9]+|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|method|preimage|proof)|pm_[A-Za-z0-9]+|preimage=[A-Za-z0-9_-]+|provider[_-]?token|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log|webhook)|secret|sk-[a-z0-9]|wallet[_-]?state|whsec_[A-Za-z0-9]+|\S+@\S+)/i

const scanForUnsafeSpendCapPreviewMaterial = (
  value: unknown,
  path: ReadonlyArray<string> = [],
): string | undefined => {
  if (typeof value === 'string') {
    const key = path.at(-1)

    if (publicLiteralValues.has(value)) {
      return undefined
    }

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
        scanForUnsafeSpendCapPreviewMaterial(item, [...path, String(index)]),
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
        : scanForUnsafeSpendCapPreviewMaterial(item, [...path, key]),
    )
    .find((unsafePath): unsafePath is string => unsafePath !== undefined)
}

const assertSafeSpendCapPreviewValue = (
  label: string,
  value: unknown,
): void => {
  const unsafePath = scanForUnsafeSpendCapPreviewMaterial(value)

  if (unsafePath !== undefined) {
    throw new OpenAgentsSpendCapPreviewUnsafe({
      reason: `${label} contains private or payment-secret material at ${unsafePath}.`,
    })
  }
}

const safeRef = (value: string): string | undefined =>
  value.trim() !== '' &&
  stableRefPattern.test(value) &&
  scanForUnsafeSpendCapPreviewMaterial(value) === undefined
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

const zeroSideEffects: OpenAgentsSpendCapPreviewSideEffectSummary = {
  callsMdk: false,
  createsEntitlement: false,
  createsPaymentArtifact: false,
  debitsCredits: false,
  mutatesPayout: false,
  redeemsCredentials: false,
}

const priceValid = (price: OpenAgentsPaidEndpointPrice): boolean =>
  Number.isInteger(price.amountMinorUnits) && price.amountMinorUnits > 0

const amountMatches = (
  left: OpenAgentsPaidEndpointPrice,
  right: OpenAgentsPaidEndpointPrice,
): boolean =>
  left.asset === right.asset && left.denomination === right.denomination

const expectedDenominationForAsset = (
  asset: OpenAgentsPaidEndpointPrice['asset'],
): OpenAgentsPaidEndpointDenomination =>
  asset === 'bitcoin'
    ? 'bitcoin_millisatoshi'
    : asset === 'credits'
      ? 'credit'
      : 'usd_cent'

const denominationMatchesAsset = (
  price: OpenAgentsPaidEndpointPrice,
): boolean => price.denomination === expectedDenominationForAsset(price.asset)

const routeIsPublicPreviewable = (
  route: OpenAgentsSpendCapPreviewRoute,
): boolean =>
  !route.privateRoute &&
  (route.path === null ||
    (
      route.path.startsWith('/') &&
      !route.path.includes('?') &&
      !route.path.includes('#') &&
      !route.path.includes('://') &&
      !route.path.includes('//')
    ))

const railSupported = (input: OpenAgentsSpendCapPreviewInput): boolean =>
  input.supportedRails.includes(input.requestedRail)

const windowTotal = (input: OpenAgentsSpendCapPreviewInput): number =>
  input.windowSpent.amountMinorUnits + input.price.amountMinorUnits

const statusForInput = (
  input: OpenAgentsSpendCapPreviewInput,
): OpenAgentsSpendCapPreviewStatus =>
  !input.agentAuthenticated
    ? 'unauthenticated_agent'
    : input.product === null || input.actionRef === null
      ? 'catalog_missing'
      : !priceValid(input.price) ||
        !priceValid(input.maxPerCall) ||
        !priceValid(input.maxPerWindow) ||
        !Number.isInteger(input.windowSpent.amountMinorUnits) ||
        input.windowSpent.amountMinorUnits < 0
        ? 'malformed_amount'
        : !denominationMatchesAsset(input.price) ||
          !amountMatches(input.price, input.maxPerCall) ||
          !amountMatches(input.price, input.maxPerWindow) ||
          !amountMatches(input.price, input.windowSpent)
          ? 'wrong_currency'
          : !railSupported(input)
            ? 'unsupported_rail'
            : input.product.status !== 'active'
              ? 'stale_catalog_entry'
              : !routeIsPublicPreviewable(input.route)
                ? 'private_route'
                : input.route.ownerGrantOnly
                  ? 'owner_grant_required'
                  : input.paymentDecision.status === 'hard_blocked'
                    ? 'blocked'
                    : input.price.amountMinorUnits >
                        input.maxPerCall.amountMinorUnits ||
                      windowTotal(input) > input.maxPerWindow.amountMinorUnits
                      ? 'over_cap'
                      : input.price.amountMinorUnits ===
                          input.maxPerCall.amountMinorUnits ||
                        windowTotal(input) ===
                          input.maxPerWindow.amountMinorUnits
                        ? 'exact_cap'
                        : 'under_cap'

const statusCodeForStatus = (
  status: OpenAgentsSpendCapPreviewStatus,
): number =>
  status === 'under_cap' || status === 'exact_cap'
    ? 200
    : status === 'over_cap'
      ? 402
      : status === 'malformed_amount' ||
        status === 'wrong_currency' ||
        status === 'unsupported_rail' ||
        status === 'catalog_missing'
        ? 400
        : status === 'stale_catalog_entry'
          ? 409
          : status === 'private_route' ||
            status === 'owner_grant_required' ||
            status === 'unauthenticated_agent' ||
            status === 'blocked'
            ? 403
            : 400

const nextActionsFromPaymentDecision = (
  actions: ReadonlyArray<OpenAgentsUnifiedPaymentNextAction>,
): ReadonlyArray<OpenAgentsSpendCapPreviewNextAction> =>
  actions
    .map(action =>
      action === 'add_credits'
        ? 'add_credits'
        : action === 'pay_l402_mdk'
          ? 'pay_l402_mdk'
          : action === 'request_manual_review'
            ? 'request_manual_review'
            : action === 'spend_internal_credits'
              ? 'spend_internal_credits'
              : action === 'use_entitlement'
                ? 'use_entitlement'
                : action === 'use_free_beta'
                  ? 'use_free_beta'
                  : 'stop',
    )
    .filter((action, index, actions) => actions.indexOf(action) === index)

const nextActionsForInput = (
  input: OpenAgentsSpendCapPreviewInput,
  status: OpenAgentsSpendCapPreviewStatus,
): ReadonlyArray<OpenAgentsSpendCapPreviewNextAction> =>
  status === 'under_cap' || status === 'exact_cap'
    ? nextActionsFromPaymentDecision(input.paymentDecision.nextActions)
    : status === 'over_cap'
      ? ['lower_spend_or_raise_cap']
      : status === 'unauthenticated_agent'
        ? ['provide_agent_token']
        : status === 'owner_grant_required'
          ? ['ask_owner_for_grant']
          : status === 'catalog_missing' || status === 'stale_catalog_entry'
            ? ['fix_catalog']
            : status === 'wrong_currency' || status === 'malformed_amount'
              ? ['fix_currency']
              : status === 'blocked'
                ? ['request_manual_review']
                : ['stop']

const reasonRefsForStatus = (
  status: OpenAgentsSpendCapPreviewStatus,
): ReadonlyArray<string> =>
  safeRefs([`reason.spend_cap_preview.${status}`])

const idempotencyGuidanceRefs = (
  input: OpenAgentsSpendCapPreviewInput,
): ReadonlyArray<string> =>
  safeRefs([
    input.idempotencyKeyRequired
      ? 'idempotency.spend_cap_preview.required'
      : 'idempotency.spend_cap_preview.optional',
    ...(input.idempotencyKeyHintRef === null
      ? []
      : [input.idempotencyKeyHintRef]),
  ])

const previewRefForInput = (input: OpenAgentsSpendCapPreviewInput): string =>
  safeRef([
    'preview',
    'spend_cap',
    input.product?.productId ?? 'missing_product',
    input.actionRef ?? 'missing_action',
    input.paymentDecision.decisionRef,
  ].join('.')) ?? 'preview.spend_cap.redacted'

const productRefForInput = (
  input: OpenAgentsSpendCapPreviewInput,
): string | null =>
  nullableSafeRef(input.product?.productId) ??
  nullableSafeRef(input.paymentDecision.productRef)

const entitlementScopeRefsForInput = (
  input: OpenAgentsSpendCapPreviewInput,
): ReadonlyArray<string> =>
  safeRefs(input.product?.entitlement.scopeRefs ?? [])

const projectionForInput = (
  input: OpenAgentsSpendCapPreviewInput,
): OpenAgentsSpendCapPreviewProjection => {
  const status = statusForInput(input)
  const windowRemainingMinorUnits = Math.max(
    0,
    input.maxPerWindow.amountMinorUnits - windowTotal(input),
  )
  const nextActions = nextActionsForInput(input, status)

  return {
    actionRef: nullableSafeRef(input.actionRef),
    actorRef:
      input.audience === 'agent' || input.audience === 'operator'
        ? nullableSafeRef(input.actorRef)
        : null,
    audience: input.audience,
    availableCreditAllowanceMinorUnits:
      Math.max(0, Math.trunc(input.availableCreditAllowanceMinorUnits)),
    dryRun: true,
    entitlementScopeRefs: entitlementScopeRefsForInput(input),
    freeAllowanceUses: input.freeAllowanceUses,
    idempotencyGuidanceRefs: idempotencyGuidanceRefs(input),
    l402MdkRecoveryAvailable: input.l402MdkRecoveryAvailable,
    maxPerCall: input.maxPerCall,
    maxPerWindow: input.maxPerWindow,
    nextActions: [...new Set(nextActions)],
    paymentDecisionStatus: input.paymentDecision.status,
    previewRef: previewRefForInput(input),
    price: input.price,
    productRef: productRefForInput(input),
    reasonRefs: reasonRefsForStatus(status),
    requestedRail: input.requestedRail,
    retryBehaviorRefs: safeRefs(input.retryBehaviorRefs),
    routeRef: safeRef(input.route.routeRef) ?? 'route.spend_cap_preview.redacted',
    safeBody: {
      action: 'spend_cap_preview',
      dryRun: true,
      nextActions,
      previewRef: previewRefForInput(input),
      productRef: productRefForInput(input),
      status,
    },
    settlementMode: input.settlementMode,
    sideEffectSummary: zeroSideEffects,
    status,
    statusCode: statusCodeForStatus(status),
    surface: input.surface,
    windowRemainingMinorUnits,
    windowSpent: input.windowSpent,
  }
}

const projectionIsSafe = (
  projection: OpenAgentsSpendCapPreviewProjection,
): boolean => scanForUnsafeSpendCapPreviewMaterial(projection) === undefined

export const openAgentsSpendCapPreviewHasPrivateMaterial = (
  value: unknown,
): boolean => scanForUnsafeSpendCapPreviewMaterial(value) !== undefined

export const previewOpenAgentsSpendCap = (
  input: OpenAgentsSpendCapPreviewInput,
): OpenAgentsSpendCapPreviewProjection => {
  assertSafeSpendCapPreviewValue('OpenAgents spend-cap preview input', input)

  const projection = projectionForInput(input)

  if (!projectionIsSafe(projection)) {
    throw new OpenAgentsSpendCapPreviewUnsafe({
      reason: 'OpenAgents spend-cap preview projection is not public-safe.',
    })
  }

  return projection
}
