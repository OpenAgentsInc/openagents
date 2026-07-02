// Khala Code plan routes (promise khala_code.free_paid_plans.v1, claim issue
// #7966): the public plan catalog, the authed current-plan read, and the
// flag-gated (DEFAULT OFF, fail-closed) paid-plan purchase seam.
//
// The paid plan's substance is the existing capture opt-out: purchase reuses
// grantPaidPrivacyEntitlement (inference-privacy-receipt-routes.ts) so there
// is exactly ONE entitlement truth and ONE dereferenceable receipt surface
// (/api/public/inference/privacy-receipts/{receiptRef}) instead of a parallel
// plan-receipt store. While KHALA_CODE_PAID_PLANS_ENABLED is off (the default,
// and the shipped state), the purchase route fails closed with
// khala_code_paid_plans_not_enabled and grants nothing — the promise blocker
// blocker.product_promises.khala_code_paid_plan_not_purchasable stays until
// an owner arms the seam and a real collected purchase exists.

import { Effect, Schema as S } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from '../http/responses'
import { parseJsonUnknown } from '../json-boundary'
import { liveAtReadStaleness } from '../public-projection-staleness'
import { compactRandomId, currentIsoTimestamp } from '../runtime-primitives'
import {
  KHALA_CODE_FREE_PLAN_ID,
  KHALA_CODE_PAID_PLAN_ID,
  khalaCodePlanCatalog,
} from './khala-code-plan-catalog'
import {
  PAID_PRIVACY_REASON_ACCOUNT_ENTITLEMENT,
  PAID_PRIVACY_REASON_READ_ERROR,
  makePaidPrivacyResolver,
} from './inference-privacy-entitlement'
import { grantPaidPrivacyEntitlement } from './inference-privacy-receipt-routes'

export type KhalaCodePlanSession = Readonly<{ accountRef: string }>

export type KhalaCodePlanRoutesDeps = Readonly<{
  authenticate: (
    request: Request,
  ) => Promise<KhalaCodePlanSession | undefined>
  confidentialComputeEnabled: boolean
  db: D1Database
  nowIso?: (() => string) | undefined
  paidPlanPurchaseArmed: boolean
}>

const PurchaseBody = S.Struct({
  idempotencyKey: S.optionalKey(S.String),
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
// from the privacy-entitlement seam. Honest mapping:
//   - account entitlement row        => paid plan, captureExcluded
//   - confidential-compute mode      => free plan, captureExcluded (the
//     deployment-wide exclusion is not a purchased plan)
//   - no entitlement                 => free plan, capturable
//   - entitlement read error         => 503 (fail-closed: never fabricate a
//     plan the caller did or did not buy)
export const handleKhalaCodePlanStatus = (
  request: Request,
  deps: KhalaCodePlanRoutesDeps,
) =>
  Effect.gen(function* () {
    if (request.method !== 'GET') {
      return noStoreJsonResponse({ error: 'method_not_allowed' }, { status: 405 })
    }

    const session = yield* Effect.promise(() => deps.authenticate(request))
    if (session === undefined) {
      return authResponse()
    }

    const resolver = makePaidPrivacyResolver({
      confidentialComputeEnabled: deps.confidentialComputeEnabled,
      db: deps.db,
    })
    const decision = yield* Effect.promise(() =>
      resolver(session.accountRef),
    )

    if (decision.reasonRef === PAID_PRIVACY_REASON_READ_ERROR) {
      return noStoreJsonResponse(
        { error: 'khala_code_plan_status_unavailable' },
        { status: 503 },
      )
    }

    const paid = decision.reasonRef === PAID_PRIVACY_REASON_ACCOUNT_ENTITLEMENT
    return noStoreJsonResponse({
      ok: true,
      plan: {
        planId: paid ? KHALA_CODE_PAID_PLAN_ID : KHALA_CODE_FREE_PLAN_ID,
        kind: paid ? 'paid' : 'free',
        captureExcluded: decision.enabled,
        reasonRef: decision.reasonRef,
      },
    })
  })

// POST /v1/khala-code/plans/purchases — the paid-plan purchase seam.
// FLAG-GATED, DEFAULT OFF, FAIL-CLOSED: while KHALA_CODE_PAID_PLANS_ENABLED
// is unarmed this returns 503 and grants nothing. When armed, it grants the
// idempotent paid-privacy entitlement (the plan's substance) and returns the
// publicly dereferenceable receipt. It collects NO payment in either state —
// the payment-collection leg is the owner-gated remainder of the promise.
export const handleKhalaCodePlanPurchase = (
  request: Request,
  deps: KhalaCodePlanRoutesDeps,
) =>
  Effect.gen(function* () {
    if (request.method !== 'POST') {
      return noStoreJsonResponse({ error: 'method_not_allowed' }, { status: 405 })
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

    const nowIso = deps.nowIso?.() ?? currentIsoTimestamp()
    const purchaseRef = compactRandomId('khala_code_paid_plan')
    const idempotencyKey =
      boundedIdempotencyKey(body.idempotencyKey) ??
      `khala-code-plan-purchase:${session.accountRef}:${purchaseRef}`
    const row = yield* Effect.tryPromise(() =>
      grantPaidPrivacyEntitlement(deps.db, {
        accountRef: session.accountRef,
        idempotencyKey,
        nowIso,
        purchaseRef,
      }),
    ).pipe(Effect.orDie)

    if (row === null) {
      return noStoreJsonResponse(
        { error: 'khala_code_plan_receipt_not_recorded' },
        { status: 500 },
      )
    }

    return noStoreJsonResponse(
      {
        ok: true,
        planId: KHALA_CODE_PAID_PLAN_ID,
        captureExcluded: true,
        entitlementRef: row.entitlement_ref,
        receiptRef: row.receipt_ref,
        receiptUrl: `/api/public/inference/privacy-receipts/${encodeURIComponent(row.receipt_ref)}`,
      },
      { status: 201 },
    )
  })
