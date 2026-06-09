import type {
  OpenAgentsSiteMdkWebhookConfig,
  OpenAgentsSiteMdkWebhookSource,
} from './site-mdk-webhooks'
import {
  nestedUnknown,
  optionalString,
  parseJsonRecord,
} from './json-boundary'
import { epochMillisToIsoTimestamp, normalizeIsoTimestamp } from './runtime-primitives'
import type {
  ForumMoneyAmount,
  ForumPaymentEventMode,
  ForumPaymentEventStatus,
} from './forum/schemas'

export type OpenAgentsForumMdkVerifiedWebhookEvent = Readonly<{
  amount: ForumMoneyAmount
  attemptId: string
  eventBodyDigestRef: string
  externalRef: string
  occurredAt: string
  paymentMode: ForumPaymentEventMode
  providerEventRef: string
  providerRef: string
  redactedEvidenceRef: string
  signatureBindingRef: string
  status: ForumPaymentEventStatus
}>

export type OpenAgentsForumMdkWebhookVerificationResult =
  | Readonly<{
      _tag: 'Verified'
      event: OpenAgentsForumMdkVerifiedWebhookEvent
    }>
  | Readonly<{
      _tag: 'Invalid'
      reason:
        | 'invalid_amount'
        | 'invalid_payload'
        | 'invalid_signature'
        | 'missing_configuration'
        | 'unsafe_provider_event'
    }>

const encoder = new TextEncoder()
const stableRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/
const maxProviderRefSegmentLength = 180

const cleanRefSegment = (value: string): string =>
  value
    .replaceAll(/[^A-Za-z0-9_-]+/g, '_')
    .slice(0, maxProviderRefSegmentLength)

const stableRefIsSafe = (value: string): boolean =>
  value.trim() !== '' && stableRefPattern.test(value)

const hex = (bytes: ArrayBuffer): string =>
  [...new Uint8Array(bytes)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')

const base64 = (bytes: ArrayBuffer): string =>
  btoa(String.fromCharCode(...new Uint8Array(bytes)))

const timingSafeEqual = (left: string, right: string): boolean => {
  const leftBytes = encoder.encode(left)
  const rightBytes = encoder.encode(right)

  if (leftBytes.length !== rightBytes.length) {
    return false
  }

  return (
    leftBytes.reduce(
      (diff, byte, index) => diff | (byte ^ (rightBytes[index] ?? 0)),
      0,
    ) === 0
  )
}

const arrayBufferFromBytes = (bytes: Uint8Array): ArrayBuffer =>
  bytes.buffer.slice(
    bytes.byteOffset,
    bytes.byteOffset + bytes.byteLength,
  ) as ArrayBuffer

const secretBytes = (secret: string): ArrayBuffer => {
  if (!secret.startsWith('whsec_')) {
    return arrayBufferFromBytes(encoder.encode(secret))
  }

  try {
    return arrayBufferFromBytes(
      Uint8Array.from(
        atob(secret.slice('whsec_'.length)),
        character => character.charCodeAt(0),
      ),
    )
  } catch {
    return arrayBufferFromBytes(encoder.encode(secret))
  }
}

const hmacSha256 = async (
  secret: string,
  signedPayload: string,
): Promise<ArrayBuffer> => {
  const key = await crypto.subtle.importKey(
    'raw',
    secretBytes(secret),
    { hash: 'SHA-256', name: 'HMAC' },
    false,
    ['sign'],
  )

  return crypto.subtle.sign('HMAC', key, encoder.encode(signedPayload))
}

const signatureCandidates = (signature: string): ReadonlyArray<string> =>
  signature
    .split(' ')
    .flatMap(part => part.split(','))
    .map(part =>
      part.trim().replace(/^v\d+[,=]?/u, '').replace(/^sha256=/iu, ''),
    )
    .filter(part => part !== '')

const anySignatureMatches = (
  signature: string,
  digest: ArrayBuffer,
): boolean => {
  const expectedBase64 = base64(digest)
  const expectedHex = hex(digest)

  return signatureCandidates(signature).some(
    candidate =>
      timingSafeEqual(candidate, expectedBase64) ||
      timingSafeEqual(candidate, expectedHex),
  )
}

const verifyStandardWebhooks = async (
  body: string,
  headers: Headers,
  secret: string,
): Promise<boolean> => {
  const id = headers.get('webhook-id')
  const signature = headers.get('webhook-signature')
  const timestamp = headers.get('webhook-timestamp')

  if (id === null || signature === null || timestamp === null) {
    return false
  }

  const digest = await hmacSha256(secret, `${id}.${timestamp}.${body}`)

  return anySignatureMatches(signature, digest)
}

const verifyDaemonHmac = async (
  body: string,
  headers: Headers,
  secret: string,
): Promise<boolean> => {
  const signature = headers.get('x-mdk-signature')
  const timestamp = headers.get('x-mdk-timestamp')

  if (signature === null || timestamp === null) {
    return false
  }

  const digest = await hmacSha256(secret, `${timestamp}.${body}`)

  return anySignatureMatches(signature, digest)
}

const verifySdkNodeControl = (
  headers: Headers,
  secret: string,
): boolean => {
  const supplied = headers.get('x-moneydevkit-webhook-secret')

  return supplied !== null && timingSafeEqual(supplied, secret)
}

const paymentStatusFromPayload = (
  payload: Record<string, unknown>,
): ForumPaymentEventStatus | undefined => {
  const eventType = (
    optionalString(payload.type) ??
    optionalString(payload.eventType) ??
    ''
  ).toLowerCase()
  const status = (
    optionalString(payload.status) ??
    optionalString(nestedUnknown(payload, ['data', 'status'])) ??
    optionalString(nestedUnknown(payload, ['data', 'payment', 'status'])) ??
    optionalString(nestedUnknown(payload, ['payment', 'status'])) ??
    ''
  ).toLowerCase()
  const combined = `${eventType} ${status}`

  if (
    combined.includes('paid') ||
    combined.includes('completed') ||
    combined.includes('succeeded') ||
    combined.includes('settled') ||
    combined.includes('payment_received')
  ) {
    return 'confirmed'
  }

  if (combined.includes('refund')) {
    return 'refunded'
  }

  if (combined.includes('reverse') || combined.includes('chargeback')) {
    return 'reversed'
  }

  if (combined.includes('failed') || combined.includes('expired')) {
    return 'failed'
  }

  if (combined.includes('replay') || combined.includes('duplicate')) {
    return 'replayed'
  }

  if (combined.includes('observed') || combined.includes('pending')) {
    return 'observed'
  }

  return undefined
}

const paymentModeFromPayload = (
  payload: Record<string, unknown>,
): ForumPaymentEventMode => {
  const value = (
    optionalString(payload.paymentMode) ??
    optionalString(payload.mode) ??
    optionalString(nestedUnknown(payload, ['data', 'paymentMode'])) ??
    optionalString(nestedUnknown(payload, ['data', 'mode'])) ??
    ''
  ).toLowerCase()

  return value === 'sandbox' || value === 'signet' || value === 'unknown'
    ? value
    : 'live'
}

const stringOrNumber = (value: unknown): string | undefined =>
  typeof value === 'string' || typeof value === 'number'
    ? String(value)
    : undefined

const satsAmountFromPayload = (
  payload: Record<string, unknown>,
): number | undefined => {
  const amountValue =
    stringOrNumber(payload.amountSats) ??
    stringOrNumber(payload.sats) ??
    stringOrNumber(nestedUnknown(payload, ['data', 'amountSats'])) ??
    stringOrNumber(nestedUnknown(payload, ['data', 'sats'])) ??
    stringOrNumber(nestedUnknown(payload, ['amount', 'amount'])) ??
    stringOrNumber(nestedUnknown(payload, ['data', 'amount', 'amount'])) ??
    stringOrNumber(nestedUnknown(payload, ['data', 'payment', 'amountSats']))
  const assetValue = (
    optionalString(payload.asset) ??
    optionalString(payload.currency) ??
    optionalString(nestedUnknown(payload, ['amount', 'asset'])) ??
    optionalString(nestedUnknown(payload, ['data', 'asset'])) ??
    optionalString(nestedUnknown(payload, ['data', 'currency'])) ??
    optionalString(nestedUnknown(payload, ['data', 'amount', 'asset'])) ??
    'sats'
  ).toLowerCase()
  const amount = amountValue === undefined ? NaN : Number(amountValue)

  return assetValue === 'sat' || assetValue === 'sats'
    ? Number.isInteger(amount) && amount > 0
      ? amount
      : undefined
    : undefined
}

const attemptIdFromPayload = (
  payload: Record<string, unknown>,
): string | undefined =>
  optionalString(payload.attemptId) ??
  optionalString(payload.directTipAttemptId) ??
  optionalString(nestedUnknown(payload, ['data', 'attemptId'])) ??
  optionalString(nestedUnknown(payload, ['data', 'directTipAttemptId'])) ??
  optionalString(nestedUnknown(payload, ['data', 'metadata', 'attemptId'])) ??
  optionalString(
    nestedUnknown(payload, ['data', 'metadata', 'directTipAttemptId']),
  ) ??
  optionalString(nestedUnknown(payload, ['metadata', 'attemptId'])) ??
  optionalString(nestedUnknown(payload, ['metadata', 'directTipAttemptId']))

const providerEventIdFromPayload = (
  payload: Record<string, unknown>,
  headers: Headers,
): string | undefined =>
  headers.get('webhook-id') ??
  headers.get('x-mdk-event-id') ??
  optionalString(payload.id) ??
  optionalString(payload.eventId) ??
  optionalString(payload.event_id) ??
  optionalString(nestedUnknown(payload, ['data', 'eventId'])) ??
  optionalString(nestedUnknown(payload, ['data', 'event_id']))

const occurredAtFromPayload = (
  payload: Record<string, unknown>,
  headers: Headers,
  fallback: string,
): string => {
  const value =
    optionalString(payload.createdAt) ??
    optionalString(payload.timestamp) ??
    optionalString(nestedUnknown(payload, ['data', 'createdAt'])) ??
    optionalString(nestedUnknown(payload, ['data', 'timestamp'])) ??
    headers.get('webhook-timestamp') ??
    headers.get('x-mdk-timestamp') ??
    fallback

  if (!Number.isNaN(Date.parse(value))) {
    return normalizeIsoTimestamp(value)
  }

  const numericTimestamp = Number(value)

  if (Number.isFinite(numericTimestamp) && numericTimestamp > 0) {
    const millis =
      numericTimestamp < 10_000_000_000
        ? numericTimestamp * 1000
        : numericTimestamp

    return epochMillisToIsoTimestamp(millis)
  }

  return fallback
}

const signatureVerified = async (
  source: OpenAgentsSiteMdkWebhookSource,
  body: string,
  headers: Headers,
  secret: string,
): Promise<boolean> =>
  source === 'dashboard_standard_webhooks'
    ? verifyStandardWebhooks(body, headers, secret)
    : source === 'daemon_invoice_hmac'
      ? verifyDaemonHmac(body, headers, secret)
      : verifySdkNodeControl(headers, secret)

const digestRefForForumMdkWebhookBody = async (
  source: OpenAgentsSiteMdkWebhookSource,
  body: string,
): Promise<string> => {
  const digest = await crypto.subtle.digest('SHA-256', encoder.encode(body))

  return `sha256:forum_mdk_webhook.${source}.${hex(digest)}`
}

export const verifyOpenAgentsForumMdkWebhook = async (input: {
  body: string
  config?: OpenAgentsSiteMdkWebhookConfig | undefined
  headers: Headers
  nowIso: string
}): Promise<OpenAgentsForumMdkWebhookVerificationResult> => {
  if (
    input.config === undefined ||
    input.config.secret.trim() === '' ||
    !stableRefIsSafe(input.config.bindingRef)
  ) {
    return { _tag: 'Invalid', reason: 'missing_configuration' }
  }

  if (
    !(await signatureVerified(
      input.config.source,
      input.body,
      input.headers,
      input.config.secret,
    ))
  ) {
    return { _tag: 'Invalid', reason: 'invalid_signature' }
  }

  const payload = parseJsonRecord(input.body)

  if (payload === undefined) {
    return { _tag: 'Invalid', reason: 'invalid_payload' }
  }

  const attemptId = attemptIdFromPayload(payload)
  const status = paymentStatusFromPayload(payload)
  const amount = satsAmountFromPayload(payload)
  const providerEventId = providerEventIdFromPayload(payload, input.headers)

  if (
    attemptId === undefined ||
    status === undefined ||
    providerEventId === undefined
  ) {
    return { _tag: 'Invalid', reason: 'invalid_payload' }
  }

  if (amount === undefined) {
    return { _tag: 'Invalid', reason: 'invalid_amount' }
  }

  const providerEventRef = `provider_event.mdk.${input.config.source}.${cleanRefSegment(
    providerEventId,
  )}`
  const providerRef = `provider.mdk_webhook.${input.config.source}`
  const externalRef = `external.payment.mdk_webhook.${input.config.source}.${cleanRefSegment(
    providerEventId,
  )}`
  const eventBodyDigestRef = await digestRefForForumMdkWebhookBody(
    input.config.source,
    input.body,
  )
  const redactedEvidenceRef = `evidence.payment.mdk_webhook.${input.config.source}.${cleanRefSegment(
    providerEventId,
  )}`

  if (
    !stableRefIsSafe(attemptId) ||
    !stableRefIsSafe(providerEventRef) ||
    !stableRefIsSafe(providerRef) ||
    !stableRefIsSafe(externalRef) ||
    !stableRefIsSafe(redactedEvidenceRef)
  ) {
    return { _tag: 'Invalid', reason: 'unsafe_provider_event' }
  }

  return {
    _tag: 'Verified',
    event: {
      amount: { amount, asset: 'sats' },
      attemptId,
      eventBodyDigestRef,
      externalRef,
      occurredAt: occurredAtFromPayload(payload, input.headers, input.nowIso),
      paymentMode: paymentModeFromPayload(payload),
      providerEventRef,
      providerRef,
      redactedEvidenceRef,
      signatureBindingRef: input.config.bindingRef,
      status,
    },
  }
}
