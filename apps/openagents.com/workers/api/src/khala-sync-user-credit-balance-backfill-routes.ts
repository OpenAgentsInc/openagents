// Khala Sync per-user credit-balance backfill route (issue #8505, Part 2).
//
// Admin-bearer-gated internal route: pages through every human user and
// seeds/reconciles their `scope.user.<userId>` `credit_balance` projection
// against the exact current `agent_balances` balance on the CFG-4 (#8519)
// Postgres-authoritative credits ledger (`PaymentsLedgerDb`). Same discipline as
// `khala-sync-public-counter-reconcile-routes.ts` (KS-6.3): a repair is never
// implicit — this route only runs on an explicit admin-bearer POST — and
// every overwrite is audited (`khala_sync_user_credit_balance_repairs`).
//
//   POST { limit?, cursor? } — backfills/reconciles one bounded page (human
//     users ordered by user id), returns
//     { processedCount, backfilledCount, reconciledCount, unchangedCount,
//       failedCount, nextCursor }. Repeat with `nextCursor` until it is
//     `null` to cover every user. A per-user failure never aborts the page
//     (see `backfillUserCreditBalancesBatch`'s fail-soft loop) — the report's
//     `failedCount` surfaces it for a retry.
//
// AUTH: same admin bearer mechanism as the other internal khala-sync routes
// (`requireAdminApiToken`, injected as `requireOperator`). Not part of the
// public OpenAPI surface. Responses are no-store and carry only aggregate
// integers/refs and bounded user ids — never connection details or balance
// amounts for any individual user.

import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { isRecord, parseJsonUnknown } from './json-boundary'
import {
  backfillUserCreditBalancesBatch,
  type UserCreditBalanceBackfillDeps,
} from './khala-sync-user-credit-balance'

type HttpResponse = globalThis.Response

export const KHALA_SYNC_USER_CREDIT_BALANCE_BACKFILL_PATH =
  '/api/internal/khala-sync/user-credit-balances/backfill'
export const KHALA_SYNC_USER_CREDIT_BALANCE_BACKFILL_ROUTE_REF =
  'route.internal.khala_sync.user_credit_balances_backfill.v0_1'

const DEFAULT_BACKFILL_LIMIT = 200
const MAX_BACKFILL_LIMIT = 1_000

export type KhalaSyncUserCreditBalanceBackfillRouteDeps = Readonly<{
  requireOperator: () => Promise<boolean>
  backfillDeps: UserCreditBalanceBackfillDeps
}>

/**
 * `POST /api/internal/khala-sync/user-credit-balances/backfill` — admin
 * bearer only. Backfills/reconciles one bounded page; repeat with the
 * returned `nextCursor` until it is `null`.
 */
export const handleKhalaSyncUserCreditBalanceBackfill = (
  request: Request,
  deps: KhalaSyncUserCreditBalanceBackfillRouteDeps,
): Effect.Effect<HttpResponse> =>
  Effect.promise(async () => {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    if (!(await deps.requireOperator())) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    let body: Record<string, unknown> = {}
    try {
      const text = await request.text()
      if (text.trim().length > 0) {
        const parsed = parseJsonUnknown(text)
        body = isRecord(parsed) ? parsed : {}
      }
    } catch {
      return noStoreJsonResponse(
        {
          error: 'invalid_request',
          reason: 'body must be valid JSON: { limit?, cursor? }.',
          routeRef: KHALA_SYNC_USER_CREDIT_BALANCE_BACKFILL_ROUTE_REF,
        },
        { status: 400 },
      )
    }

    const rawLimit = body.limit
    const limit =
      typeof rawLimit === 'number' && Number.isFinite(rawLimit) && rawLimit > 0
        ? Math.min(MAX_BACKFILL_LIMIT, Math.trunc(rawLimit))
        : DEFAULT_BACKFILL_LIMIT
    const cursor = typeof body.cursor === 'string' ? body.cursor : undefined

    const result = await backfillUserCreditBalancesBatch(deps.backfillDeps, {
      cursor,
      limit,
    })

    if (!result.ok) {
      return noStoreJsonResponse(
        {
          error: 'khala_sync_user_credit_balance_backfill_failed',
          messageSafe: result.messageSafe,
          ok: false,
          reason: result.reason,
          routeRef: KHALA_SYNC_USER_CREDIT_BALANCE_BACKFILL_ROUTE_REF,
        },
        { status: 503 },
      )
    }

    return noStoreJsonResponse({
      ok: true,
      report: result.report,
      routeRef: KHALA_SYNC_USER_CREDIT_BALANCE_BACKFILL_ROUTE_REF,
    })
  })
