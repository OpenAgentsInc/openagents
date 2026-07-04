import { Effect, Match as M } from 'effect'

import {
  BusinessAffiliateAttributionStoreError,
  createBusinessAffiliateCode,
  readBusinessAffiliateAttributionReport,
  systemBusinessAffiliateAttributionRuntime,
  type BusinessAffiliateAttributionRuntime,
} from './business-affiliate-attribution'
import { methodNotAllowed, noStoreJsonResponse, unauthorized } from './http/responses'
import { optionalString, readJsonObject } from './json-boundary'

type HttpResponse = globalThis.Response

export type BusinessAffiliateAttributionRouteOptions = Readonly<{
  nowIso?: (() => string) | undefined
  requireAdminApiToken: (request: Request) => Promise<boolean>
  runtime?: BusinessAffiliateAttributionRuntime | undefined
}>

const routeErrorResponse = (
  error: BusinessAffiliateAttributionStoreError,
): HttpResponse =>
  M.value(error.kind).pipe(
    M.when('not_found', () =>
      noStoreJsonResponse(
        { error: 'business_affiliate_not_found', reason: error.reason },
        { status: 404 },
      ),
    ),
    M.when('validation_error', () =>
      noStoreJsonResponse(
        {
          error: 'business_affiliate_validation_error',
          reason: error.reason,
        },
        { status: 400 },
      ),
    ),
    M.orElse(() =>
      noStoreJsonResponse(
        { error: 'business_affiliate_storage_error', reason: error.reason },
        { status: 500 },
      ),
    ),
  )

const requireOperator = async (
  options: BusinessAffiliateAttributionRouteOptions,
  request: Request,
): Promise<HttpResponse | undefined> =>
  (await options.requireAdminApiToken(request)) ? undefined : unauthorized()

const runtimeFor = (
  options: BusinessAffiliateAttributionRouteOptions,
): BusinessAffiliateAttributionRuntime => ({
  ...(options.runtime ?? systemBusinessAffiliateAttributionRuntime),
  nowIso:
    options.nowIso ??
    options.runtime?.nowIso ??
    systemBusinessAffiliateAttributionRuntime.nowIso,
})

export const handleOperatorBusinessAffiliateCodeApi = (
  request: Request,
  db: D1Database,
  options: BusinessAffiliateAttributionRouteOptions,
) => {
  if (request.method !== 'POST') {
    return Effect.succeed(methodNotAllowed(['POST']))
  }

  return Effect.tryPromise({
    catch: error =>
      error instanceof BusinessAffiliateAttributionStoreError
        ? error
        : new BusinessAffiliateAttributionStoreError({
            kind: 'storage_error',
            reason: error instanceof Error ? error.message : String(error),
          }),
    try: async () => {
      const denial = await requireOperator(options, request)
      if (denial !== undefined) return denial

      const body = await readJsonObject(request)
      const record = await createBusinessAffiliateCode(
        db,
        {
          code: optionalString(body.code) ?? '',
          issuedByRef: optionalString(body.issuedByRef),
          ownerRef: optionalString(body.ownerRef) ?? '',
          policyState: optionalString(body.policyState) as
            | 'active'
            | 'paused'
            | 'archived'
            | undefined,
        },
        runtimeFor(options),
      )

      return noStoreJsonResponse(
        {
          code: record,
          authorityBoundary:
            'Operator-issued attribution code only; no payout, settlement, public earning claim, spend, send, or agent authority.',
        },
        { status: 201 },
      )
    },
  }).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
}

export const handleOperatorBusinessAffiliateAttributionApi = (
  request: Request,
  db: D1Database,
  options: BusinessAffiliateAttributionRouteOptions,
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  return Effect.tryPromise({
    catch: error =>
      error instanceof BusinessAffiliateAttributionStoreError
        ? error
        : new BusinessAffiliateAttributionStoreError({
            kind: 'storage_error',
            reason: error instanceof Error ? error.message : String(error),
          }),
    try: async () => {
      const denial = await requireOperator(options, request)
      if (denial !== undefined) return denial

      const url = new URL(request.url)
      const report = await readBusinessAffiliateAttributionReport(db, {
        code: url.searchParams.get('code') ?? '',
        nowIso: runtimeFor(options).nowIso(),
      })

      return noStoreJsonResponse({ report })
    },
  }).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
}
