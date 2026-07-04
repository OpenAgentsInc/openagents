// Khala Sync public tokens-served reconcile/repair route (KS-6.3, #8304).
//
// Admin-bearer-gated internal route enforcing SPEC §7 invariant 8: the
// `scope.public.tokens-served` projection must reconcile to exact
// `token_usage_events` rows.
//
//   GET  — reconcile only: recompute the exact D1 SUM, read the projection,
//          report `{ exactTokensServed, projectedTokensServed, driftTokens,
//          inSync }`. NEVER writes.
//   POST — explicit repair: body `{ repair: true, auditNote? }` sets the
//          projection to the exact SUM with an audited
//          `khala_sync_public_counter_repairs` row. Against an
//          uninitialized counter this IS the first-deploy backfill
//          (`repairSource: "backfill"`). A POST without `repair: true` is a
//          400 — drift is never overwritten implicitly.
//
// Drift is additionally logged as the typed
// `khala_sync_tokens_served_projection_drift` diagnostic by the shared
// reconcile helper (the same one the scheduled sweep uses).
//
// AUTH: same admin bearer mechanism as the other internal khala-sync routes
// (`requireAdminApiToken`, injected as `requireOperator`). Not part of the
// public OpenAPI surface. Responses are no-store and carry only aggregate
// integers/refs — never connection details or per-user material.

import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  reconcileTokensServedProjection,
  type TokensServedReconcileDeps,
} from './khala-sync-public-tokens-served'

type HttpResponse = globalThis.Response

export const KHALA_SYNC_TOKENS_SERVED_RECONCILE_PATH =
  '/api/internal/khala-sync/public-counters/tokens-served/reconcile'
export const KHALA_SYNC_TOKENS_SERVED_RECONCILE_ROUTE_REF =
  'route.internal.khala_sync.public_counters.tokens_served_reconcile.v0_1'

export type KhalaSyncTokensServedReconcileRouteDeps = Readonly<{
  /** Same admin bearer predicate the other internal khala-sync routes use. */
  requireOperator: () => Promise<boolean>
  reconcileDeps: TokensServedReconcileDeps
}>

const reconcileResultStatus: Record<string, number> = {
  exact_read_failed: 503,
  no_binding: 503,
  projection_read_failed: 503,
  repair_failed: 503,
}

/**
 * `GET|POST /api/internal/khala-sync/public-counters/tokens-served/reconcile`
 * — admin bearer only. GET reconciles (read-only); POST with
 * `{ repair: true }` repairs/backfills with an audit note.
 */
export const handleKhalaSyncTokensServedReconcile = (
  request: Request,
  deps: KhalaSyncTokensServedReconcileRouteDeps,
): Effect.Effect<HttpResponse> =>
  Effect.promise(async () => {
    if (request.method !== 'GET' && request.method !== 'POST') {
      return methodNotAllowed(['GET', 'POST'])
    }

    if (!(await deps.requireOperator())) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    let repair = false
    let auditNote: string | undefined
    if (request.method === 'POST') {
      let body: unknown
      try {
        body = await request.json()
      } catch {
        return noStoreJsonResponse(
          {
            error: 'invalid_request',
            reason: 'POST requires a JSON body: { repair: true, auditNote? }.',
            routeRef: KHALA_SYNC_TOKENS_SERVED_RECONCILE_ROUTE_REF,
          },
          { status: 400 },
        )
      }
      const parsed = body as { repair?: unknown; auditNote?: unknown }
      if (parsed?.repair !== true) {
        // A repair is an explicit, audited decision — never implied.
        return noStoreJsonResponse(
          {
            error: 'invalid_request',
            reason:
              'Repair must be explicit: POST { repair: true, auditNote? }. ' +
              'Use GET for a read-only reconcile.',
            routeRef: KHALA_SYNC_TOKENS_SERVED_RECONCILE_ROUTE_REF,
          },
          { status: 400 },
        )
      }
      repair = true
      auditNote =
        typeof parsed.auditNote === 'string' ? parsed.auditNote : undefined
    }

    const result = await reconcileTokensServedProjection(deps.reconcileDeps, {
      auditNote,
      repair,
    })

    if (!result.ok) {
      return noStoreJsonResponse(
        {
          error: 'khala_sync_tokens_served_reconcile_failed',
          ok: false,
          reason: result.reason,
          messageSafe: result.messageSafe,
          routeRef: KHALA_SYNC_TOKENS_SERVED_RECONCILE_ROUTE_REF,
        },
        { status: reconcileResultStatus[result.reason] ?? 500 },
      )
    }

    return noStoreJsonResponse({
      ok: true,
      report: result.report,
      routeRef: KHALA_SYNC_TOKENS_SERVED_RECONCILE_ROUTE_REF,
    })
  })
