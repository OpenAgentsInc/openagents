import { Redacted } from 'effect'

import type { WorkerSecret } from './config'
import { recordProviderEmailSuppression } from './email-preferences'
import {
  nestedUnknown,
  optionalString,
  parseJsonRecord,
  stringArrayFromUnknown,
} from './json-boundary'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

export type ResendWebhookResult = Readonly<{
  duplicate: boolean
  eventType: string
  providerEventId: string
  status: 'accepted' | 'invalid' | 'unauthorized'
}>

export type ResendWebhookRuntime = Readonly<{
  makeId: (prefix: string) => string
  nowIso: () => string
}>

export const systemResendWebhookRuntime: ResendWebhookRuntime = {
  makeId: compactRandomId,
  nowIso: currentIsoTimestamp,
}

type DeliveryProjection = Readonly<{
  errorMessage: string | null
  errorName: string | null
  status: 'accepted' | 'failed' | 'unknown_external_state'
}>

const encoder = new TextEncoder()

const compactText = (value: string | null | undefined, maxLength: number) =>
  value?.trim().replace(/\s+/g, ' ').slice(0, maxLength) ?? null

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
  const value = secret.startsWith('whsec_')
    ? secret.slice('whsec_'.length)
    : secret

  try {
    return arrayBufferFromBytes(
      Uint8Array.from(atob(value), character => character.charCodeAt(0)),
    )
  } catch {
    return arrayBufferFromBytes(encoder.encode(value))
  }
}

export const verifyResendWebhookSignature = async (
  input: Readonly<{
    body: string
    headers: Headers
    secret: Redacted.Redacted<WorkerSecret>
  }>,
): Promise<boolean> => {
  const id = input.headers.get('svix-id')
  const signature = input.headers.get('svix-signature')
  const timestamp = input.headers.get('svix-timestamp')

  if (id === null || signature === null || timestamp === null) {
    return false
  }

  const key = await crypto.subtle.importKey(
    'raw',
    secretBytes(Redacted.value(input.secret)),
    { hash: 'SHA-256', name: 'HMAC' },
    false,
    ['sign'],
  )
  const signed = `${id}.${timestamp}.${input.body}`
  const digest = await crypto.subtle.sign('HMAC', key, encoder.encode(signed))
  const expectedBase64 = base64(digest)
  const expectedHex = hex(digest)

  return signature
    .split(' ')
    .flatMap(part => part.split(','))
    .some(part => {
      const candidate = part.trim().replace(/^v1,?/, '')

      return (
        timingSafeEqual(candidate, expectedBase64) ||
        timingSafeEqual(candidate, expectedHex)
      )
    })
}

const firstEmail = (value: unknown): string | null => {
  const values = stringArrayFromUnknown(value)

  return values[0] ?? optionalString(value) ?? null
}

const providerMessageId = (payload: Record<string, unknown>): string | null =>
  optionalString(nestedUnknown(payload, ['data', 'email_id'])) ??
  optionalString(nestedUnknown(payload, ['data', 'id'])) ??
  optionalString(nestedUnknown(payload, ['email_id'])) ??
  null

const recipientEmail = (payload: Record<string, unknown>): string | null =>
  firstEmail(nestedUnknown(payload, ['data', 'to'])) ??
  optionalString(nestedUnknown(payload, ['data', 'email'])) ??
  firstEmail(nestedUnknown(payload, ['to'])) ??
  optionalString(nestedUnknown(payload, ['email'])) ??
  null

const deliveryProjection = (
  eventType: string,
  payload: Record<string, unknown>,
): DeliveryProjection => {
  if (eventType === 'email.delivered') {
    return { errorMessage: null, errorName: null, status: 'accepted' }
  }

  if (
    eventType === 'email.bounced' ||
    eventType === 'email.complained' ||
    eventType === 'email.failed'
  ) {
    return {
      errorMessage:
        compactText(
          optionalString(
            nestedUnknown(payload, ['data', 'error', 'message']),
          ) ??
            optionalString(nestedUnknown(payload, ['data', 'reason'])) ??
            eventType,
          500,
        ) ?? eventType,
      errorName:
        compactText(
          optionalString(nestedUnknown(payload, ['data', 'error', 'name'])) ??
            eventType.replace('email.', 'resend_'),
          120,
        ) ?? 'resend_event',
      status: 'failed',
    }
  }

  return {
    errorMessage: null,
    errorName: null,
    status: 'unknown_external_state',
  }
}

const payloadSummary = (
  input: Readonly<{
    eventType: string
    occurredAt: string | null
    providerMessageId: string | null
    recipient: string | null
  }>,
): string =>
  JSON.stringify({
    eventType: compactText(input.eventType, 120),
    occurredAt: compactText(input.occurredAt, 80),
    providerMessageId: compactText(input.providerMessageId, 160),
    recipient: compactText(input.recipient, 320),
  })

export const handleResendWebhook = async (
  db: D1Database,
  input: Readonly<{
    body: string
    headers: Headers
    secret?: Redacted.Redacted<WorkerSecret> | undefined
  }>,
  runtime: ResendWebhookRuntime = systemResendWebhookRuntime,
): Promise<ResendWebhookResult> => {
  if (
    input.secret !== undefined &&
    !(await verifyResendWebhookSignature({
      body: input.body,
      headers: input.headers,
      secret: input.secret,
    }))
  ) {
    return {
      duplicate: false,
      eventType: 'invalid_signature',
      providerEventId: input.headers.get('svix-id') ?? 'missing_svix_id',
      status: 'unauthorized',
    }
  }

  const payload = parseJsonRecord(input.body)
  const eventType = compactText(optionalString(payload?.type), 120)
  const providerEventId = compactText(
    input.headers.get('svix-id') ?? optionalString(payload?.id),
    160,
  )

  if (payload === undefined || eventType === null || providerEventId === null) {
    return {
      duplicate: false,
      eventType: eventType ?? 'invalid_payload',
      providerEventId: providerEventId ?? 'invalid_payload',
      status: 'invalid',
    }
  }

  const now = runtime.nowIso()
  const messageId = providerMessageId(payload)
  const recipient = recipientEmail(payload)
  const occurredAt =
    optionalString(payload.created_at) ??
    optionalString(nestedUnknown(payload, ['data', 'created_at']))
  const result = await db
    .prepare(
      `INSERT INTO email_provider_events
        (id, provider, provider_event_id, event_type, email,
         email_message_id, provider_message_id, occurred_at,
         payload_summary_json, source_authority_ref, created_at)
       VALUES (?, 'resend', ?, ?, ?, NULL, ?, ?, ?, ?, ?)
       ON CONFLICT(provider, provider_event_id) DO NOTHING`,
    )
    .bind(
      runtime.makeId('email_provider_event'),
      providerEventId,
      eventType,
      recipient,
      messageId,
      occurredAt ?? null,
      payloadSummary({
        eventType,
        occurredAt: occurredAt ?? null,
        providerMessageId: messageId,
        recipient,
      }),
      `resend.webhook:${eventType}`,
      now,
    )
    .run()
  const duplicate =
    typeof (result.meta as { changes?: unknown }).changes === 'number' &&
    (result.meta as { changes: number }).changes === 0
  const projection = deliveryProjection(eventType, payload)

  if (!duplicate && messageId !== null) {
    await db
      .prepare(
        `UPDATE email_deliveries
            SET status = ?,
                error_name = ?,
                error_message = ?,
                completed_at = ?,
                updated_at = ?
          WHERE provider = 'resend'
            AND provider_message_id = ?`,
      )
      .bind(
        projection.status,
        projection.errorName,
        projection.errorMessage,
        now,
        now,
        messageId,
      )
      .run()
  }

  if (
    !duplicate &&
    recipient !== null &&
    (eventType === 'email.bounced' || eventType === 'email.complained')
  ) {
    await recordProviderEmailSuppression(
      db,
      {
        email: recipient,
        providerEventId,
        reason: eventType === 'email.bounced' ? 'bounce' : 'complaint',
        sourceAuthorityRef: `resend.webhook:${eventType}`,
      },
      runtime,
    )
  }

  return {
    duplicate,
    eventType,
    providerEventId,
    status: 'accepted',
  }
}
