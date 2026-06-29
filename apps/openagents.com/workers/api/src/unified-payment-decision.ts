import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  BuyerPaymentCreditDebitRecord,
  BuyerPaymentLedgerProjection,
  BuyerPaymentReceiptRecord,
  BuyerPaymentRedemptionRecord,
  BuyerPaymentSpendLimitRecord,
  projectBuyerPaymentLedgerRecord,
} from './buyer-payment-ledger'
import {
  OpenAgentsBuyerPaymentEntitlementPolicyProjection,
} from './buyer-payment-entitlement-policy'
import {
  classifyOpenAgentsPaymentLimitPolicy,
  OpenAgentsPaymentLimitClass,
  OpenAgentsPaymentLimitPolicyProjection,
  OpenAgentsPaymentPolicyAudience,
  OpenAgentsPaymentPolicySurface,
  projectOpenAgentsPaymentLimitPolicyDecision,
} from './payment-limit-policy'
import { openAgentsRunnerGatewayPayloadHasPrivateMaterial } from './runner-gateway'

export const OpenAgentsUnifiedPaymentDecisionStatus = S.Literals([
  'allow',
  'exhausted',
  'hard_blocked',
  'manual_review',
  'provider_unavailable',
  'recoverable_by_credits',
  'recoverable_by_either',
  'recoverable_by_l402_mdk',
])
export type OpenAgentsUnifiedPaymentDecisionStatus =
  typeof OpenAgentsUnifiedPaymentDecisionStatus.Type

export const OpenAgentsUnifiedPaymentSource = S.Literals([
  'credit_balance',
  'free_beta',
  'l402_mdk',
  'none',
  'product_entitlement',
])
export type OpenAgentsUnifiedPaymentSource =
  typeof OpenAgentsUnifiedPaymentSource.Type

export const OpenAgentsUnifiedPaymentNextAction = S.Literals([
  'add_credits',
  'pay_l402_mdk',
  'request_manual_review',
  'retry_later',
  'spend_internal_credits',
  'stop',
  'use_entitlement',
  'use_free_beta',
])
export type OpenAgentsUnifiedPaymentNextAction =
  typeof OpenAgentsUnifiedPaymentNextAction.Type

export const OpenAgentsStripeTopUpState = S.Literals([
  'available',
  'completed',
  'failed',
  'not_configured',
  'pending',
])
export type OpenAgentsStripeTopUpState =
  typeof OpenAgentsStripeTopUpState.Type

export const OpenAgentsL402MdkProviderState = S.Literals([
  'available',
  'missing_config',
  'provider_unavailable',
])
export type OpenAgentsL402MdkProviderState =
  typeof OpenAgentsL402MdkProviderState.Type

export const OpenAgentsUnifiedPaymentCreditState = S.Struct({
  balanceMinorUnits: S.Number,
  creditDebit: S.NullOr(BuyerPaymentCreditDebitRecord),
  creditLedgerRefs: S.Array(S.String),
  currency: S.Literal('USD'),
  requiredMinorUnits: S.Number,
  stripeTopUpRefs: S.Array(S.String),
  stripeTopUpState: OpenAgentsStripeTopUpState,
})
export type OpenAgentsUnifiedPaymentCreditState =
  typeof OpenAgentsUnifiedPaymentCreditState.Type

export const OpenAgentsUnifiedPaymentFreeBetaState = S.Struct({
  allowanceRef: S.NullOr(S.String),
  available: S.Boolean,
  remainingUses: S.NullOr(S.Number),
})
export type OpenAgentsUnifiedPaymentFreeBetaState =
  typeof OpenAgentsUnifiedPaymentFreeBetaState.Type

export const OpenAgentsUnifiedPaymentL402MdkState = S.Struct({
  entitlementDecision:
    S.NullOr(OpenAgentsBuyerPaymentEntitlementPolicyProjection),
  providerState: OpenAgentsL402MdkProviderState,
  receipt: S.NullOr(BuyerPaymentReceiptRecord),
  redemption: S.NullOr(BuyerPaymentRedemptionRecord),
})
export type OpenAgentsUnifiedPaymentL402MdkState =
  typeof OpenAgentsUnifiedPaymentL402MdkState.Type

export const OpenAgentsUnifiedPaymentSourceRefs = S.Struct({
  creditLedgerRefs: S.Array(S.String),
  entitlementRefs: S.Array(S.String),
  l402RedemptionRef: S.NullOr(S.String),
  mdkCheckoutReceiptRef: S.NullOr(S.String),
  policyRefs: S.Array(S.String),
  spendCapRefs: S.Array(S.String),
  stripeTopUpRefs: S.Array(S.String),
})
export type OpenAgentsUnifiedPaymentSourceRefs =
  typeof OpenAgentsUnifiedPaymentSourceRefs.Type

export const OpenAgentsUnifiedPaymentDecisionInput = S.Struct({
  actorRef: S.String,
  audience: OpenAgentsPaymentPolicyAudience,
  creditState: OpenAgentsUnifiedPaymentCreditState,
  entitlementDecision:
    S.NullOr(OpenAgentsBuyerPaymentEntitlementPolicyProjection),
  freeBeta: OpenAgentsUnifiedPaymentFreeBetaState,
  idempotencyKeyHash: S.String,
  l402Mdk: OpenAgentsUnifiedPaymentL402MdkState,
  limitClass: OpenAgentsPaymentLimitClass,
  nowIso: S.String,
  policyRefs: S.Array(S.String),
  productRef: S.String,
  publicSummaryRef: S.String,
  requiredEndpointRefs: S.Array(S.String),
  requiredScopeRefs: S.Array(S.String),
  sourceRefs: OpenAgentsUnifiedPaymentSourceRefs,
  spendLimit: S.NullOr(BuyerPaymentSpendLimitRecord),
  surface: OpenAgentsPaymentPolicySurface,
})
export type OpenAgentsUnifiedPaymentDecisionInput =
  typeof OpenAgentsUnifiedPaymentDecisionInput.Type

export const OpenAgentsUnifiedPaymentDecisionProjection = S.Struct({
  actorRef: S.NullOr(S.String),
  audience: OpenAgentsPaymentPolicyAudience,
  creditDebit: S.NullOr(BuyerPaymentLedgerProjection),
  decisionRef: S.String,
  entitlementDecision:
    S.NullOr(OpenAgentsBuyerPaymentEntitlementPolicyProjection),
  freeBetaAllowanceRef: S.NullOr(S.String),
  l402MdkReceipt: S.NullOr(BuyerPaymentLedgerProjection),
  l402MdkRedemption: S.NullOr(BuyerPaymentLedgerProjection),
  nextActions: S.Array(OpenAgentsUnifiedPaymentNextAction),
  paymentSource: OpenAgentsUnifiedPaymentSource,
  policyDecision: OpenAgentsPaymentLimitPolicyProjection,
  productRef: S.String,
  reasonRefs: S.Array(S.String),
  safeBody: S.Record(S.String, S.Unknown),
  sourceRefs: OpenAgentsUnifiedPaymentSourceRefs,
  spendLimit: S.NullOr(BuyerPaymentLedgerProjection),
  status: OpenAgentsUnifiedPaymentDecisionStatus,
  statusCode: S.Number,
  surface: OpenAgentsPaymentPolicySurface,
})
export type OpenAgentsUnifiedPaymentDecisionProjection =
  typeof OpenAgentsUnifiedPaymentDecisionProjection.Type

export class OpenAgentsUnifiedPaymentDecisionUnsafe extends S.TaggedErrorClass<OpenAgentsUnifiedPaymentDecisionUnsafe>()(
  'OpenAgentsUnifiedPaymentDecisionUnsafe',
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
  /(access[_-]?token|bearer|callback[_-]?token|checkout[_-]?id|cookie|customer[_-]?(email|name|id)|email[_-]?body|invoice|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|method|preimage|proof)|preimage|private[_-]?key|provider[_-]?(account|grant|payload|token)|raw[_-]?(credit|invoice|payment|payload|prompt|runner|run[_-]?log|webhook)|secret|source[_-]?archive|stripe[_-]?(customer|invoice|payment|secret|webhook)|wallet|webhook)/i
const unsafeValuePattern =
  /(bearer\s+|checkout_id=|cus_[A-Za-z0-9]+|evt_[A-Za-z0-9]+|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github[_-]?pat_[a-z0-9_]+|in_[A-Za-z0-9]+|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|method|preimage|proof)|pm_[A-Za-z0-9]+|preimage=[A-Za-z0-9_-]+|provider[_-]?token|raw[_-]?(credit|invoice|payment|payload|prompt|runner|run[_-]?log|webhook)|secret|sk-[a-z0-9]|wallet[_-]?state|whsec_[A-Za-z0-9]+|\S+@\S+)/i

const scanForUnsafeUnifiedPaymentMaterial = (
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
        scanForUnsafeUnifiedPaymentMaterial(item, [
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
        : scanForUnsafeUnifiedPaymentMaterial(item, [...path, key]),
    )
    .find((unsafePath): unsafePath is string => unsafePath !== undefined)
}

const assertSafeUnifiedPaymentValue = (
  label: string,
  value: unknown,
): void => {
  const unsafePath = scanForUnsafeUnifiedPaymentMaterial(value)

  if (unsafePath !== undefined) {
    throw new OpenAgentsUnifiedPaymentDecisionUnsafe({
      reason:
        `${label} contains private or payment-secret material at ${unsafePath}.`,
    })
  }
}

const safeRef = (value: string): string | undefined =>
  value.trim() !== '' &&
  stableRefPattern.test(value) &&
  scanForUnsafeUnifiedPaymentMaterial(value) === undefined
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

const entitlementDecisionAllows = (
  decision: OpenAgentsBuyerPaymentEntitlementPolicyProjection | null,
): boolean =>
  decision !== null &&
  decision.useEntitlement &&
  (
    decision.status === 'allow_existing' ||
    decision.status === 'consume_one_shot' ||
    decision.status === 'create_entitlement' ||
    decision.status === 'decrement_quota' ||
    decision.status === 'renew_entitlement'
  )

const creditBalanceCovers = (
  creditState: OpenAgentsUnifiedPaymentCreditState,
): boolean =>
  creditState.requiredMinorUnits > 0 &&
  creditState.balanceMinorUnits >= creditState.requiredMinorUnits

const creditRecoveryAvailable = (
  creditState: OpenAgentsUnifiedPaymentCreditState,
): boolean =>
  creditState.stripeTopUpState === 'available' ||
  creditState.stripeTopUpState === 'pending' ||
  creditState.stripeTopUpState === 'completed'

const l402MdkRecoveryAvailable = (
  l402Mdk: OpenAgentsUnifiedPaymentL402MdkState,
): boolean => l402Mdk.providerState === 'available'

const providerUnavailable = (
  input: OpenAgentsUnifiedPaymentDecisionInput,
): boolean =>
  input.limitClass === 'provider_capacity' ||
  input.l402Mdk.providerState === 'provider_unavailable'

const hardBlocked = (
  limitClass: typeof OpenAgentsPaymentLimitClass.Type,
): boolean =>
  limitClass === 'safety' ||
  limitClass === 'abuse' ||
  limitClass === 'private_authority'

const statusForInput = (
  input: OpenAgentsUnifiedPaymentDecisionInput,
): OpenAgentsUnifiedPaymentDecisionStatus => {
  const creditRecoverable = creditRecoveryAvailable(input.creditState)
  const l402Recoverable = l402MdkRecoveryAvailable(input.l402Mdk)

  return hardBlocked(input.limitClass)
    ? 'hard_blocked'
    : input.limitClass === 'manual_review'
      ? 'manual_review'
      : providerUnavailable(input) &&
        !creditRecoverable &&
        !entitlementDecisionAllows(input.entitlementDecision) &&
        !entitlementDecisionAllows(input.l402Mdk.entitlementDecision) &&
        !input.freeBeta.available
        ? 'provider_unavailable'
        : entitlementDecisionAllows(input.entitlementDecision) ||
          entitlementDecisionAllows(input.l402Mdk.entitlementDecision) ||
          creditBalanceCovers(input.creditState) ||
          input.freeBeta.available
          ? 'allow'
          : creditRecoverable && l402Recoverable
            ? 'recoverable_by_either'
            : creditRecoverable
              ? 'recoverable_by_credits'
              : l402Recoverable
                ? 'recoverable_by_l402_mdk'
                : input.l402Mdk.providerState === 'missing_config'
                  ? 'exhausted'
                  : 'exhausted'
}

const paymentSourceForInput = (
  input: OpenAgentsUnifiedPaymentDecisionInput,
  status: OpenAgentsUnifiedPaymentDecisionStatus,
): OpenAgentsUnifiedPaymentSource =>
  status !== 'allow'
    ? 'none'
    : entitlementDecisionAllows(input.entitlementDecision)
      ? 'product_entitlement'
      : entitlementDecisionAllows(input.l402Mdk.entitlementDecision)
        ? 'l402_mdk'
        : creditBalanceCovers(input.creditState)
          ? 'credit_balance'
          : input.freeBeta.available
            ? 'free_beta'
            : 'none'

const nextActionsForInput = (
  input: OpenAgentsUnifiedPaymentDecisionInput,
  status: OpenAgentsUnifiedPaymentDecisionStatus,
  source: OpenAgentsUnifiedPaymentSource,
): ReadonlyArray<OpenAgentsUnifiedPaymentNextAction> => {
  if (status === 'allow') {
    return source === 'product_entitlement' || source === 'l402_mdk'
      ? ['use_entitlement']
      : source === 'credit_balance'
        ? ['spend_internal_credits']
        : source === 'free_beta'
          ? ['use_free_beta']
          : ['stop']
  }

  if (status === 'recoverable_by_either') {
    return ['add_credits', 'pay_l402_mdk']
  }

  if (status === 'recoverable_by_credits') {
    return ['add_credits']
  }

  if (status === 'recoverable_by_l402_mdk') {
    return ['pay_l402_mdk']
  }

  if (status === 'manual_review') {
    return ['request_manual_review']
  }

  if (status === 'provider_unavailable') {
    return ['retry_later']
  }

  return ['stop']
}

const statusCodeForStatus = (
  status: OpenAgentsUnifiedPaymentDecisionStatus,
): number =>
  status === 'allow'
    ? 200
    : status === 'recoverable_by_credits' ||
      status === 'recoverable_by_either' ||
      status === 'recoverable_by_l402_mdk' ||
      status === 'exhausted'
      ? 402
      : status === 'provider_unavailable'
        ? 503
        : 403

const reasonRefsForInput = (
  input: OpenAgentsUnifiedPaymentDecisionInput,
  status: OpenAgentsUnifiedPaymentDecisionStatus,
  source: OpenAgentsUnifiedPaymentSource,
): ReadonlyArray<string> =>
  safeRefs([
    status === 'allow'
      ? `reason.unified_payment.allowed.${source}`
      : `reason.unified_payment.${status}`,
    ...input.policyRefs,
  ])

const decisionRefForInput = (
  input: OpenAgentsUnifiedPaymentDecisionInput,
): string =>
  safeRef([
    'decision',
    'unified_payment',
    input.productRef,
    input.idempotencyKeyHash,
  ].join('.')) ?? 'decision.unified_payment.redacted'

const sourceRefsForInput = (
  input: OpenAgentsUnifiedPaymentDecisionInput,
): OpenAgentsUnifiedPaymentSourceRefs => ({
  creditLedgerRefs: safeRefs([
    ...input.sourceRefs.creditLedgerRefs,
    ...input.creditState.creditLedgerRefs,
    ...(input.creditState.creditDebit === null
      ? []
      : [input.creditState.creditDebit.debitRef]),
  ]),
  entitlementRefs: safeRefs([
    ...input.sourceRefs.entitlementRefs,
    ...(input.entitlementDecision?.entitlementRef === null ||
    input.entitlementDecision?.entitlementRef === undefined
      ? []
      : [input.entitlementDecision.entitlementRef]),
    ...(input.l402Mdk.entitlementDecision?.entitlementRef === null ||
    input.l402Mdk.entitlementDecision?.entitlementRef === undefined
      ? []
      : [input.l402Mdk.entitlementDecision.entitlementRef]),
  ]),
  l402RedemptionRef: nullableSafeRef(
    input.sourceRefs.l402RedemptionRef ??
      input.l402Mdk.redemption?.redemptionRef,
  ),
  mdkCheckoutReceiptRef: nullableSafeRef(
    input.sourceRefs.mdkCheckoutReceiptRef ?? input.l402Mdk.receipt?.receiptRef,
  ),
  policyRefs: safeRefs([...input.sourceRefs.policyRefs, ...input.policyRefs]),
  spendCapRefs: safeRefs([
    ...input.sourceRefs.spendCapRefs,
    ...(input.spendLimit === null ? [] : [input.spendLimit.spendLimitRef]),
  ]),
  stripeTopUpRefs: safeRefs([
    ...input.sourceRefs.stripeTopUpRefs,
    ...input.creditState.stripeTopUpRefs,
  ]),
})

const paymentLimitPolicyForInput = (
  input: OpenAgentsUnifiedPaymentDecisionInput,
): OpenAgentsPaymentLimitPolicyProjection => {
  const decision = classifyOpenAgentsPaymentLimitPolicy({
    creditsAvailable:
      creditBalanceCovers(input.creditState) ||
      creditRecoveryAvailable(input.creditState),
    entitlementScopeRefs: input.requiredScopeRefs,
    freeBetaAvailable: input.freeBeta.available,
    l402MdkAvailable: l402MdkRecoveryAvailable(input.l402Mdk),
    limitClass: input.limitClass,
    publicSummaryRef: input.publicSummaryRef,
    reasonRefs: input.policyRefs,
    requiredEndpointRefs: input.requiredEndpointRefs,
    requiredProductRefs: [input.productRef],
    spendCapCaveatRefs: input.sourceRefs.spendCapRefs,
    statusRefs: [
      `status.unified_payment.l402_mdk.${input.l402Mdk.providerState}`,
      `status.unified_payment.stripe_top_up.${input.creditState.stripeTopUpState}`,
    ],
    surface: input.surface,
  })

  return projectOpenAgentsPaymentLimitPolicyDecision(decision, input.audience)
}

const projectionForInput = (
  input: OpenAgentsUnifiedPaymentDecisionInput,
): OpenAgentsUnifiedPaymentDecisionProjection => {
  const status = statusForInput(input)
  const source = paymentSourceForInput(input, status)
  const nextActions = nextActionsForInput(input, status, source)
  const sourceRefs = sourceRefsForInput(input)
  const policyDecision = paymentLimitPolicyForInput(input)
  const reasonRefs = reasonRefsForInput(input, status, source)
  const actorRef =
    input.audience === 'operator' || input.audience === 'agent'
      ? nullableSafeRef(input.actorRef)
      : null

  return {
    actorRef,
    audience: input.audience,
    creditDebit: input.creditState.creditDebit === null
      ? null
      : projectBuyerPaymentLedgerRecord(
        'credit_debit',
        input.creditState.creditDebit,
        input.audience,
      ),
    decisionRef: decisionRefForInput(input),
    entitlementDecision:
      input.entitlementDecision ?? input.l402Mdk.entitlementDecision,
    freeBetaAllowanceRef: nullableSafeRef(input.freeBeta.allowanceRef),
    l402MdkReceipt: input.l402Mdk.receipt === null
      ? null
      : projectBuyerPaymentLedgerRecord(
        'receipt',
        input.l402Mdk.receipt,
        input.audience,
      ),
    l402MdkRedemption: input.l402Mdk.redemption === null
      ? null
      : projectBuyerPaymentLedgerRecord(
        'redemption',
        input.l402Mdk.redemption,
        input.audience,
      ),
    nextActions: [...new Set(nextActions)],
    paymentSource: source,
    policyDecision,
    productRef: nullableSafeRef(input.productRef) ??
      'product.unified_payment.redacted',
    reasonRefs,
    safeBody: {
      action: 'unified_payment_decision',
      decisionRef: decisionRefForInput(input),
      nextActions,
      paymentSource: source,
      productRef: input.productRef,
      status,
    },
    sourceRefs,
    spendLimit: input.spendLimit === null
      ? null
      : projectBuyerPaymentLedgerRecord(
        'spend_limit',
        input.spendLimit,
        input.audience,
      ),
    status,
    statusCode: statusCodeForStatus(status),
    surface: input.surface,
  }
}

const projectionIsSafe = (
  projection: OpenAgentsUnifiedPaymentDecisionProjection,
): boolean => scanForUnsafeUnifiedPaymentMaterial(projection) === undefined

export const openAgentsUnifiedPaymentDecisionHasPrivateMaterial = (
  value: unknown,
): boolean => scanForUnsafeUnifiedPaymentMaterial(value) !== undefined

export const evaluateOpenAgentsUnifiedPaymentDecision = (
  input: OpenAgentsUnifiedPaymentDecisionInput,
): OpenAgentsUnifiedPaymentDecisionProjection => {
  assertSafeUnifiedPaymentValue('OpenAgents unified payment decision input', input)

  const projection = projectionForInput(input)

  if (!projectionIsSafe(projection)) {
    throw new OpenAgentsUnifiedPaymentDecisionUnsafe({
      reason: 'OpenAgents unified payment decision projection is not safe.',
    })
  }

  return projection
}
