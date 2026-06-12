import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  type BuyerPaymentChallengeRecord,
  BuyerPaymentLedgerAmount,
  decodeBuyerPaymentLedgerAmount,
} from './buyer-payment-ledger'
import {
  OpenAgentsPaidEndpointMethod,
} from './paid-endpoint-product-catalog'
import { OpenAgentsPaymentPolicyAudience } from './payment-limit-policy'
import { parseJsonUnknown } from './json-boundary'
import { openAgentsRunnerGatewayPayloadHasPrivateMaterial } from './runner-gateway'

export const OpenAgentsL402CredentialVersion = S.Literal('oa-l402-v1')
export type OpenAgentsL402CredentialVersion =
  typeof OpenAgentsL402CredentialVersion.Type

export const OpenAgentsL402CredentialPayload = S.Struct({
  amount: BuyerPaymentLedgerAmount,
  challengeRef: S.String,
  credentialRef: S.String,
  endpointRef: S.String,
  entitlementScopeRefs: S.Array(S.String),
  expiresAt: S.String,
  idempotencyKeyHash: S.String,
  issuedAt: S.String,
  method: OpenAgentsPaidEndpointMethod,
  path: S.String,
  paymentHashRef: S.String,
  productId: S.String,
  replayNonceRef: S.String,
  requestBodyDigest: S.NullOr(S.String),
  version: OpenAgentsL402CredentialVersion,
})
export type OpenAgentsL402CredentialPayload =
  typeof OpenAgentsL402CredentialPayload.Type

export const OpenAgentsL402CredentialEnvelope = S.Struct({
  credential: S.String,
  payload: OpenAgentsL402CredentialPayload,
  payloadBase64Url: S.String,
  signatureBase64Url: S.String,
  signerRef: S.String,
})
export type OpenAgentsL402CredentialEnvelope =
  typeof OpenAgentsL402CredentialEnvelope.Type

export const OpenAgentsL402CredentialProjection = S.Struct({
  amount: BuyerPaymentLedgerAmount,
  audience: OpenAgentsPaymentPolicyAudience,
  challengeRef: S.String,
  credentialRef: S.String,
  endpointRef: S.String,
  entitlementScopeRefs: S.Array(S.String),
  expiresAt: S.String,
  method: OpenAgentsPaidEndpointMethod,
  path: S.String,
  paymentHashRef: S.NullOr(S.String),
  productId: S.String,
  publicSummaryRef: S.String,
  replayNonceRef: S.NullOr(S.String),
  signerRef: S.NullOr(S.String),
  version: OpenAgentsL402CredentialVersion,
})
export type OpenAgentsL402CredentialProjection =
  typeof OpenAgentsL402CredentialProjection.Type

export const OpenAgentsL402VerificationStatus = S.Literals([
  'amount_mismatch',
  'consumed_or_replayed',
  'expired',
  'malformed',
  'proof_missing',
  'resource_mismatch',
  'signature_invalid',
  'valid',
])
export type OpenAgentsL402VerificationStatus =
  typeof OpenAgentsL402VerificationStatus.Type

export const OpenAgentsL402VerificationResult = S.Struct({
  credentialRef: S.NullOr(S.String),
  payload: S.NullOr(OpenAgentsL402CredentialPayload),
  reasonRef: S.String,
  status: OpenAgentsL402VerificationStatus,
})
export type OpenAgentsL402VerificationResult =
  typeof OpenAgentsL402VerificationResult.Type

export type OpenAgentsL402SigningBoundary = Readonly<{
  signerRef: string
  sign: (canonicalPayload: string) => Promise<string>
  verify: (canonicalPayload: string, signatureBase64Url: string) => Promise<boolean>
}>

export type OpenAgentsL402VerificationExpectation = Readonly<{
  amount: typeof BuyerPaymentLedgerAmount.Type
  challengeRef: string
  consumedCredentialRefs?: ReadonlyArray<string> | undefined
  endpointRef: string
  entitlementScopeRefs?: ReadonlyArray<string> | undefined
  method: typeof OpenAgentsPaidEndpointMethod.Type
  nowIso: string
  path: string
  paymentProofRef?: string | null | undefined
  productId: string
  requestBodyDigest?: string | null | undefined
  requirePaymentProof?: boolean | undefined
}>

export class OpenAgentsL402CredentialUnsafe extends S.TaggedErrorClass<OpenAgentsL402CredentialUnsafe>()(
  'OpenAgentsL402CredentialUnsafe',
  {
    reason: S.String,
  },
) {}

const textEncoder = new TextEncoder()
const textDecoder = new TextDecoder()

const unsafeL402KeyPattern =
  /(access[_-]?token|bearer|callback[_-]?token|cookie|customer[_-]?(email|name)|email[_-]?body|invoice|mdk|mnemonic|oauth|payment[_-]?preimage|preimage|private[_-]?key|provider[_-]?(account|grant|payload|token)|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log)|secret|source[_-]?archive|wallet|webhook)/i

const unsafeL402ValuePattern =
  /(bearer\s+|checkout_id=|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github[_-]?pat_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|mdk_access_token|mnemonic|payment_preimage=|preimage|provider[_-]?token|raw[_-]?invoice|raw[_-]?payment|raw[_-]?payload|raw[_-]?prompt|raw[_-]?runner|raw[_-]?run[_-]?log|secret|sk-[a-z0-9]|\S+@\S+)/i

const stableRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/

const scanForUnsafeL402Material = (
  value: unknown,
  path: ReadonlyArray<string> = [],
): string | undefined => {
  if (typeof value === 'string') {
    return containsProviderSecretMaterial(value) ||
      unsafeL402ValuePattern.test(value) ||
      openAgentsRunnerGatewayPayloadHasPrivateMaterial(value)
      ? path.join('.') || '<root>'
      : undefined
  }

  if (Array.isArray(value)) {
    for (const [index, item] of value.entries()) {
      const unsafePath = scanForUnsafeL402Material(item, [
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
    if (unsafeL402KeyPattern.test(key)) {
      return [...path, key].join('.')
    }

    const unsafePath = scanForUnsafeL402Material(item, [...path, key])

    if (unsafePath !== undefined) {
      return unsafePath
    }
  }

  return undefined
}

const assertSafeL402Value = (label: string, value: unknown): void => {
  const unsafePath = scanForUnsafeL402Material(value)

  if (unsafePath !== undefined) {
    throw new OpenAgentsL402CredentialUnsafe({
      reason: `${label} contains private or payment-secret material at ${unsafePath}.`,
    })
  }
}

const safeRef = (value: string): string | undefined =>
  value.trim() !== '' &&
  stableRefPattern.test(value) &&
  scanForUnsafeL402Material(value) === undefined
    ? value
    : undefined

const safeRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs)]
    .map(ref => safeRef(ref))
    .filter((ref): ref is string => ref !== undefined)

const base64UrlEncode = (bytes: Uint8Array): string => {
  let binary = ''

  for (const byte of bytes) {
    binary += String.fromCharCode(byte)
  }

  return btoa(binary).replaceAll('+', '-').replaceAll('/', '_').replace(/=+$/u, '')
}

const base64UrlDecode = (value: string): Uint8Array => {
  const normalized = value.replaceAll('-', '+').replaceAll('_', '/')
  const padded = normalized.padEnd(Math.ceil(normalized.length / 4) * 4, '=')
  const binary = atob(padded)
  const bytes = new Uint8Array(binary.length)

  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index)
  }

  return bytes
}

const arrayBufferFromBytes = (bytes: Uint8Array): ArrayBuffer =>
  bytes.buffer.slice(
    bytes.byteOffset,
    bytes.byteOffset + bytes.byteLength,
  ) as ArrayBuffer

const stableJson = (value: unknown): string => {
  if (value === null || typeof value !== 'object') {
    return JSON.stringify(value)
  }

  if (Array.isArray(value)) {
    return `[${value.map(stableJson).join(',')}]`
  }

  const record = value as Record<string, unknown>

  return `{${Object.keys(record)
    .sort()
    .map(key => `${JSON.stringify(key)}:${stableJson(record[key])}`)
    .join(',')}}`
}

const canonicalPayload = (payload: OpenAgentsL402CredentialPayload): string =>
  stableJson(payload)

const parseCredential = (
  credential: string,
): Readonly<{
  payload: OpenAgentsL402CredentialPayload
  payloadBase64Url: string
  signatureBase64Url: string
}> | undefined => {
  const [version, payloadBase64Url, signatureBase64Url, extra] =
    credential.split('.')

  if (
    version !== 'oa-l402-v1' ||
    payloadBase64Url === undefined ||
    signatureBase64Url === undefined ||
    extra !== undefined
  ) {
    return undefined
  }

  try {
    const payloadUnknown = parseJsonUnknown(
      textDecoder.decode(base64UrlDecode(payloadBase64Url)),
    )
    const payload = S.decodeUnknownSync(OpenAgentsL402CredentialPayload)(
      payloadUnknown,
    )

    assertSafeL402Value('OpenAgents L402 credential payload', payload)
    decodeBuyerPaymentLedgerAmount(payload.amount)

    return {
      payload,
      payloadBase64Url,
      signatureBase64Url,
    }
  } catch {
    return undefined
  }
}

export const makeOpenAgentsL402HmacSigningBoundary = async (input: {
  secretKeyMaterial: string
  signerRef: string
}): Promise<OpenAgentsL402SigningBoundary> => {
  if (input.secretKeyMaterial.trim() === '') {
    throw new OpenAgentsL402CredentialUnsafe({
      reason: 'L402 signing key material is required.',
    })
  }

  const key = await crypto.subtle.importKey(
    'raw',
    textEncoder.encode(input.secretKeyMaterial),
    { hash: 'SHA-256', name: 'HMAC' },
    false,
    ['sign', 'verify'],
  )

  return {
    signerRef: input.signerRef,
    sign: async canonical => {
      const signature = await crypto.subtle.sign(
        'HMAC',
        key,
        textEncoder.encode(canonical),
      )

      return base64UrlEncode(new Uint8Array(signature))
    },
    verify: async (canonical, signatureBase64Url) =>
      crypto.subtle.verify(
        'HMAC',
        key,
        arrayBufferFromBytes(base64UrlDecode(signatureBase64Url)),
        textEncoder.encode(canonical),
      ),
  }
}

export const l402PayloadFromBuyerPaymentChallenge = (input: {
  challenge: BuyerPaymentChallengeRecord
  credentialRef: string
  endpointRef: string
  entitlementScopeRefs: ReadonlyArray<string>
  issuedAt: string
  paymentHashRef: string
  replayNonceRef: string
}): OpenAgentsL402CredentialPayload => {
  assertSafeL402Value('Buyer payment challenge L402 payload input', input)

  return {
    amount: decodeBuyerPaymentLedgerAmount(input.challenge.price),
    challengeRef: input.challenge.challengeRef,
    credentialRef: input.credentialRef,
    endpointRef: input.endpointRef,
    entitlementScopeRefs: safeRefs(input.entitlementScopeRefs),
    expiresAt: input.challenge.expiresAt,
    idempotencyKeyHash: input.challenge.idempotencyKeyHash,
    issuedAt: input.issuedAt,
    method: input.challenge.method,
    path: input.challenge.path,
    paymentHashRef: input.paymentHashRef,
    productId: input.challenge.productId,
    replayNonceRef: input.replayNonceRef,
    requestBodyDigest: input.challenge.requestBodyDigest,
    version: 'oa-l402-v1',
  }
}

export const mintOpenAgentsL402Credential = async (
  payload: OpenAgentsL402CredentialPayload,
  signer: OpenAgentsL402SigningBoundary,
): Promise<OpenAgentsL402CredentialEnvelope> => {
  assertSafeL402Value('OpenAgents L402 credential payload', payload)
  decodeBuyerPaymentLedgerAmount(payload.amount)

  const canonical = canonicalPayload(payload)
  const payloadBase64Url = base64UrlEncode(textEncoder.encode(canonical))
  const signatureBase64Url = await signer.sign(canonical)
  const credential = [
    payload.version,
    payloadBase64Url,
    signatureBase64Url,
  ].join('.')
  const envelope = {
    credential,
    payload,
    payloadBase64Url,
    signatureBase64Url,
    signerRef: signer.signerRef,
  }

  assertSafeL402Value('OpenAgents L402 credential envelope', envelope)

  return envelope
}

const verificationResult = (
  status: typeof OpenAgentsL402VerificationStatus.Type,
  reasonRef: string,
  payload: OpenAgentsL402CredentialPayload | null,
): OpenAgentsL402VerificationResult => ({
  credentialRef: payload?.credentialRef ?? null,
  payload,
  reasonRef,
  status,
})

const amountsEqual = (
  left: typeof BuyerPaymentLedgerAmount.Type,
  right: typeof BuyerPaymentLedgerAmount.Type,
): boolean =>
  left.amountMinorUnits === right.amountMinorUnits &&
  left.asset === right.asset &&
  left.denomination === right.denomination

const resourceMatches = (
  payload: OpenAgentsL402CredentialPayload,
  expected: OpenAgentsL402VerificationExpectation,
): boolean =>
  payload.challengeRef === expected.challengeRef &&
  payload.endpointRef === expected.endpointRef &&
  payload.method === expected.method &&
  payload.path === expected.path &&
  payload.productId === expected.productId &&
  (expected.requestBodyDigest === undefined ||
    expected.requestBodyDigest === null ||
    payload.requestBodyDigest === expected.requestBodyDigest)

const entitlementScopesMatch = (
  payload: OpenAgentsL402CredentialPayload,
  expectedScopes: ReadonlyArray<string> | undefined,
): boolean =>
  expectedScopes === undefined ||
  expectedScopes.length === 0 ||
  expectedScopes.every(scope => payload.entitlementScopeRefs.includes(scope))

export const verifyOpenAgentsL402Credential = async (
  credential: string,
  signer: OpenAgentsL402SigningBoundary,
  expected: OpenAgentsL402VerificationExpectation,
): Promise<OpenAgentsL402VerificationResult> => {
  assertSafeL402Value('OpenAgents L402 verification expectation', expected)
  decodeBuyerPaymentLedgerAmount(expected.amount)

  const parsed = parseCredential(credential)

  if (parsed === undefined) {
    return verificationResult(
      'malformed',
      'reason.l402_credential.malformed',
      null,
    )
  }

  const payload = parsed.payload

  if (
    !(await signer.verify(
      canonicalPayload(payload),
      parsed.signatureBase64Url,
    ))
  ) {
    return verificationResult(
      'signature_invalid',
      'reason.l402_credential.signature_invalid',
      payload,
    )
  }

  if (
    expected.requirePaymentProof === true &&
    (expected.paymentProofRef === undefined ||
      expected.paymentProofRef === null ||
      safeRef(expected.paymentProofRef) === undefined)
  ) {
    return verificationResult(
      'proof_missing',
      'reason.l402_credential.proof_missing',
      payload,
    )
  }

  if (payload.expiresAt <= expected.nowIso) {
    return verificationResult(
      'expired',
      'reason.l402_credential.expired',
      payload,
    )
  }

  if (
    (expected.consumedCredentialRefs ?? []).includes(payload.credentialRef) ||
    (expected.consumedCredentialRefs ?? []).includes(payload.replayNonceRef)
  ) {
    return verificationResult(
      'consumed_or_replayed',
      'reason.l402_credential.consumed_or_replayed',
      payload,
    )
  }

  if (
    !resourceMatches(payload, expected) ||
    !entitlementScopesMatch(payload, expected.entitlementScopeRefs)
  ) {
    return verificationResult(
      'resource_mismatch',
      'reason.l402_credential.resource_mismatch',
      payload,
    )
  }

  if (!amountsEqual(payload.amount, expected.amount)) {
    return verificationResult(
      'amount_mismatch',
      'reason.l402_credential.amount_mismatch',
      payload,
    )
  }

  return verificationResult('valid', 'reason.l402_credential.valid', payload)
}

export const projectOpenAgentsL402Credential = (
  envelope: OpenAgentsL402CredentialEnvelope,
  audience: typeof OpenAgentsPaymentPolicyAudience.Type,
): OpenAgentsL402CredentialProjection => {
  const privileged = audience === 'agent' || audience === 'customer'
  const operator = audience === 'operator'

  return {
    amount: envelope.payload.amount,
    audience,
    challengeRef: safeRef(envelope.payload.challengeRef) ?? 'challenge.redacted',
    credentialRef:
      safeRef(envelope.payload.credentialRef) ?? 'credential.redacted',
    endpointRef: safeRef(envelope.payload.endpointRef) ?? 'endpoint.redacted',
    entitlementScopeRefs: safeRefs(envelope.payload.entitlementScopeRefs),
    expiresAt: envelope.payload.expiresAt,
    method: envelope.payload.method,
    path: envelope.payload.path,
    paymentHashRef:
      privileged || operator
        ? safeRef(envelope.payload.paymentHashRef) ?? null
        : null,
    productId: safeRef(envelope.payload.productId) ?? 'product.redacted',
    publicSummaryRef: 'summary.l402_credential.redacted.v1',
    replayNonceRef:
      operator ? safeRef(envelope.payload.replayNonceRef) ?? null : null,
    signerRef: operator ? safeRef(envelope.signerRef) ?? null : null,
    version: envelope.payload.version,
  }
}

export const openAgentsL402CredentialProjectionHasPrivateMaterial = (
  projection: OpenAgentsL402CredentialProjection,
): boolean => scanForUnsafeL402Material(projection) !== undefined
