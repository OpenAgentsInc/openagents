import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import { BuyerPaymentLedgerAmount } from './buyer-payment-ledger'
import { OpenAgentsPaymentPolicyAudience } from './payment-limit-policy'
import { openAgentsRunnerGatewayPayloadHasPrivateMaterial } from './runner-gateway'

export const OpenAgentsPaymentHeaderParseStatus = S.Literals([
  'bearer_only',
  'collision',
  'l402_authorization',
  'lsat_authorization',
  'malformed',
  'missing',
  'unsupported_scheme',
  'x_openagents_l402',
])
export type OpenAgentsPaymentHeaderParseStatus =
  typeof OpenAgentsPaymentHeaderParseStatus.Type

export const OpenAgentsPaymentHeaderCredentialSource = S.Literals([
  'authorization_l402',
  'authorization_lsat',
  'x_openagents_l402',
])
export type OpenAgentsPaymentHeaderCredentialSource =
  typeof OpenAgentsPaymentHeaderCredentialSource.Type

export const OpenAgentsPaymentHeaderParseResult = S.Struct({
  bearerAuthPresent: S.Boolean,
  challengeRef: S.NullOr(S.String),
  credential: S.NullOr(S.String),
  credentialSource: S.NullOr(OpenAgentsPaymentHeaderCredentialSource),
  proofRef: S.NullOr(S.String),
  reasonRef: S.String,
  status: OpenAgentsPaymentHeaderParseStatus,
})
export type OpenAgentsPaymentHeaderParseResult =
  typeof OpenAgentsPaymentHeaderParseResult.Type

export const OpenAgentsPaymentHeaderProjection = S.Struct({
  audience: OpenAgentsPaymentPolicyAudience,
  bearerAuthPresent: S.Boolean,
  challengeRef: S.NullOr(S.String),
  credentialPresent: S.Boolean,
  credentialSource: S.NullOr(OpenAgentsPaymentHeaderCredentialSource),
  proofRef: S.NullOr(S.String),
  reasonRef: S.String,
  status: OpenAgentsPaymentHeaderParseStatus,
})
export type OpenAgentsPaymentHeaderProjection =
  typeof OpenAgentsPaymentHeaderProjection.Type

export const OpenAgentsL402WwwAuthenticateInput = S.Struct({
  amount: BuyerPaymentLedgerAmount,
  challengeRef: S.String,
  docsRef: S.String,
  endpointRef: S.String,
  expiresAt: S.String,
  productId: S.String,
})
export type OpenAgentsL402WwwAuthenticateInput =
  typeof OpenAgentsL402WwwAuthenticateInput.Type

export class OpenAgentsPaymentHeaderUnsafe extends S.TaggedErrorClass<OpenAgentsPaymentHeaderUnsafe>()(
  'OpenAgentsPaymentHeaderUnsafe',
  {
    reason: S.String,
  },
) {}

const unsafeHeaderValuePattern =
  /(bearer\s+|checkout_id=|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github[_-]?pat_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|mdk_access_token|mnemonic|payment_preimage=|preimage|provider[_-]?token|raw[_-]?invoice|raw[_-]?payment|raw[_-]?payload|raw[_-]?prompt|raw[_-]?runner|raw[_-]?run[_-]?log|secret|sk-[a-z0-9]|\S+@\S+)/i

const stableRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,320}$/

const isUnsafeHeaderValue = (value: unknown): boolean =>
  containsProviderSecretMaterial(String(value)) ||
  unsafeHeaderValuePattern.test(String(value)) ||
  openAgentsRunnerGatewayPayloadHasPrivateMaterial(value)

const assertSafeHeaderValue = (label: string, value: unknown): void => {
  if (isUnsafeHeaderValue(value)) {
    throw new OpenAgentsPaymentHeaderUnsafe({
      reason: `${label} contains private or payment-secret material.`,
    })
  }
}

const safeRef = (value: string): string | undefined =>
  value.trim() !== '' &&
  stableRefPattern.test(value) &&
  !isUnsafeHeaderValue(value)
    ? value
    : undefined

const quoted = (value: string): string => `"${value.replaceAll('"', '')}"`

export const formatOpenAgentsL402WwwAuthenticate = (
  input: OpenAgentsL402WwwAuthenticateInput,
): string => {
  assertSafeHeaderValue('L402 WWW-Authenticate input', input)

  const fields = [
    ['challenge_ref', input.challengeRef],
    ['product_id', input.productId],
    ['endpoint_ref', input.endpointRef],
    ['amount', String(input.amount.amountMinorUnits)],
    ['asset', input.amount.asset],
    ['denomination', input.amount.denomination],
    ['expires_at', input.expiresAt],
    ['docs_ref', input.docsRef],
  ] as const

  if (
    !fields.every(([, value]) =>
      safeRef(value) !== undefined || /^\d+$/u.test(value)
    )
  ) {
    throw new OpenAgentsPaymentHeaderUnsafe({
      reason: 'L402 WWW-Authenticate fields must be public-safe refs.',
    })
  }

  return `L402 ${fields
    .map(([key, value]) => `${key}=${quoted(value)}`)
    .join(', ')}`
}

export const formatOpenAgentsPaymentCredentialPair = (input: {
  credential: string
  proofRef: string
}): string => {
  assertSafeHeaderValue('OpenAgents payment proof ref', input.proofRef)

  if (!input.credential.startsWith('oa-l402-v1.')) {
    throw new OpenAgentsPaymentHeaderUnsafe({
      reason: 'OpenAgents L402 credential must use the oa-l402-v1 format.',
    })
  }

  if (safeRef(input.proofRef) === undefined) {
    throw new OpenAgentsPaymentHeaderUnsafe({
      reason: 'OpenAgents payment proof must be a public-safe proof ref.',
    })
  }

  return `${input.credential}:${input.proofRef}`
}

export const formatOpenAgentsL402Authorization = (input: {
  credential: string
  proofRef: string
}): string => `L402 ${formatOpenAgentsPaymentCredentialPair(input)}`

export const formatOpenAgentsXOpenAgentsL402 = formatOpenAgentsPaymentCredentialPair

const emptyResult = (
  status: OpenAgentsPaymentHeaderParseStatus,
  reasonRef: string,
  bearerAuthPresent = false,
): OpenAgentsPaymentHeaderParseResult => ({
  bearerAuthPresent,
  challengeRef: null,
  credential: null,
  credentialSource: null,
  proofRef: null,
  reasonRef,
  status,
})

const parseCredentialPair = (value: string): Readonly<{
  credential: string
  proofRef: string
}> | undefined => {
  const separatorIndex = value.indexOf(':')

  if (separatorIndex <= 0 || separatorIndex === value.length - 1) {
    return undefined
  }

  const credential = value.slice(0, separatorIndex)
  const proofRef = value.slice(separatorIndex + 1)

  if (
    !credential.startsWith('oa-l402-v1.') ||
    safeRef(proofRef) === undefined
  ) {
    return undefined
  }

  return { credential, proofRef }
}

const parsePaymentHeaderValue = (
  value: string,
  source: OpenAgentsPaymentHeaderCredentialSource,
  status: OpenAgentsPaymentHeaderParseStatus,
  bearerAuthPresent: boolean,
): OpenAgentsPaymentHeaderParseResult => {
  const parsed = parseCredentialPair(value.trim())

  return parsed === undefined
    ? emptyResult(
        'malformed',
        'reason.payment_header.malformed_payment_credential',
        bearerAuthPresent,
      )
    : {
        bearerAuthPresent,
        challengeRef: null,
        credential: parsed.credential,
        credentialSource: source,
        proofRef: parsed.proofRef,
        reasonRef: `reason.payment_header.${status}`,
        status,
      }
}

export const parseOpenAgentsPaymentHeaders = (
  headers: Headers,
): OpenAgentsPaymentHeaderParseResult => {
  const authorization = headers.get('authorization')
  const xOpenAgentsL402 = headers.get('x-openagents-l402')

  if (authorization === null && xOpenAgentsL402 === null) {
    return emptyResult('missing', 'reason.payment_header.missing')
  }

  if (xOpenAgentsL402 !== null) {
    assertSafeHeaderValue('X-OpenAgents-L402 header', xOpenAgentsL402)
  }

  if (authorization === null) {
    return parsePaymentHeaderValue(
      xOpenAgentsL402 ?? '',
      'x_openagents_l402',
      'x_openagents_l402',
      false,
    )
  }

  const [scheme, ...rest] = authorization.trim().split(/\s+/u)
  const credentialText = rest.join(' ')
  const lowerScheme = scheme?.toLowerCase()
  const bearerAuthPresent = lowerScheme === 'bearer'

  if (bearerAuthPresent) {
    return xOpenAgentsL402 === null
      ? emptyResult(
          'bearer_only',
          'reason.payment_header.bearer_auth_without_payment',
          true,
        )
      : parsePaymentHeaderValue(
          xOpenAgentsL402,
          'x_openagents_l402',
          'x_openagents_l402',
          true,
        )
  }

  if (xOpenAgentsL402 !== null) {
    return emptyResult(
      'collision',
      'reason.payment_header.authorization_payment_collision',
      false,
    )
  }

  if (lowerScheme === 'l402') {
    return parsePaymentHeaderValue(
      credentialText,
      'authorization_l402',
      'l402_authorization',
      false,
    )
  }

  if (lowerScheme === 'lsat') {
    return parsePaymentHeaderValue(
      credentialText,
      'authorization_lsat',
      'lsat_authorization',
      false,
    )
  }

  return emptyResult(
    'unsupported_scheme',
    'reason.payment_header.unsupported_authorization_scheme',
    false,
  )
}

export const projectOpenAgentsPaymentHeaderResult = (
  result: OpenAgentsPaymentHeaderParseResult,
  audience: typeof OpenAgentsPaymentPolicyAudience.Type,
): OpenAgentsPaymentHeaderProjection => {
  const privileged = audience === 'agent' || audience === 'customer' ||
    audience === 'operator'

  return {
    audience,
    bearerAuthPresent: result.bearerAuthPresent,
    challengeRef: result.challengeRef,
    credentialPresent: result.credential !== null,
    credentialSource: result.credentialSource,
    proofRef: privileged ? result.proofRef : null,
    reasonRef: result.reasonRef,
    status: result.status,
  }
}

export const openAgentsPaymentHeaderProjectionHasPrivateMaterial = (
  projection: OpenAgentsPaymentHeaderProjection,
): boolean => isUnsafeHeaderValue(projection)
