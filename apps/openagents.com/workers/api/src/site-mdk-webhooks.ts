import { Schema as S } from 'effect'

import {
  nestedUnknown,
  optionalString,
  parseJsonRecord,
} from './json-boundary'
import {
  type OpenAgentsHostedMdkCheckoutStatus,
} from './hosted-mdk-client'
import type { OpenAgentsSiteMdkProviderEventKind } from './site-mdk-reconciliation'
import {
  epochMillisToIsoTimestamp,
  normalizeIsoTimestamp,
} from './runtime-primitives'

export const OpenAgentsSiteMdkWebhookSource = S.Literals([
  'daemon_invoice_hmac',
  'dashboard_standard_webhooks',
  'sdk_node_control',
])
export type OpenAgentsSiteMdkWebhookSource =
  typeof OpenAgentsSiteMdkWebhookSource.Type

export type OpenAgentsSiteMdkWebhookConfig = Readonly<{
  bindingRef: string
  secret: string
  source: OpenAgentsSiteMdkWebhookSource
}>

export type OpenAgentsSiteMdkVerifiedWebhookEvent = Readonly<{
  checkoutRef: string
  checkoutStatus: OpenAgentsHostedMdkCheckoutStatus
  eventBodyDigestRef: string
  eventKind: OpenAgentsSiteMdkProviderEventKind
  occurredAt: string
  providerEventRef: string
  signatureBindingRef: string
}>

export type OpenAgentsSiteMdkWebhookVerificationResult =
  | Readonly<{
      _tag: 'Verified'
      event: OpenAgentsSiteMdkVerifiedWebhookEvent
    }>
  | Readonly<{
      _tag: 'Invalid'
      reason:
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

const checkoutStatusFromPayload = (
  payload: Record<string, unknown>,
): OpenAgentsHostedMdkCheckoutStatus | undefined => {
  const eventType = (
    optionalString(payload.type) ??
    optionalString(payload.eventType) ??
    ''
  ).toLowerCase()
  const status = (
    optionalString(payload.status) ??
    optionalString(nestedUnknown(payload, ['data', 'status'])) ??
    optionalString(nestedUnknown(payload, ['data', 'checkout', 'status'])) ??
    optionalString(nestedUnknown(payload, ['checkout', 'status'])) ??
    ''
  ).toLowerCase()
  const combined = `${eventType} ${status}`

  if (
    combined.includes('completed') ||
    combined.includes('paid') ||
    combined.includes('payment_received') ||
    combined.includes('incoming-payment') ||
    combined.includes('incoming_payment')
  ) {
    return 'payment_received'
  }

  if (combined.includes('expired')) {
    return 'expired'
  }

  if (
    combined.includes('pending') ||
    combined.includes('unconfirmed') ||
    combined.includes('payment_pending')
  ) {
    return 'pending_payment'
  }

  if (combined.includes('created') || combined.includes('confirmed')) {
    return 'created'
  }

  return undefined
}

const eventKindFromStatus = (
  status: OpenAgentsHostedMdkCheckoutStatus,
): OpenAgentsSiteMdkProviderEventKind =>
  status === 'payment_received'
    ? 'payment_received'
    : status === 'pending_payment'
      ? 'payment_pending'
      : status === 'expired'
        ? 'checkout_expired'
        : 'checkout_observed'

const checkoutIdFromPayload = (
  payload: Record<string, unknown>,
): string | undefined =>
  optionalString(payload.checkoutId) ??
  optionalString(payload.checkout_id) ??
  optionalString(nestedUnknown(payload, ['checkout', 'id'])) ??
  optionalString(nestedUnknown(payload, ['data', 'checkoutId'])) ??
  optionalString(nestedUnknown(payload, ['data', 'checkout_id'])) ??
  optionalString(nestedUnknown(payload, ['data', 'checkout', 'id'])) ??
  optionalString(nestedUnknown(payload, ['data', 'id']))

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

const checkoutRefFromProviderId = (checkoutId: string): string | undefined => {
  const ref = checkoutId.startsWith('mdk_checkout.')
    ? checkoutId
    : `mdk_checkout.${cleanRefSegment(checkoutId)}`

  return stableRefIsSafe(ref) ? ref : undefined
}

export const digestRefForSiteMdkWebhookBody = async (
  source: OpenAgentsSiteMdkWebhookSource,
  body: string,
): Promise<string> => {
  const digest = await crypto.subtle.digest('SHA-256', encoder.encode(body))

  return `sha256:site_mdk_webhook.${source}.${hex(digest)}`
}

export const verifyOpenAgentsSiteMdkWebhook = async (
  input: Readonly<{
    body: string
    config?: OpenAgentsSiteMdkWebhookConfig | undefined
    headers: Headers
    nowIso: string
  }>,
): Promise<OpenAgentsSiteMdkWebhookVerificationResult> => {
  if (
    input.config === undefined ||
    input.config.secret.trim() === '' ||
    !stableRefIsSafe(input.config.bindingRef)
  ) {
    return { _tag: 'Invalid', reason: 'missing_configuration' }
  }

  const signatureVerified =
    input.config.source === 'dashboard_standard_webhooks'
      ? await verifyStandardWebhooks(
          input.body,
          input.headers,
          input.config.secret,
        )
      : input.config.source === 'daemon_invoice_hmac'
        ? await verifyDaemonHmac(input.body, input.headers, input.config.secret)
        : verifySdkNodeControl(input.headers, input.config.secret)

  if (!signatureVerified) {
    return { _tag: 'Invalid', reason: 'invalid_signature' }
  }

  const payload = parseJsonRecord(input.body)

  if (payload === undefined) {
    return { _tag: 'Invalid', reason: 'invalid_payload' }
  }

  const checkoutStatus = checkoutStatusFromPayload(payload)
  const checkoutId = checkoutIdFromPayload(payload)
  const providerEventId = providerEventIdFromPayload(payload, input.headers)

  if (
    checkoutStatus === undefined ||
    checkoutId === undefined ||
    providerEventId === undefined
  ) {
    return { _tag: 'Invalid', reason: 'invalid_payload' }
  }

  const checkoutRef = checkoutRefFromProviderId(checkoutId)
  const providerEventRef = `provider_event.mdk.${input.config.source}.${cleanRefSegment(
    providerEventId,
  )}`

  if (
    checkoutRef === undefined ||
    !stableRefIsSafe(providerEventRef) ||
    providerEventRef.includes('secret')
  ) {
    return { _tag: 'Invalid', reason: 'unsafe_provider_event' }
  }

  return {
    _tag: 'Verified',
    event: {
      checkoutRef,
      checkoutStatus,
      eventBodyDigestRef: await digestRefForSiteMdkWebhookBody(
        input.config.source,
        input.body,
      ),
      eventKind: eventKindFromStatus(checkoutStatus),
      occurredAt: occurredAtFromPayload(payload, input.headers, input.nowIso),
      providerEventRef,
      signatureBindingRef: input.config.bindingRef,
    },
  }
}
