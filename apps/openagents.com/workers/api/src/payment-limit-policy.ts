import { Schema as S } from 'effect'

import { openAgentsRunnerGatewayPayloadHasPrivateMaterial } from './runner-gateway'

export const OpenAgentsPaymentPolicySurface = S.Literals([
  'agent_api',
  'billing',
  'forum_paid_action',
  'runner',
  'site_checkout',
])
export type OpenAgentsPaymentPolicySurface =
  typeof OpenAgentsPaymentPolicySurface.Type

export const OpenAgentsPaymentLimitClass = S.Literals([
  'abuse',
  'credits',
  'economic_usage',
  'free_beta_allowance',
  'l402_mdk_recoverable',
  'manual_review',
  'private_authority',
  'provider_capacity',
  'safety',
])
export type OpenAgentsPaymentLimitClass =
  typeof OpenAgentsPaymentLimitClass.Type

export const OpenAgentsPaymentRecoveryAction = S.Literals([
  'credit_balance',
  'free_beta',
  'l402_mdk',
  'manual_review',
])
export type OpenAgentsPaymentRecoveryAction =
  typeof OpenAgentsPaymentRecoveryAction.Type

export const OpenAgentsPaymentPolicyDecisionStatus = S.Literals([
  'allow',
  'blocked',
  'manual_review',
  'recoverable',
])
export type OpenAgentsPaymentPolicyDecisionStatus =
  typeof OpenAgentsPaymentPolicyDecisionStatus.Type

export const OpenAgentsPaymentPolicyAudience = S.Literals([
  'agent',
  'customer',
  'operator',
  'public',
])
export type OpenAgentsPaymentPolicyAudience =
  typeof OpenAgentsPaymentPolicyAudience.Type

export const OpenAgentsPaymentLimitPolicyDecision = S.Struct({
  decisionStatus: OpenAgentsPaymentPolicyDecisionStatus,
  entitlementScopeRefs: S.Array(S.String),
  limitClass: OpenAgentsPaymentLimitClass,
  operatorCostRefs: S.Array(S.String),
  privateAccountRefs: S.Array(S.String),
  publicSummaryRef: S.String,
  reasonRefs: S.Array(S.String),
  recoveryActions: S.Array(OpenAgentsPaymentRecoveryAction),
  requiredEndpointRefs: S.Array(S.String),
  requiredProductRefs: S.Array(S.String),
  spendCapCaveatRefs: S.Array(S.String),
  statusRefs: S.Array(S.String),
  surface: OpenAgentsPaymentPolicySurface,
})
export type OpenAgentsPaymentLimitPolicyDecision =
  typeof OpenAgentsPaymentLimitPolicyDecision.Type

export const OpenAgentsPaymentLimitPolicyProjection = S.Struct({
  audience: OpenAgentsPaymentPolicyAudience,
  decisionStatus: OpenAgentsPaymentPolicyDecisionStatus,
  entitlementScopeRefs: S.Array(S.String),
  limitClass: OpenAgentsPaymentLimitClass,
  operatorCostRefs: S.Array(S.String),
  privateAccountRefs: S.Array(S.String),
  publicSummaryRef: S.String,
  reasonRefs: S.Array(S.String),
  recoveryActions: S.Array(OpenAgentsPaymentRecoveryAction),
  requiredEndpointRefs: S.Array(S.String),
  requiredProductRefs: S.Array(S.String),
  spendCapCaveatRefs: S.Array(S.String),
  statusRefs: S.Array(S.String),
  surface: OpenAgentsPaymentPolicySurface,
})
export type OpenAgentsPaymentLimitPolicyProjection =
  typeof OpenAgentsPaymentLimitPolicyProjection.Type

export type OpenAgentsPaymentLimitPolicyInput = Readonly<{
  creditsAvailable?: boolean | undefined
  entitlementScopeRefs?: ReadonlyArray<string> | undefined
  freeBetaAvailable?: boolean | undefined
  l402MdkAvailable?: boolean | undefined
  limitClass: OpenAgentsPaymentLimitClass
  operatorCostRefs?: ReadonlyArray<string> | undefined
  privateAccountRefs?: ReadonlyArray<string> | undefined
  publicSummaryRef?: string | undefined
  reasonRefs?: ReadonlyArray<string> | undefined
  requiredEndpointRefs?: ReadonlyArray<string> | undefined
  requiredProductRefs?: ReadonlyArray<string> | undefined
  spendCapCaveatRefs?: ReadonlyArray<string> | undefined
  statusRefs?: ReadonlyArray<string> | undefined
  surface: OpenAgentsPaymentPolicySurface
}>

const unsafePaymentPolicyRefPattern =
  /(bearer\s+|callback[_-]?token|cookie|customer[_-]?(email|name)|email[_-]?body|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github[_-]?pat_[a-z0-9_]+|internal[_-]?prompt|invoice(?![_-]?ref)|mnemonic|oauth|payment[_-]?preimage|preimage|private[_-]?(key|prompt)|provider[_-]?(account|grant|payload|token)|raw[_-]?(invoice|payment|prompt|runner|run[_-]?log)|secret|sk-[a-z0-9]|source[_-]?archive|wallet|\S+@\S+)/i

const safeRef = (ref: string): boolean =>
  ref.trim() !== '' &&
  !unsafePaymentPolicyRefPattern.test(ref) &&
  !openAgentsRunnerGatewayPayloadHasPrivateMaterial(ref)

const safeRefs = (refs: ReadonlyArray<string> | undefined): ReadonlyArray<string> =>
  [...new Set(refs ?? [])].filter(safeRef)

const safeRefOrFallback = (
  ref: string | undefined,
  fallback: string,
): string => safeRefs(ref === undefined ? [] : [ref])[0] ?? fallback

const hardBlockReason = (
  limitClass: OpenAgentsPaymentLimitClass,
): string =>
  limitClass === 'safety'
    ? 'reason.payment_policy.safety_not_payable'
    : limitClass === 'abuse'
      ? 'reason.payment_policy.abuse_not_payable'
      : 'reason.payment_policy.private_authority_not_payable'

const recoverableActions = (
  input: OpenAgentsPaymentLimitPolicyInput,
): ReadonlyArray<OpenAgentsPaymentRecoveryAction> => {
  const actions: OpenAgentsPaymentRecoveryAction[] = []

  if (input.creditsAvailable) {
    actions.push('credit_balance')
  }
  if (input.l402MdkAvailable) {
    actions.push('l402_mdk')
  }

  return actions
}

const decision = (
  input: OpenAgentsPaymentLimitPolicyInput,
  status: OpenAgentsPaymentPolicyDecisionStatus,
  actions: ReadonlyArray<OpenAgentsPaymentRecoveryAction>,
  defaultReasonRefs: ReadonlyArray<string>,
  defaultStatusRefs: ReadonlyArray<string>,
): OpenAgentsPaymentLimitPolicyDecision => ({
  decisionStatus: status,
  entitlementScopeRefs: safeRefs(input.entitlementScopeRefs),
  limitClass: input.limitClass,
  operatorCostRefs: safeRefs(input.operatorCostRefs),
  privateAccountRefs: safeRefs(input.privateAccountRefs),
  publicSummaryRef: safeRefOrFallback(
    input.publicSummaryRef,
    `summary.payment_policy.${input.limitClass}.${status}`,
  ),
  reasonRefs: safeRefs([
    ...defaultReasonRefs,
    ...(input.reasonRefs ?? []),
  ]),
  recoveryActions: [...new Set(actions)],
  requiredEndpointRefs: safeRefs(input.requiredEndpointRefs),
  requiredProductRefs: safeRefs(input.requiredProductRefs),
  spendCapCaveatRefs: safeRefs(input.spendCapCaveatRefs),
  statusRefs: safeRefs([
    ...defaultStatusRefs,
    ...(input.statusRefs ?? []),
  ]),
  surface: input.surface,
})

export const classifyOpenAgentsPaymentLimitPolicy = (
  input: OpenAgentsPaymentLimitPolicyInput,
): OpenAgentsPaymentLimitPolicyDecision => {
  if (
    input.limitClass === 'safety' ||
    input.limitClass === 'abuse' ||
    input.limitClass === 'private_authority'
  ) {
    return decision(
      input,
      'blocked',
      [],
      [hardBlockReason(input.limitClass)],
      ['status.payment_policy.not_payable'],
    )
  }

  if (
    input.limitClass === 'provider_capacity' ||
    input.limitClass === 'manual_review'
  ) {
    return decision(
      input,
      'manual_review',
      ['manual_review'],
      ['reason.payment_policy.manual_review_required'],
      ['status.payment_policy.manual_review_required'],
    )
  }

  if (input.limitClass === 'free_beta_allowance' && input.freeBetaAvailable) {
    return decision(
      input,
      'allow',
      ['free_beta'],
      ['reason.payment_policy.free_beta_allowance_available'],
      ['status.payment_policy.allowed_free_beta'],
    )
  }

  if (input.limitClass === 'credits' && input.creditsAvailable) {
    return decision(
      input,
      'allow',
      ['credit_balance'],
      ['reason.payment_policy.credit_balance_available'],
      ['status.payment_policy.allowed_credit_balance'],
    )
  }

  const actions = recoverableActions(input)

  return actions.length === 0
    ? decision(
        input,
        'manual_review',
        ['manual_review'],
        ['reason.payment_policy.no_payment_recovery_available'],
        ['status.payment_policy.manual_review_required'],
      )
    : decision(
        input,
        'recoverable',
        actions,
        ['reason.payment_policy.economic_limit_recoverable'],
        ['status.payment_policy.payment_recovery_available'],
      )
}

export const projectOpenAgentsPaymentLimitPolicyDecision = (
  source: OpenAgentsPaymentLimitPolicyDecision,
  audience: OpenAgentsPaymentPolicyAudience,
): OpenAgentsPaymentLimitPolicyProjection => {
  const operator = audience === 'operator'

  return {
    audience,
    decisionStatus: source.decisionStatus,
    entitlementScopeRefs: safeRefs(source.entitlementScopeRefs),
    limitClass: source.limitClass,
    operatorCostRefs: operator ? safeRefs(source.operatorCostRefs) : [],
    privateAccountRefs: [],
    publicSummaryRef: safeRefOrFallback(
      source.publicSummaryRef,
      'summary.payment_policy.redacted',
    ),
    reasonRefs: safeRefs(source.reasonRefs),
    recoveryActions: source.recoveryActions,
    requiredEndpointRefs: safeRefs(source.requiredEndpointRefs),
    requiredProductRefs: safeRefs(source.requiredProductRefs),
    spendCapCaveatRefs: safeRefs(source.spendCapCaveatRefs),
    statusRefs: safeRefs(source.statusRefs),
    surface: source.surface,
  }
}

export const openAgentsPaymentLimitPolicyProjectionHasPrivateMaterial = (
  projection: OpenAgentsPaymentLimitPolicyProjection,
): boolean =>
  unsafePaymentPolicyRefPattern.test(JSON.stringify(projection)) ||
  openAgentsRunnerGatewayPayloadHasPrivateMaterial(projection)
