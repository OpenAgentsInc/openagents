import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import type { BuyerPaymentChallengeRecord } from './buyer-payment-ledger'
import type { OpenAgentsPaidEndpointProductRecord } from './paid-endpoint-product-catalog'
import {
  classifyOpenAgentsPaymentLimitPolicy,
} from './payment-limit-policy'
import {
  type OpenAgentsL402VerificationResult,
} from './l402-credential-service'
import {
  OpenAgentsL402ResponseContract,
  OpenAgentsL402ResponseContractUnsafe,
  authMissingL402Response,
  credentialFailureL402Response,
  forbiddenL402ResponseFromPolicy,
  l402ResponseContractHasPrivateMaterial,
  paymentRequiredL402Response,
  scopeMissingL402Response,
} from './l402-response-contract'

const challenge: BuyerPaymentChallengeRecord = {
  actorRef: 'agent:user_123',
  archivedAt: null,
  challengeRef: 'challenge.l402.agent_api.proposals.1',
  createdAt: '2026-06-06T09:00:00.000Z',
  expiresAt: '2026-06-06T09:10:00.000Z',
  id: 'buyer_payment_challenge_response_1',
  idempotencyKeyHash: 'hash.challenge.response.1',
  metadataRefs: ['metadata.response.safe'],
  method: 'POST',
  ownerUserId: 'user_owner_123',
  path: '/api/agents/proposals',
  price: {
    amountMinorUnits: 500,
    asset: 'usd',
    denomination: 'usd_cent',
  },
  productId: 'product.agent_api.proposals.day',
  publicProjectionJson: '{}',
  requestBodyDigest: 'sha256:request_body_digest',
  spendCap: {
    amountMinorUnits: 500,
    asset: 'usd',
    denomination: 'usd_cent',
  },
  status: 'issued',
  surface: 'agent_api',
}

const product: OpenAgentsPaidEndpointProductRecord = {
  binding: {
    actionRef: null,
    kind: 'agent_api_endpoint',
    method: 'POST',
    pathTemplate: '/api/agents/proposals',
    resourceRef: 'resource.agent_api.proposals',
  },
  displayName: 'Agent proposal intake',
  entitlement: {
    durationSeconds: 86_400,
    kind: 'duration_quota',
    quotaUnits: 20,
    scopeRefs: ['entitlement.agent_api.proposals.day'],
  },
  internalEconomicsRefs: ['internal_economics.agent_api.proposals'],
  operatorNoteRefs: ['operator_note.payment_policy.reviewed'],
  price: challenge.price,
  productId: challenge.productId,
  projectionPolicy: 'agent_visible',
  providerBindingRefs: ['provider_binding.openagents.hosted_api'],
  publicAgentDocRefs: ['docs.api.agent_proposals'],
  publicSummaryRef: 'summary.product.agent_api.proposals.day',
  spendCapHintRefs: ['spend_cap.credits.max_500'],
  status: 'active',
  surface: 'agent_api',
}

describe('OpenAgents L402 response contract', () => {
  test('builds a 402 payment-required response only for recoverable economic policy', () => {
    const policyDecision = classifyOpenAgentsPaymentLimitPolicy({
      creditsAvailable: true,
      l402MdkAvailable: true,
      limitClass: 'economic_usage',
      requiredEndpointRefs: ['endpoint.agent_api.proposals'],
      requiredProductRefs: [product.productId],
      surface: 'agent_api',
    })
    const response = paymentRequiredL402Response({
      audience: 'agent',
      challenge,
      endpointRef: 'endpoint.agent_api.proposals',
      policyDecision,
      product,
    })

    expect(S.decodeUnknownSync(OpenAgentsL402ResponseContract)(response))
      .toEqual(response)
    expect(response).toMatchObject({
      credentialStatus: null,
      errorKind: 'payment_required',
      policyDecision: 'recoverable',
      productId: product.productId,
      statusCode: 402,
    })
    expect(response.challenge).toMatchObject({
      amount: challenge.price,
      challengeRef: challenge.challengeRef,
      docsRefs: ['docs.api.agent_proposals'],
      endpointRef: 'endpoint.agent_api.proposals',
      entitlementScopeRefs: ['entitlement.agent_api.proposals.day'],
      headerRefs: [
        'header.www_authenticate.l402',
        'header.x_openagents_l402',
      ],
      spendCap: challenge.spendCap,
    })
    expect(l402ResponseContractHasPrivateMaterial(response)).toBe(false)
  })

  test.each([
    ['safety', 'safety_denied'],
    ['abuse', 'abuse_denied'],
    ['private_authority', 'private_authority_denied'],
    ['provider_capacity', 'provider_capacity_unavailable'],
    ['manual_review', 'manual_review_required'],
  ] as const)('builds 403 without payment instructions for %s policy', (
    limitClass,
    errorKind,
  ) => {
    const policyDecision = classifyOpenAgentsPaymentLimitPolicy({
      creditsAvailable: true,
      l402MdkAvailable: true,
      limitClass,
      surface: 'agent_api',
    })
    const response = forbiddenL402ResponseFromPolicy({
      audience: 'agent',
      policyDecision,
    })

    expect(response.statusCode).toBe(403)
    expect(response.errorKind).toBe(errorKind)
    expect(response.challenge).toBe(null)
    expect(response.recoveryActionRefs).toEqual([])
    expect(response.headerRefs).toEqual([])
    expect(l402ResponseContractHasPrivateMaterial(response)).toBe(false)
  })

  test.each([
    ['malformed', 'malformed_credential'],
    ['signature_invalid', 'signature_invalid'],
    ['proof_missing', 'proof_missing'],
    ['expired', 'expired_credential'],
    ['consumed_or_replayed', 'consumed_or_replayed'],
    ['resource_mismatch', 'resource_mismatch'],
    ['amount_mismatch', 'amount_mismatch'],
  ] as const)('maps %s verification failure to 401 %s', (
    status,
    errorKind,
  ) => {
    const verification: OpenAgentsL402VerificationResult = {
      credentialRef: 'credential.l402.agent_api.proposals.1',
      payload: {
        amount: challenge.price,
        challengeRef: challenge.challengeRef,
        credentialRef: 'credential.l402.agent_api.proposals.1',
        endpointRef: 'endpoint.agent_api.proposals',
        entitlementScopeRefs: ['entitlement.agent_api.proposals.day'],
        expiresAt: challenge.expiresAt,
        idempotencyKeyHash: challenge.idempotencyKeyHash,
        issuedAt: challenge.createdAt,
        method: challenge.method,
        path: challenge.path,
        paymentHashRef: 'payment_hash.redacted.mdk.1',
        productId: challenge.productId,
        replayNonceRef: 'replay_nonce.l402.agent_api.proposals.1',
        requestBodyDigest: challenge.requestBodyDigest,
        version: 'oa-l402-v1',
      },
      reasonRef: `reason.l402_credential.${status}`,
      status,
    }
    const response = credentialFailureL402Response({
      audience: 'agent',
      result: verification,
    })

    expect(response.statusCode).toBe(401)
    expect(response.errorKind).toBe(errorKind)
    expect(response.credentialStatus).toBe(status)
    expect(response.challenge).toBe(null)
    expect(l402ResponseContractHasPrivateMaterial(response)).toBe(false)
  })

  test('builds auth-missing and scope-missing contracts without payment recovery', () => {
    const authMissing = authMissingL402Response('agent')
    const scopeMissing = scopeMissingL402Response('agent')

    expect(authMissing).toMatchObject({
      challenge: null,
      errorKind: 'auth_missing',
      recoveryActionRefs: [],
      statusCode: 401,
    })
    expect(scopeMissing).toMatchObject({
      challenge: null,
      errorKind: 'scope_missing',
      recoveryActionRefs: [],
      statusCode: 403,
    })
  })

  test('rejects unsafe refs before creating a response projection', () => {
    const policyDecision = classifyOpenAgentsPaymentLimitPolicy({
      creditsAvailable: true,
      l402MdkAvailable: true,
      limitClass: 'economic_usage',
      surface: 'agent_api',
    })

    expect(() =>
      paymentRequiredL402Response({
        audience: 'agent',
        challenge: {
          ...challenge,
          challengeRef: 'lnbc2500n1rawinvoice',
        },
        endpointRef: 'endpoint.agent_api.proposals',
        policyDecision,
        product,
      }),
    ).toThrow(OpenAgentsL402ResponseContractUnsafe)
  })
})
