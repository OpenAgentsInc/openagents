// RevenueCat webhook verification + parsing (MM-E2, #8482).
//
// PIN: #8481 (RevenueCat client/account) is HELD — no live RevenueCat
// project, webhook secret, or real sandbox event exists yet. This module is
// built against RevenueCat's DOCUMENTED webhook shape (their "Webhooks"
// event reference: a top-level `{ api_version, event }` envelope, event
// `type` naming the lifecycle transition, `app_user_id`/`product_id`/
// `store`/`transaction_id`/`original_transaction_id`/`id` as the fields this
// rail needs) and their documented "Authorization header" webhook auth
// mechanism (a static shared secret configured in the RevenueCat dashboard,
// sent back verbatim as the request's `Authorization` header on every call —
// simpler than HMAC body-signing, and what RevenueCat's dashboard actually
// offers). Tests use fixture payloads shaped to this reference; a real
// sandbox event should be diffed against this parser once #8481 lands a live
// project, per the lane's "live sandbox verification waits for E1" note.

const encoder = new TextEncoder()

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

/** Verifies the `Authorization` request header against the configured
 * RevenueCat webhook secret. `undefined` secret (not yet configured, e.g.
 * before #8481 lands one) always fails closed. */
export const verifyRevenueCatWebhookAuth = (
  request: Request,
  configuredSecret: string | undefined,
): boolean => {
  if (configuredSecret === undefined || configuredSecret.trim().length === 0) {
    return false
  }

  const header = request.headers.get('authorization')
  if (header === null) {
    return false
  }

  // RevenueCat sends the configured secret verbatim as the Authorization
  // header value (their docs show plain values, optionally already
  // "Bearer "-prefixed depending on how the dashboard field is filled in) —
  // accept either form so a dashboard operator's exact string doesn't have
  // to guess a convention.
  const bearerPrefix = 'Bearer '
  const presented = header.startsWith(bearerPrefix)
    ? header.slice(bearerPrefix.length)
    : header

  return timingSafeEqual(presented, configuredSecret)
}

export type RevenueCatStore = 'app_store' | 'play_store' | 'other'

const storeFromRevenueCatValue = (value: unknown): RevenueCatStore => {
  if (typeof value !== 'string') return 'other'
  const normalized = value.toUpperCase()
  if (normalized === 'APP_STORE') return 'app_store'
  if (normalized === 'PLAY_STORE') return 'play_store'
  return 'other'
}

export type RevenueCatEventKind =
  | 'purchase'
  | 'refund'
  | 'ignored'

const PURCHASE_EVENT_TYPES: ReadonlySet<string> = new Set([
  'INITIAL_PURCHASE',
  'NON_RENEWING_PURCHASE',
])
const REFUND_EVENT_TYPES: ReadonlySet<string> = new Set([
  'REFUND',
  'CANCELLATION',
])

const kindFromRevenueCatType = (type: unknown): RevenueCatEventKind => {
  if (typeof type !== 'string') return 'ignored'
  if (PURCHASE_EVENT_TYPES.has(type)) return 'purchase'
  if (REFUND_EVENT_TYPES.has(type)) return 'refund'
  return 'ignored'
}

export type ParsedRevenueCatEvent = Readonly<{
  kind: RevenueCatEventKind
  eventId: string
  rawType: string
  appUserId: string
  productId: string
  store: RevenueCatStore
  transactionId: string
  originalTransactionId: string
  environment: 'sandbox' | 'production'
}>

/** Defensive parse: extracts only the fields this rail needs from
 * RevenueCat's `{ api_version, event }` envelope. Returns `undefined` for a
 * payload missing any REQUIRED field (never throws on unexpected shape —
 * webhooks must ack malformed-but-authenticated bodies with a typed
 * rejection, not a 500). */
export const parseRevenueCatWebhookBody = (
  body: unknown,
): ParsedRevenueCatEvent | undefined => {
  if (typeof body !== 'object' || body === null) return undefined
  const event = (body as { event?: unknown }).event
  if (typeof event !== 'object' || event === null) return undefined

  const record = event as Record<string, unknown>
  const eventId = typeof record.id === 'string' ? record.id : undefined
  const appUserId =
    typeof record.app_user_id === 'string' ? record.app_user_id : undefined
  const productId =
    typeof record.product_id === 'string' ? record.product_id : undefined
  const transactionId =
    typeof record.transaction_id === 'string' ? record.transaction_id : undefined

  if (
    eventId === undefined ||
    appUserId === undefined ||
    productId === undefined ||
    transactionId === undefined
  ) {
    return undefined
  }

  const originalTransactionId =
    typeof record.original_transaction_id === 'string'
      ? record.original_transaction_id
      : transactionId
  const environment =
    typeof record.environment === 'string' && record.environment.toUpperCase() === 'PRODUCTION'
      ? ('production' as const)
      : ('sandbox' as const)

  return {
    appUserId,
    environment,
    eventId,
    kind: kindFromRevenueCatType(record.type),
    originalTransactionId,
    productId,
    rawType: typeof record.type === 'string' ? record.type : 'UNKNOWN',
    store: storeFromRevenueCatValue(record.store),
    transactionId,
  }
}
