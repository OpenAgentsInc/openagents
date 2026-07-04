// Khala Code plan routes (promise khala_code.free_paid_plans.v1, claim issue
// #7966): the public plan catalog, the authed current-plan read, and the
// flag-gated (DEFAULT OFF, fail-closed) paid-plan purchase seam.
//
// The paid plan's substance is the existing capture opt-out: purchase reuses
// grantPaidPrivacyEntitlement (inference-privacy-receipt-routes.ts) so there
// is exactly ONE entitlement truth and ONE dereferenceable receipt surface
// (/api/public/inference/privacy-receipts/{receiptRef}) instead of a parallel
// plan-receipt store. While KHALA_CODE_PAID_PLANS_ENABLED is off (the
// default), the purchase route fails closed with
// khala_code_paid_plans_not_enabled and grants nothing. When armed, it first
// creates a real payment requirement (Stripe Checkout or Spark/MPP Lightning)
// and grants the entitlement only after the payment settles.

import { Effect, Schema as S } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from '../http/responses'
import { parseJsonUnknown } from '../json-boundary'
import { liveAtReadStaleness } from '../public-projection-staleness'
import { currentIsoTimestamp } from '../runtime-primitives'
import {
  KHALA_CODE_FREE_PLAN_ID,
  KHALA_CODE_PAID_PLAN_ID,
  khalaCodePlanCatalog,
} from './khala-code-plan-catalog'
import type { BillingDomainMirror } from '../billing'
import {
  fulfillKhalaCodePaidPlanPaymentIntent,
  lightningPaymentRequestFromIntent,
  makeKhalaCodePaidPlanPurchaseRef,
  readKhalaCodePaidPlanIntentByIdempotencyKey,
  readKhalaCodePaidPlanIntentByLightningPaymentHash,
  recordKhalaCodePaidPlanLightningIntent,
  type KhalaCodePaidPlanStripeCheckout,
} from './khala-code-paid-plan-payments'
import {
  PAID_PRIVACY_REASON_CONFIDENTIAL_COMPUTE,
  PAID_PRIVACY_REASON_NONE,
  PAID_PRIVACY_REASON_READ_ERROR,
  readAccountPaidPrivacy,
} from './inference-privacy-entitlement'
import type { MintLightningInvoice } from './mpp/mpp-lightning-invoice'
import { readPreimage, verifyLightningPreimage } from './mpp/mpp-lightning-verify'

export type KhalaCodePlanSession = Readonly<{ accountRef: string }>

export type KhalaCodePlanRoutesDeps = Readonly<{
  authenticate: (
    request: Request,
  ) => Promise<KhalaCodePlanSession | undefined>
  confidentialComputeEnabled: boolean
  createStripePaidPlanCheckout?: (input: {
    accountRef: string
    db: D1Database
    idempotencyKey: string
    nowIso: string
    purchaseRef: string
  }) => Promise<KhalaCodePaidPlanStripeCheckout>
  db: D1Database
  mintLightningInvoice?: MintLightningInvoice | undefined
  /** KS-8.7 (#8318) fail-soft Postgres mirror (billing-store.ts). */
  mirror?: BillingDomainMirror | undefined
  nowIso?: (() => string) | undefined
  paidPlanPurchaseArmed: boolean
  paidPlanPriceSats?: number | undefined
}>

const PurchaseRail = S.Literals(['stripe_checkout', 'lightning_mpp'])

const PurchaseBody = S.Struct({
  idempotencyKey: S.optionalKey(S.String),
  lightningPaymentHash: S.optionalKey(S.String),
  preimage: S.optionalKey(S.String),
  rail: S.optionalKey(PurchaseRail),
})

const safeJsonParse = (text: string): unknown => {
  try {
    return parseJsonUnknown(text)
  } catch {
    return null
  }
}

const boundedIdempotencyKey = (
  value: string | undefined,
): string | undefined => {
  const trimmed = value?.trim()
  return trimmed !== undefined &&
    trimmed !== '' &&
    /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,127}$/.test(trimmed)
    ? trimmed
    : undefined
}

const decodePurchaseBody = (value: unknown) => {
  try {
    return S.decodeUnknownSync(PurchaseBody)(value)
  } catch {
    return undefined
  }
}

export const readKhalaCodePaidPlanPriceSats = (
  value: unknown,
): number | undefined => {
  if (typeof value !== 'string') {
    return undefined
  }
  const parsed = Number.parseInt(value.trim(), 10)
  return Number.isFinite(parsed) && parsed > 0 ? parsed : undefined
}

const readBody = (request: Request) =>
  Effect.promise(() => request.text().catch(() => ''))

const authResponse = () => {
  const headers = new Headers({ 'www-authenticate': 'Bearer' })
  return noStoreJsonResponse({ error: 'unauthorized' }, { headers, status: 401 })
}

// The catalog is static text plus one deployment-config input (the purchase
// flag), recomputed on every read — live_at_read with no data tables behind
// it, only the catalog module and the fail-closed flag read.
const planCatalogStaleness = liveAtReadStaleness([
  'module:khala-code-plan-catalog.ts',
  'env:KHALA_CODE_PAID_PLANS_ENABLED',
])

// GET /api/public/khala-code/plans — the public, agent-readable plan catalog.
// Read-only, no auth, no DB, no secrets; purchasability reflects the real
// fail-closed flag state instead of hardcoded copy.
export const handleKhalaCodePlanCatalogApi = (
  request: Request,
  deps: Readonly<{
    nowIso?: (() => string) | undefined
    paidPlanPurchaseArmed: boolean
  }>,
) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.succeed(
        noStoreJsonResponse({
          catalog: khalaCodePlanCatalog({
            generatedAt: deps.nowIso?.() ?? currentIsoTimestamp(),
            paidPlanPurchaseArmed: deps.paidPlanPurchaseArmed,
            staleness: planCatalogStaleness,
          }),
        }),
      )

// GET /v1/khala-code/plan — the caller's current plan, resolved SERVER-SIDE
// from the privacy-entitlement seam. Honest mapping: the per-account
// entitlement row is ALWAYS read (deployment-wide confidential-compute mode
// must not hide a purchased plan), and:
//   - account entitlement row        => paid plan, captureExcluded
//   - confidential-compute mode only => free plan, captureExcluded (the
//     deployment-wide exclusion is not a purchased plan)
//   - neither                        => free plan, capturable
//   - entitlement read error         => 503 (fail-closed: never fabricate a
//     plan the caller did or did not buy)
export const handleKhalaCodePlanStatus = (
  request: Request,
  deps: KhalaCodePlanRoutesDeps,
) =>
  Effect.gen(function* () {
    if (request.method !== 'GET') {
      return methodNotAllowed(['GET'])
    }

    const session = yield* Effect.promise(() => deps.authenticate(request))
    if (session === undefined) {
      return authResponse()
    }

    const decision = yield* Effect.promise(() =>
      readAccountPaidPrivacy(deps.db, session.accountRef),
    )

    if (decision.reasonRef === PAID_PRIVACY_REASON_READ_ERROR) {
      return noStoreJsonResponse(
        { error: 'khala_code_plan_status_unavailable' },
        { status: 503 },
      )
    }

    const paid = decision.enabled
    const captureExcluded = paid || deps.confidentialComputeEnabled
    return noStoreJsonResponse({
      ok: true,
      plan: {
        planId: paid ? KHALA_CODE_PAID_PLAN_ID : KHALA_CODE_FREE_PLAN_ID,
        kind: paid ? 'paid' : 'free',
        captureExcluded,
        reasonRef: paid
          ? decision.reasonRef
          : deps.confidentialComputeEnabled
            ? PAID_PRIVACY_REASON_CONFIDENTIAL_COMPUTE
            : PAID_PRIVACY_REASON_NONE,
      },
    })
  })

// POST /v1/khala-code/plans/purchases — the paid-plan purchase seam.
// FLAG-GATED, DEFAULT OFF, FAIL-CLOSED: while KHALA_CODE_PAID_PLANS_ENABLED
// is unarmed this returns 503 and grants nothing. When armed, it returns a
// payment-required object for the selected rail. Only a completed Stripe
// Checkout webhook or a locally verified Lightning preimage fulfills the
// existing paid-privacy entitlement receipt.
export const handleKhalaCodePlanPurchase = (
  request: Request,
  deps: KhalaCodePlanRoutesDeps,
) =>
  Effect.gen(function* () {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    if (!deps.paidPlanPurchaseArmed) {
      return noStoreJsonResponse(
        { error: 'khala_code_paid_plans_not_enabled' },
        { status: 503 },
      )
    }

    const session = yield* Effect.promise(() => deps.authenticate(request))
    if (session === undefined) {
      return authResponse()
    }

    const text = yield* readBody(request)
    const parsed = text === '' ? {} : safeJsonParse(text)
    if (parsed === null) {
      return noStoreJsonResponse({ error: 'invalid_json' }, { status: 400 })
    }

    const body = decodePurchaseBody(parsed)
    if (body === undefined) {
      return noStoreJsonResponse(
        { error: 'invalid_request_schema' },
        { status: 400 },
      )
    }

    // A supplied-but-invalid key is REJECTED, never silently replaced: a
    // silent fallback would make retries with the same (invalid) key mint
    // duplicate purchase receipts with no signal to the client.
    const clientKey = boundedIdempotencyKey(body.idempotencyKey)
    if (body.idempotencyKey !== undefined && clientKey === undefined) {
      return noStoreJsonResponse(
        { error: 'invalid_idempotency_key' },
        { status: 400 },
      )
    }

    const rail =
      body.rail ??
      (body.lightningPaymentHash !== undefined || body.preimage !== undefined
        ? 'lightning_mpp'
        : 'stripe_checkout')
    const nowIso = deps.nowIso?.() ?? currentIsoTimestamp()
    const purchaseRef = makeKhalaCodePaidPlanPurchaseRef()
    // The effective key is ALWAYS namespaced by route, account, and rail: the
    // idempotency_key column is globally unique across purchase surfaces, so a
    // raw client key could collide with another account or rail. Namespacing
    // confines every client key to its own account and selected payment rail.
    const idempotencyKey = `khala-code-plan-purchase:${session.accountRef}:${rail}:${clientKey ?? purchaseRef}`

    if (rail === 'stripe_checkout') {
      if (
        body.lightningPaymentHash !== undefined ||
        body.preimage !== undefined
      ) {
        return noStoreJsonResponse(
          { error: 'invalid_request_schema' },
          { status: 400 },
        )
      }
      if (deps.createStripePaidPlanCheckout === undefined) {
        return noStoreJsonResponse(
          { error: 'khala_code_paid_plan_payment_rail_unavailable' },
          { status: 503 },
        )
      }

      const checkout = yield* Effect.tryPromise(() =>
        deps.createStripePaidPlanCheckout!({
          accountRef: session.accountRef,
          db: deps.db,
          idempotencyKey,
          nowIso,
          purchaseRef,
        }),
      ).pipe(Effect.catch(() => Effect.succeed(null)))

      return checkout === null
        ? noStoreJsonResponse(
            { error: 'khala_code_paid_plan_payment_rail_unavailable' },
            { status: 503 },
          )
        : noStoreJsonResponse(checkout, { status: 202 })
    }

    if (
      body.lightningPaymentHash !== undefined ||
      body.preimage !== undefined
    ) {
      const paymentHash = body.lightningPaymentHash?.trim().toLowerCase()
      const preimage = readPreimage({ preimage: body.preimage })
      if (
        paymentHash === undefined ||
        paymentHash === '' ||
        preimage === undefined
      ) {
        return noStoreJsonResponse(
          { error: 'khala_code_paid_plan_lightning_proof_required' },
          { status: 400 },
        )
      }

      const intent = yield* Effect.tryPromise(() =>
        readKhalaCodePaidPlanIntentByLightningPaymentHash(deps.db, paymentHash),
      ).pipe(Effect.catch(() => Effect.succeed(null)))

      if (intent === null || intent.accountRef !== session.accountRef) {
        return noStoreJsonResponse(
          { error: 'khala_code_paid_plan_payment_not_found' },
          { status: 404 },
        )
      }

      const verified = yield* Effect.promise(() =>
        verifyLightningPreimage(preimage, paymentHash),
      )
      if (!verified.ok) {
        return noStoreJsonResponse(
          { error: 'khala_code_paid_plan_lightning_preimage_invalid' },
          { status: 402 },
        )
      }

      const fulfilled = yield* Effect.tryPromise(() =>
        fulfillKhalaCodePaidPlanPaymentIntent(
          deps.db,
          { intent, nowIso },
          deps.mirror,
        ),
      ).pipe(Effect.catch(() => Effect.succeed(null)))

      return fulfilled === null
        ? noStoreJsonResponse(
            { error: 'khala_code_plan_receipt_not_recorded' },
            { status: 500 },
          )
        : noStoreJsonResponse(fulfilled, { status: 201 })
    }

    if (
      deps.mintLightningInvoice === undefined ||
      deps.paidPlanPriceSats === undefined ||
      deps.paidPlanPriceSats <= 0
    ) {
      return noStoreJsonResponse(
        { error: 'khala_code_paid_plan_payment_rail_unavailable' },
        { status: 503 },
      )
    }

    const existing = yield* Effect.tryPromise(() =>
      readKhalaCodePaidPlanIntentByIdempotencyKey(deps.db, {
        accountRef: session.accountRef,
        idempotencyKey,
      }),
    ).pipe(Effect.catch(() => Effect.succeed(null)))

    if (existing !== null) {
      if (existing.status === 'fulfilled') {
        const fulfilled = yield* Effect.tryPromise(() =>
          fulfillKhalaCodePaidPlanPaymentIntent(
            deps.db,
            {
              intent: existing,
              nowIso,
            },
            deps.mirror,
          ),
        ).pipe(Effect.catch(() => Effect.succeed(null)))

        return fulfilled === null
          ? noStoreJsonResponse(
              { error: 'khala_code_plan_receipt_not_recorded' },
              { status: 500 },
            )
          : noStoreJsonResponse(fulfilled, { status: 200 })
      }

      return noStoreJsonResponse(lightningPaymentRequestFromIntent(existing), {
        status: 202,
      })
    }

    const invoice = yield* deps
      .mintLightningInvoice({
        amountSats: deps.paidPlanPriceSats,
        correlationRef: purchaseRef,
        description: 'Khala Code paid plan',
      })
      .pipe(Effect.catch(() => Effect.succeed(null)))

    if (invoice === null) {
      return noStoreJsonResponse(
        { error: 'khala_code_paid_plan_payment_rail_unavailable' },
        { status: 503 },
      )
    }

    const intent = yield* Effect.tryPromise(() =>
      recordKhalaCodePaidPlanLightningIntent(
        deps.db,
        {
          accountRef: session.accountRef,
          amountSats: deps.paidPlanPriceSats!,
          idempotencyKey,
          invoice,
          nowIso,
          purchaseRef,
        },
        deps.mirror,
      ),
    ).pipe(Effect.catch(() => Effect.succeed(null)))

    return intent === null
      ? noStoreJsonResponse(
          { error: 'khala_code_paid_plan_payment_intent_not_recorded' },
          { status: 500 },
        )
      : noStoreJsonResponse(lightningPaymentRequestFromIntent(intent), {
          status: 202,
        })
  })
