// `POST /api/webhooks/revenuecat` — RevenueCat webhook ingestion (MM-E2,
// #8482). Verifies the shared "Authorization header" secret, claims the
// event id against the replay guard, then either fulfills a purchase
// (crediting Pool B via the SKU catalog's server-owned amount — never the
// payload's own price) or claws back a refund. See
// `inference/iap-revenuecat-webhook.ts`'s module doc for the #8481 pin.

import { Effect } from 'effect'

import { noStoreJsonResponse, methodNotAllowed, unauthorized } from './http/responses'
import { readJsonObject } from './json-boundary'
import { iapCreditPackFromSku } from './inference/iap-credit-pack-catalog'
import {
  claimIapWebhookEvent,
  fulfillIapCreditPackPurchase,
  refundIapCreditPackPurchase,
} from './inference/iap-credit-pack-payments'
import {
  parseRevenueCatWebhookBody,
  verifyRevenueCatWebhookAuth,
} from './inference/iap-revenuecat-webhook'
import { currentIsoTimestamp } from './runtime-primitives'

type HttpResponse = globalThis.Response

export const IAP_REVENUECAT_WEBHOOK_PATH = '/api/webhooks/revenuecat'

export type IapWebhookRouteDependencies<Bindings> = Readonly<{
  db: (env: Bindings) => D1Database
  webhookSecret: (env: Bindings) => string | undefined
}>

const routeRevenueCatWebhook = async <Bindings>(
  dependencies: IapWebhookRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Promise<HttpResponse> => {
  if (!verifyRevenueCatWebhookAuth(request, dependencies.webhookSecret(env))) {
    return unauthorized()
  }

  let body: unknown
  try {
    body = await readJsonObject(request)
  } catch {
    return noStoreJsonResponse({ error: 'invalid_request' }, { status: 400 })
  }

  const event = parseRevenueCatWebhookBody(body)
  if (event === undefined) {
    return noStoreJsonResponse(
      { error: 'invalid_request', reason: 'unrecognized RevenueCat webhook payload shape' },
      { status: 400 },
    )
  }

  const db = dependencies.db(env)
  const nowIso = currentIsoTimestamp()

  const claim = await claimIapWebhookEvent(db, {
    eventId: event.eventId,
    eventType: event.rawType,
    nowIso,
  })
  if (!claim.firstDelivery) {
    return noStoreJsonResponse({ action: 'ignored', ok: true, reason: 'duplicate_event_id' })
  }

  if (event.kind === 'ignored') {
    return noStoreJsonResponse({ action: 'ignored', ok: true, reason: 'unhandled_event_type' })
  }

  if (event.kind === 'purchase') {
    if (event.store !== 'app_store' && event.store !== 'play_store') {
      return noStoreJsonResponse({ action: 'ignored', ok: true, reason: 'unsupported_store' })
    }

    const pack = iapCreditPackFromSku(event.productId)
    if (pack === undefined) {
      return noStoreJsonResponse({ action: 'ignored', ok: true, reason: 'sku_not_in_catalog' })
    }

    const outcome = await Effect.runPromise(
      fulfillIapCreditPackPurchase(db, {
        amountUsdCents: pack.amountUsdCents,
        eventId: event.eventId,
        sku: event.productId,
        store: event.store,
        storeTransactionId: event.transactionId,
        userId: event.appUserId,
      }),
    )

    if (!outcome.ok) {
      return noStoreJsonResponse({ action: 'refused', ok: false, reason: outcome.reason }, { status: 422 })
    }

    return noStoreJsonResponse({
      action: outcome.alreadyFulfilled ? 'already_fulfilled' : 'fulfilled',
      ok: true,
      purchaseRef: outcome.purchase.purchaseRef,
    })
  }

  // event.kind === 'refund'
  const outcome = await Effect.runPromise(
    refundIapCreditPackPurchase(db, event.originalTransactionId),
  )

  if (!outcome.ok) {
    // The refunded purchase was never seen (e.g. it wasn't a credit-pack SKU
    // to begin with) — nothing to claw back, ack anyway so RevenueCat
    // doesn't retry forever for a purchase this rail never fulfilled.
    return noStoreJsonResponse({ action: 'ignored', ok: true, reason: 'purchase_not_found' })
  }

  return noStoreJsonResponse({
    action: outcome.alreadyRefunded ? 'already_refunded' : 'refunded',
    clawedBack: outcome.clawback.clawedBack,
    insufficientBalance: outcome.clawback.insufficientBalance,
    ok: true,
  })
}

export const handleIapRevenueCatWebhookRequest = <Bindings>(
  dependencies: IapWebhookRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<HttpResponse> =>
  Effect.promise(async () => {
    if (request.method !== 'POST') return methodNotAllowed(['POST'])
    return routeRevenueCatWebhook(dependencies, request, env)
  })
