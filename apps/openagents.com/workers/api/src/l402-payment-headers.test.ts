import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OpenAgentsPaymentHeaderParseResult,
  OpenAgentsPaymentHeaderProjection,
  OpenAgentsPaymentHeaderUnsafe,
  formatOpenAgentsL402Authorization,
  formatOpenAgentsL402WwwAuthenticate,
  formatOpenAgentsPaymentCredentialPair,
  formatOpenAgentsXOpenAgentsL402,
  openAgentsPaymentHeaderProjectionHasPrivateMaterial,
  parseOpenAgentsPaymentHeaders,
  projectOpenAgentsPaymentHeaderResult,
} from './l402-payment-headers'

const credential = 'oa-l402-v1.unit_test_signature'
const proofRef = 'payment_proof.redacted.mdk.1'
const credentialPair = `${credential}:${proofRef}`

const decodeParseResult = (headers: Headers) => {
  const result = parseOpenAgentsPaymentHeaders(headers)

  return S.decodeUnknownSync(OpenAgentsPaymentHeaderParseResult)(result)
}

describe('OpenAgents L402 payment headers', () => {
  test('formats public-safe L402 challenge and credential headers', () => {
    const wwwAuthenticate = formatOpenAgentsL402WwwAuthenticate({
      amount: {
        amountMinorUnits: 500,
        asset: 'usd',
        denomination: 'usd_cent',
      },
      challengeRef: 'challenge.l402.agent_api.proposals.1',
      docsRef: 'docs.api.agent_proposals',
      endpointRef: 'endpoint.agent_api.proposals',
      expiresAt: '2026-06-06T09:10:00.000Z',
      productId: 'product.agent_api.proposals.day',
    })

    expect(wwwAuthenticate).toBe(
      'L402 challenge_ref="challenge.l402.agent_api.proposals.1", ' +
        'product_id="product.agent_api.proposals.day", ' +
        'endpoint_ref="endpoint.agent_api.proposals", amount="500", ' +
        'asset="usd", denomination="usd_cent", ' +
        'expires_at="2026-06-06T09:10:00.000Z", ' +
        'docs_ref="docs.api.agent_proposals"',
    )
    expect(formatOpenAgentsPaymentCredentialPair({
      credential,
      proofRef,
    })).toBe(credentialPair)
    expect(formatOpenAgentsL402Authorization({ credential, proofRef })).toBe(
      `L402 ${credentialPair}`,
    )
    expect(formatOpenAgentsXOpenAgentsL402({ credential, proofRef })).toBe(
      credentialPair,
    )
  })

  test('rejects raw invoices, preimages, and non-OpenAgents credentials while formatting', () => {
    expect(() =>
      formatOpenAgentsL402WwwAuthenticate({
        amount: {
          amountMinorUnits: 500,
          asset: 'usd',
          denomination: 'usd_cent',
        },
        challengeRef: 'lnbc2500n1rawinvoice',
        docsRef: 'docs.api.agent_proposals',
        endpointRef: 'endpoint.agent_api.proposals',
        expiresAt: '2026-06-06T09:10:00.000Z',
        productId: 'product.agent_api.proposals.day',
      }),
    ).toThrow(OpenAgentsPaymentHeaderUnsafe)
    expect(() =>
      formatOpenAgentsPaymentCredentialPair({
        credential,
        proofRef: 'payment_preimage=secret',
      }),
    ).toThrow(OpenAgentsPaymentHeaderUnsafe)
    expect(() =>
      formatOpenAgentsPaymentCredentialPair({
        credential: 'legacy-lsat-token',
        proofRef,
      }),
    ).toThrow(OpenAgentsPaymentHeaderUnsafe)
  })

  test.each([
    [
      'missing',
      new Headers(),
      {
        bearerAuthPresent: false,
        credential: null,
        credentialSource: null,
        proofRef: null,
        status: 'missing',
      },
    ],
    [
      'bearer only',
      new Headers({ Authorization: 'Bearer oa_agent_unit_test' }),
      {
        bearerAuthPresent: true,
        credential: null,
        credentialSource: null,
        proofRef: null,
        status: 'bearer_only',
      },
    ],
    [
      'bearer plus OpenAgents L402',
      new Headers({
        Authorization: 'Bearer oa_agent_unit_test',
        'X-OpenAgents-L402': credentialPair,
      }),
      {
        bearerAuthPresent: true,
        credential,
        credentialSource: 'x_openagents_l402',
        proofRef,
        status: 'x_openagents_l402',
      },
    ],
    [
      'OpenAgents L402 authorization',
      new Headers({ Authorization: `L402 ${credentialPair}` }),
      {
        bearerAuthPresent: false,
        credential,
        credentialSource: 'authorization_l402',
        proofRef,
        status: 'l402_authorization',
      },
    ],
    [
      'legacy LSAT authorization',
      new Headers({ Authorization: `LSAT ${credentialPair}` }),
      {
        bearerAuthPresent: false,
        credential,
        credentialSource: 'authorization_lsat',
        proofRef,
        status: 'lsat_authorization',
      },
    ],
    [
      'unsupported authorization scheme',
      new Headers({ Authorization: 'Basic abc123' }),
      {
        bearerAuthPresent: false,
        credential: null,
        credentialSource: null,
        proofRef: null,
        status: 'unsupported_scheme',
      },
    ],
    [
      'malformed authorization proof',
      new Headers({ Authorization: `L402 ${credential}` }),
      {
        bearerAuthPresent: false,
        credential: null,
        credentialSource: null,
        proofRef: null,
        status: 'malformed',
      },
    ],
    [
      'payment header collision',
      new Headers({
        Authorization: `L402 ${credentialPair}`,
        'X-OpenAgents-L402': credentialPair,
      }),
      {
        bearerAuthPresent: false,
        credential: null,
        credentialSource: null,
        proofRef: null,
        status: 'collision',
      },
    ],
  ] as const)('parses %s', (_, headers, expected) => {
    expect(decodeParseResult(headers)).toMatchObject(expected)
  })

  test('marks unsafe parsed payment material as malformed or unsafe', () => {
    expect(decodeParseResult(new Headers({
      Authorization: `L402 ${credential}:payment_preimage=secret`,
    }))).toMatchObject({
      status: 'malformed',
    })
    expect(() =>
      parseOpenAgentsPaymentHeaders(new Headers({
        'X-OpenAgents-L402': `${credential}:payment_preimage=secret`,
      })),
    ).toThrow(OpenAgentsPaymentHeaderUnsafe)
  })

  test('projects payment headers without exposing raw credentials', () => {
    const result = decodeParseResult(new Headers({
      Authorization: 'Bearer oa_agent_unit_test',
      'X-OpenAgents-L402': credentialPair,
    }))
    const publicProjection = projectOpenAgentsPaymentHeaderResult(
      result,
      'public',
    )
    const agentProjection = projectOpenAgentsPaymentHeaderResult(
      result,
      'agent',
    )

    expect(S.decodeUnknownSync(OpenAgentsPaymentHeaderProjection)(
      publicProjection,
    )).toEqual(publicProjection)
    expect(publicProjection).toMatchObject({
      credentialPresent: true,
      proofRef: null,
      status: 'x_openagents_l402',
    })
    expect(agentProjection.proofRef).toBe(proofRef)
    expect(JSON.stringify(publicProjection)).not.toContain(credential)
    expect(openAgentsPaymentHeaderProjectionHasPrivateMaterial(
      publicProjection,
    )).toBe(false)
    expect(openAgentsPaymentHeaderProjectionHasPrivateMaterial(
      agentProjection,
    )).toBe(false)
  })
})
