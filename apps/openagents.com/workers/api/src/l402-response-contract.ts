import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  type BuyerPaymentChallengeRecord,
  BuyerPaymentLedgerAmount,
} from './buyer-payment-ledger'
import {
  type OpenAgentsPaidEndpointProductRecord,
} from './paid-endpoint-product-catalog'
import {
  OpenAgentsPaymentLimitPolicyDecision,
  OpenAgentsPaymentPolicyAudience,
  type OpenAgentsPaymentLimitPolicyDecision as OpenAgentsPaymentLimitPolicyDecisionType,
} from './payment-limit-policy'
import {
  OpenAgentsL402VerificationResult,
  type OpenAgentsL402VerificationResult as OpenAgentsL402VerificationResultType,
} from './l402-credential-service'
import { openAgentsRunnerGatewayPayloadHasPrivateMaterial } from './runner-gateway'

export const OpenAgentsL402HttpStatus = S.Literals([401, 402, 403])
export type OpenAgentsL402HttpStatus = typeof OpenAgentsL402HttpStatus.Type

export const OpenAgentsL402ResponseErrorKind = S.Literals([
  'abuse_denied',
  'amount_mismatch',
  'auth_missing',
  'consumed_or_replayed',
  'expired_credential',
  'invalid_proof',
  'malformed_credential',
  'manual_review_required',
  'payment_required',
  'private_authority_denied',
  'proof_missing',
  'provider_capacity_unavailable',
  'resource_mismatch',
  'safety_denied',
  'scope_missing',
  'signature_invalid',
])
export type OpenAgentsL402ResponseErrorKind =
  typeof OpenAgentsL402ResponseErrorKind.Type

export const OpenAgentsL402ChallengeContract = S.Struct({
  amount: BuyerPaymentLedgerAmount,
  challengeRef: S.String,
  docsRefs: S.Array(S.String),
  endpointRef: S.String,
  entitlementScopeRefs: S.Array(S.String),
  expiresAt: S.String,
  headerRefs: S.Array(S.String),
  productId: S.String,
  retryActionRefs: S.Array(S.String),
  spendCap: BuyerPaymentLedgerAmount,
})
export type OpenAgentsL402ChallengeContract =
  typeof OpenAgentsL402ChallengeContract.Type

export const OpenAgentsL402ResponseContract = S.Struct({
  audience: OpenAgentsPaymentPolicyAudience,
  challenge: S.NullOr(OpenAgentsL402ChallengeContract),
  credentialStatus: S.NullOr(OpenAgentsL402VerificationResult.fields.status),
  docsRefs: S.Array(S.String),
  errorKind: OpenAgentsL402ResponseErrorKind,
  headerRefs: S.Array(S.String),
  policyDecision: S.NullOr(OpenAgentsPaymentLimitPolicyDecision.fields.decisionStatus),
  productId: S.NullOr(S.String),
  publicSummaryRef: S.String,
  reasonRefs: S.Array(S.String),
  recoveryActionRefs: S.Array(S.String),
  statusCode: OpenAgentsL402HttpStatus,
  statusRefs: S.Array(S.String),
})
export type OpenAgentsL402ResponseContract =
  typeof OpenAgentsL402ResponseContract.Type

export class OpenAgentsL402ResponseContractUnsafe extends S.TaggedErrorClass<OpenAgentsL402ResponseContractUnsafe>()(
  'OpenAgentsL402ResponseContractUnsafe',
  {
    reason: S.String,
  },
) {}

const unsafeResponseKeyPattern =
  /(access[_-]?token|bearer|callback[_-]?token|cookie|customer[_-]?(email|name)|email[_-]?body|invoice|mdk|mnemonic|oauth|payment[_-]?preimage|preimage|private[_-]?key|provider[_-]?(account|grant|payload|token)|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log)|secret|source[_-]?archive|wallet|webhook)/i

const unsafeResponseValuePattern =
  /(\/Users\/|\/home\/|\.mdk-wallet|bearer\s+|checkout_id=|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github[_-]?pat_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|mdk_access_token|mnemonic|payment_preimage=|preimage|provider[_-]?token|raw[_-]?invoice|raw[_-]?payment|raw[_-]?payload|raw[_-]?prompt|raw[_-]?runner|raw[_-]?run[_-]?log|secret|sk-[a-z0-9]|\S+@\S+)/i

const stableRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/

const scanForUnsafeResponseMaterial = (
  value: unknown,
  path: ReadonlyArray<string> = [],
): string | undefined => {
  if (typeof value === 'string') {
    return containsProviderSecretMaterial(value) ||
      unsafeResponseValuePattern.test(value) ||
      openAgentsRunnerGatewayPayloadHasPrivateMaterial(value)
      ? path.join('.') || '<root>'
      : undefined
  }

  if (Array.isArray(value)) {
    for (const [index, item] of value.entries()) {
      const unsafePath = scanForUnsafeResponseMaterial(item, [
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
    if (unsafeResponseKeyPattern.test(key)) {
      return [...path, key].join('.')
    }

    const unsafePath = scanForUnsafeResponseMaterial(item, [...path, key])

    if (unsafePath !== undefined) {
      return unsafePath
    }
  }

  return undefined
}

const assertSafeResponseValue = (label: string, value: unknown): void => {
  const unsafePath = scanForUnsafeResponseMaterial(value)

  if (unsafePath !== undefined) {
    throw new OpenAgentsL402ResponseContractUnsafe({
      reason: `${label} contains private or payment-secret material at ${unsafePath}.`,
    })
  }
}

const safeRef = (value: string): string | undefined =>
  value.trim() !== '' &&
  stableRefPattern.test(value) &&
  scanForUnsafeResponseMaterial(value) === undefined
    ? value
    : undefined

const safeRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs)]
    .map(ref => safeRef(ref))
    .filter((ref): ref is string => ref !== undefined)

const defaultHeaderRefs = [
  'header.www_authenticate.l402',
  'header.x_openagents_l402',
]

const productDocs = (
  product: OpenAgentsPaidEndpointProductRecord,
): ReadonlyArray<string> => safeRefs(product.publicAgentDocRefs)

const challengeContract = (input: {
  challenge: BuyerPaymentChallengeRecord
  endpointRef: string
  product: OpenAgentsPaidEndpointProductRecord
  retryActionRefs?: ReadonlyArray<string> | undefined
}): OpenAgentsL402ChallengeContract => {
  assertSafeResponseValue('OpenAgents L402 challenge contract input', input)

  return {
    amount: input.challenge.price,
    challengeRef:
      safeRef(input.challenge.challengeRef) ?? 'challenge.payment.redacted',
    docsRefs: productDocs(input.product),
    endpointRef: safeRef(input.endpointRef) ?? 'endpoint.payment.redacted',
    entitlementScopeRefs: safeRefs(input.product.entitlement.scopeRefs),
    expiresAt: input.challenge.expiresAt,
    headerRefs: defaultHeaderRefs,
    productId: safeRef(input.product.productId) ?? 'product.payment.redacted',
    retryActionRefs: safeRefs(input.retryActionRefs ?? [
      'action.payment.retry_after_l402',
    ]),
    spendCap: input.challenge.spendCap,
  }
}

const policyErrorKind = (
  decision: OpenAgentsPaymentLimitPolicyDecisionType,
): OpenAgentsL402ResponseErrorKind =>
  decision.limitClass === 'safety'
    ? 'safety_denied'
    : decision.limitClass === 'abuse'
      ? 'abuse_denied'
      : decision.limitClass === 'private_authority'
        ? 'private_authority_denied'
        : decision.limitClass === 'provider_capacity'
          ? 'provider_capacity_unavailable'
          : 'manual_review_required'

const response = (
  input: Omit<OpenAgentsL402ResponseContract, 'reasonRefs' | 'statusRefs'> & {
    reasonRefs?: ReadonlyArray<string>
    statusRefs?: ReadonlyArray<string>
  },
): OpenAgentsL402ResponseContract => {
  assertSafeResponseValue('OpenAgents L402 response contract', input)

  return {
    ...input,
    docsRefs: safeRefs(input.docsRefs),
    headerRefs: safeRefs(input.headerRefs),
    reasonRefs: safeRefs(input.reasonRefs ?? []),
    recoveryActionRefs: safeRefs(input.recoveryActionRefs),
    statusRefs: safeRefs(input.statusRefs ?? []),
  }
}

export const paymentRequiredL402Response = (input: {
  audience: typeof OpenAgentsPaymentPolicyAudience.Type
  challenge: BuyerPaymentChallengeRecord
  endpointRef: string
  policyDecision: OpenAgentsPaymentLimitPolicyDecisionType
  product: OpenAgentsPaidEndpointProductRecord
  retryActionRefs?: ReadonlyArray<string> | undefined
}): OpenAgentsL402ResponseContract => {
  if (input.policyDecision.decisionStatus !== 'recoverable') {
    throw new OpenAgentsL402ResponseContractUnsafe({
      reason:
        'Payment-required L402 responses require a recoverable payment policy decision.',
    })
  }

  const challenge = challengeContract(input)

  return response({
    audience: input.audience,
    challenge,
    credentialStatus: null,
    docsRefs: challenge.docsRefs,
    errorKind: 'payment_required',
    headerRefs: challenge.headerRefs,
    policyDecision: input.policyDecision.decisionStatus,
    productId: challenge.productId,
    publicSummaryRef: 'summary.l402_response.payment_required',
    reasonRefs: input.policyDecision.reasonRefs,
    recoveryActionRefs: safeRefs(
      input.policyDecision.recoveryActions.map(
        action => `recovery_action.${action}`,
      ),
    ),
    statusCode: 402,
    statusRefs: input.policyDecision.statusRefs,
  })
}

export const forbiddenL402ResponseFromPolicy = (input: {
  audience: typeof OpenAgentsPaymentPolicyAudience.Type
  policyDecision: OpenAgentsPaymentLimitPolicyDecisionType
}): OpenAgentsL402ResponseContract => {
  if (input.policyDecision.decisionStatus === 'recoverable') {
    throw new OpenAgentsL402ResponseContractUnsafe({
      reason: 'Recoverable decisions should use a 402 response contract.',
    })
  }

  return response({
    audience: input.audience,
    challenge: null,
    credentialStatus: null,
    docsRefs: [],
    errorKind: policyErrorKind(input.policyDecision),
    headerRefs: [],
    policyDecision: input.policyDecision.decisionStatus,
    productId: null,
    publicSummaryRef: `summary.l402_response.${policyErrorKind(input.policyDecision)}`,
    reasonRefs: input.policyDecision.reasonRefs,
    recoveryActionRefs: [],
    statusCode: 403,
    statusRefs: input.policyDecision.statusRefs,
  })
}

export const authMissingL402Response = (
  audience: typeof OpenAgentsPaymentPolicyAudience.Type,
): OpenAgentsL402ResponseContract =>
  response({
    audience,
    challenge: null,
    credentialStatus: null,
    docsRefs: ['docs.auth.agent_bearer_or_browser_session'],
    errorKind: 'auth_missing',
    headerRefs: [],
    policyDecision: null,
    productId: null,
    publicSummaryRef: 'summary.l402_response.auth_missing',
    reasonRefs: ['reason.l402_response.auth_missing'],
    recoveryActionRefs: [],
    statusCode: 401,
    statusRefs: ['status.l402_response.auth_missing'],
  })

export const scopeMissingL402Response = (
  audience: typeof OpenAgentsPaymentPolicyAudience.Type,
): OpenAgentsL402ResponseContract =>
  response({
    audience,
    challenge: null,
    credentialStatus: null,
    docsRefs: ['docs.auth.scope_required'],
    errorKind: 'scope_missing',
    headerRefs: [],
    policyDecision: null,
    productId: null,
    publicSummaryRef: 'summary.l402_response.scope_missing',
    reasonRefs: ['reason.l402_response.scope_missing'],
    recoveryActionRefs: [],
    statusCode: 403,
    statusRefs: ['status.l402_response.scope_missing'],
  })

const verificationErrorKind = (
  status: OpenAgentsL402VerificationResultType['status'],
): OpenAgentsL402ResponseErrorKind => {
  switch (status) {
    case 'amount_mismatch':
      return 'amount_mismatch'
    case 'consumed_or_replayed':
      return 'consumed_or_replayed'
    case 'expired':
      return 'expired_credential'
    case 'proof_missing':
      return 'proof_missing'
    case 'resource_mismatch':
      return 'resource_mismatch'
    case 'signature_invalid':
      return 'signature_invalid'
    case 'valid':
      return 'payment_required'
    case 'malformed':
      return 'malformed_credential'
  }
}

export const credentialFailureL402Response = (input: {
  audience: typeof OpenAgentsPaymentPolicyAudience.Type
  result: OpenAgentsL402VerificationResultType
}): OpenAgentsL402ResponseContract => {
  if (input.result.status === 'valid') {
    throw new OpenAgentsL402ResponseContractUnsafe({
      reason: 'Valid L402 credentials should not produce a failure response.',
    })
  }

  return response({
    audience: input.audience,
    challenge: null,
    credentialStatus: input.result.status,
    docsRefs: ['docs.l402.credentials'],
    errorKind: verificationErrorKind(input.result.status),
    headerRefs: [],
    policyDecision: null,
    productId: input.result.payload?.productId ?? null,
    publicSummaryRef: `summary.l402_response.${verificationErrorKind(
      input.result.status,
    )}`,
    reasonRefs: [input.result.reasonRef],
    recoveryActionRefs: [],
    statusCode: 401,
    statusRefs: [`status.l402_response.${input.result.status}`],
  })
}

export const l402ResponseContractHasPrivateMaterial = (
  contract: OpenAgentsL402ResponseContract,
): boolean => scanForUnsafeResponseMaterial(contract) !== undefined
