import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import type { BuyerPaymentChallengeRecord } from './buyer-payment-ledger'
import {
  OpenAgentsL402CredentialProjection,
  OpenAgentsL402CredentialUnsafe,
  l402PayloadFromBuyerPaymentChallenge,
  makeOpenAgentsL402HmacSigningBoundary,
  mintOpenAgentsL402Credential,
  openAgentsL402CredentialProjectionHasPrivateMaterial,
  projectOpenAgentsL402Credential,
  verifyOpenAgentsL402Credential,
} from './l402-credential-service'

const challenge: BuyerPaymentChallengeRecord = {
  actorRef: 'agent:user_123',
  archivedAt: null,
  challengeRef: 'challenge.l402.agent_api.proposals.1',
  createdAt: '2026-06-06T08:00:00.000Z',
  expiresAt: '2026-06-06T08:10:00.000Z',
  id: 'buyer_payment_challenge_l402_1',
  idempotencyKeyHash: 'hash.challenge.l402.1',
  metadataRefs: ['metadata.l402.challenge'],
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

const expected = {
  amount: challenge.price,
  challengeRef: challenge.challengeRef,
  endpointRef: 'endpoint.agent_api.proposals',
  entitlementScopeRefs: ['entitlement.agent_api.proposals.day'],
  method: challenge.method,
  nowIso: '2026-06-06T08:05:00.000Z',
  path: challenge.path,
  paymentProofRef: 'payment_proof.redacted.mdk.1',
  productId: challenge.productId,
  requestBodyDigest: challenge.requestBodyDigest,
  requirePaymentProof: true,
} as const

const makeEnvelope = async () => {
  const signer = await makeOpenAgentsL402HmacSigningBoundary({
    secretKeyMaterial: 'unit-test-l402-secret',
    signerRef: 'signer.l402.unit_test',
  })
  const payload = l402PayloadFromBuyerPaymentChallenge({
    challenge,
    credentialRef: 'credential.l402.agent_api.proposals.1',
    endpointRef: expected.endpointRef,
    entitlementScopeRefs: expected.entitlementScopeRefs,
    issuedAt: '2026-06-06T08:00:00.000Z',
    paymentHashRef: 'payment_hash.redacted.mdk.1',
    replayNonceRef: 'replay_nonce.l402.agent_api.proposals.1',
  })
  const envelope = await mintOpenAgentsL402Credential(payload, signer)

  return { envelope, payload, signer }
}

describe('OpenAgents L402 credential service', () => {
  test('mints and verifies a credential bound to method, path, product, amount, expiry, and payment hash ref', async () => {
    const { envelope, payload, signer } = await makeEnvelope()
    const result = await verifyOpenAgentsL402Credential(
      envelope.credential,
      signer,
      expected,
    )

    expect(envelope.credential).toMatch(/^oa-l402-v1\./u)
    expect(envelope.payload).toEqual(payload)
    expect(result).toMatchObject({
      credentialRef: payload.credentialRef,
      payload,
      reasonRef: 'reason.l402_credential.valid',
      status: 'valid',
    })
  })

  test.each([
    [
      'expired',
      { nowIso: '2026-06-06T08:10:00.000Z' },
      'reason.l402_credential.expired',
    ],
    [
      'amount_mismatch',
      {
        amount: {
          amountMinorUnits: 501,
          asset: 'usd',
          denomination: 'usd_cent',
        },
      },
      'reason.l402_credential.amount_mismatch',
    ],
    [
      'resource_mismatch',
      { path: '/api/agents/other' },
      'reason.l402_credential.resource_mismatch',
    ],
    [
      'consumed_or_replayed',
      { consumedCredentialRefs: ['credential.l402.agent_api.proposals.1'] },
      'reason.l402_credential.consumed_or_replayed',
    ],
    [
      'proof_missing',
      { paymentProofRef: null },
      'reason.l402_credential.proof_missing',
    ],
  ] as const)('returns %s verification status', async (
    status,
    overrides,
    reasonRef,
  ) => {
    const { envelope, signer } = await makeEnvelope()
    const result = await verifyOpenAgentsL402Credential(
      envelope.credential,
      signer,
      {
        ...expected,
        ...overrides,
      },
    )

    expect(result.status).toBe(status)
    expect(result.reasonRef).toBe(reasonRef)
  })

  test('rejects malformed and signature-invalid credentials', async () => {
    const { envelope, signer } = await makeEnvelope()
    const malformed = await verifyOpenAgentsL402Credential(
      'not-a-real-credential',
      signer,
      expected,
    )
    const signatureInvalid = await verifyOpenAgentsL402Credential(
      `${envelope.credential.slice(0, -4)}xxxx`,
      signer,
      expected,
    )

    expect(malformed).toMatchObject({
      payload: null,
      reasonRef: 'reason.l402_credential.malformed',
      status: 'malformed',
    })
    expect(signatureInvalid.status).toBe('signature_invalid')
  })

  test('rejects raw payment and wallet material before minting or verifying', async () => {
    const { payload, signer } = await makeEnvelope()

    expect(() =>
      l402PayloadFromBuyerPaymentChallenge({
        challenge,
        credentialRef: 'credential.l402.agent_api.proposals.1',
        endpointRef: expected.endpointRef,
        entitlementScopeRefs: expected.entitlementScopeRefs,
        issuedAt: '2026-06-06T08:00:00.000Z',
        paymentHashRef: 'lnbc2500n1rawinvoice',
        replayNonceRef: 'replay_nonce.l402.agent_api.proposals.1',
      }),
    ).toThrow(OpenAgentsL402CredentialUnsafe)

    await expect(
      mintOpenAgentsL402Credential(
        {
          ...payload,
          paymentHashRef: 'payment_preimage=secret',
        },
        signer,
      ),
    ).rejects.toBeInstanceOf(OpenAgentsL402CredentialUnsafe)
  })

  test('projects credential metadata without exposing raw credential strings or secret material', async () => {
    const { envelope } = await makeEnvelope()
    const publicProjection = projectOpenAgentsL402Credential(envelope, 'public')
    const agentProjection = projectOpenAgentsL402Credential(envelope, 'agent')
    const operatorProjection = projectOpenAgentsL402Credential(
      envelope,
      'operator',
    )

    expect(S.decodeUnknownSync(OpenAgentsL402CredentialProjection)(
      publicProjection,
    )).toEqual(publicProjection)
    expect(publicProjection.paymentHashRef).toBe(null)
    expect(publicProjection.replayNonceRef).toBe(null)
    expect(publicProjection.signerRef).toBe(null)
    expect(agentProjection.paymentHashRef).toBe(
      envelope.payload.paymentHashRef,
    )
    expect(agentProjection.replayNonceRef).toBe(null)
    expect(operatorProjection.replayNonceRef).toBe(
      envelope.payload.replayNonceRef,
    )
    expect(operatorProjection.signerRef).toBe(envelope.signerRef)

    for (const projection of [
      publicProjection,
      agentProjection,
      operatorProjection,
    ]) {
      expect(openAgentsL402CredentialProjectionHasPrivateMaterial(projection))
        .toBe(false)
      expect(JSON.stringify(projection)).not.toContain(envelope.credential)
    }
  })
})
