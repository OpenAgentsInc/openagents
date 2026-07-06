// Server-side push sender via the Expo push service (MM-G2, #8486). Direct
// APNs/FCM is a documented later hardening path per the issue text — this is
// the Expo-hosted relay, the standard integration for an Expo-managed app.
//
// HONEST GAP: Expo's push API is two-phase — this immediately sends and
// reads the synchronous "ticket" response (which DOES already surface
// `DeviceNotRegistered` for tokens Expo can reject up front), but does NOT
// implement the async receipts step (`/getReceipts`, polled ~15+ minutes
// later for delivery-time failures APNs/FCM only report after the fact).
// That deeper receipt-polling loop is a real follow-up, not done here —
// token invalidation from the immediate ticket response IS handled (see
// `pruneInvalidatedTokens` below).

import { removePushDeviceTokensByExpoToken } from './push-device-tokens'
import type { PushNotificationPayload } from './push-notify-events'

export const EXPO_PUSH_SEND_URL = 'https://exp.host/--/api/v2/push/send'
/** Expo's own documented per-request cap. */
export const EXPO_PUSH_MAX_MESSAGES_PER_REQUEST = 100

export type ExpoPushMessage = Readonly<{
  to: string
  title: string
  body: string
  data: Record<string, unknown>
}>

export const buildExpoPushMessage = (
  expoPushToken: string,
  payload: PushNotificationPayload,
): ExpoPushMessage => ({
  body: payload.body,
  data: payload.data,
  title: payload.title,
  to: expoPushToken,
})

/** Expo's own ticket-response shape (subset actually consumed here). */
export type ExpoPushTicket =
  | Readonly<{ status: 'ok'; id: string }>
  | Readonly<{
      status: 'error'
      message: string
      details?: Readonly<{ error?: string }>
    }>

export type ExpoPushSendResponse = Readonly<{ data: ReadonlyArray<ExpoPushTicket> }>

export type FetchLike = (url: string, init: RequestInit) => Promise<Response>

/** Splits into batches no larger than Expo's per-request cap. Exported so
 * callers/tests can verify batching independent of the network call. */
export const chunkExpoPushMessages = (
  messages: ReadonlyArray<ExpoPushMessage>,
  size: number = EXPO_PUSH_MAX_MESSAGES_PER_REQUEST,
): ReadonlyArray<ReadonlyArray<ExpoPushMessage>> => {
  const chunks: Array<ReadonlyArray<ExpoPushMessage>> = []
  for (let i = 0; i < messages.length; i += size) {
    chunks.push(messages.slice(i, i + size))
  }
  return chunks
}

export type SendExpoPushMessagesResult = Readonly<{
  /** Tickets in the SAME order as the flattened input messages. */
  tickets: ReadonlyArray<ExpoPushTicket>
  /** Expo push tokens pruned from the registry because their ticket already
   * reported `DeviceNotRegistered`. */
  invalidatedTokens: ReadonlyArray<string>
}>

const sendOneBatch = async (
  fetchImpl: FetchLike,
  batch: ReadonlyArray<ExpoPushMessage>,
): Promise<ReadonlyArray<ExpoPushTicket>> => {
  if (batch.length === 0) return []

  const response = await fetchImpl(EXPO_PUSH_SEND_URL, {
    body: JSON.stringify(batch),
    headers: {
      accept: 'application/json',
      'content-type': 'application/json',
    },
    method: 'POST',
  })

  if (!response.ok) {
    // Whole-batch transport failure: every message in this batch is treated
    // as an (unprunable — we don't know WHICH token, if any, was at fault)
    // error ticket so the caller's summary is honest about a send failure.
    return batch.map(() => ({
      details: { error: 'transport_error' },
      message: `Expo push send failed with HTTP ${response.status}`,
      status: 'error' as const,
    }))
  }

  const body = (await response.json()) as ExpoPushSendResponse
  return body.data
}

/**
 * Sends every message (batched to Expo's cap), then prunes any device token
 * whose ticket already reports `DeviceNotRegistered` (the immediate-rejection
 * case; the deeper async receipts step is the documented follow-up gap
 * above). Never throws for a partial/whole batch failure — every outcome is
 * captured as an error ticket in the returned array so the caller can build
 * an honest per-recipient summary.
 */
export const sendExpoPushMessages = async (
  db: D1Database,
  messages: ReadonlyArray<ExpoPushMessage>,
  fetchImpl: FetchLike = fetch,
): Promise<SendExpoPushMessagesResult> => {
  const batches = chunkExpoPushMessages(messages)
  const tickets: Array<ExpoPushTicket> = []

  for (const batch of batches) {
    tickets.push(...(await sendOneBatch(fetchImpl, batch)))
  }

  const invalidatedTokens: Array<string> = []
  for (const [index, ticket] of tickets.entries()) {
    if (ticket.status === 'error' && ticket.details?.error === 'DeviceNotRegistered') {
      const token = messages[index]?.to
      if (token !== undefined) {
        await removePushDeviceTokensByExpoToken(db, token)
        invalidatedTokens.push(token)
      }
    }
  }

  return { invalidatedTokens, tickets }
}
