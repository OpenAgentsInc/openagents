// OB-6 (P1 Track C, #8563): admin-gated HTTP surface for the daily sales
// ledger computed in `business-outreach-daily-ledger.ts`. Follows the exact
// same owner-gate composition and response shape conventions as
// `admin-ops-routes.ts` (AIUR-3, #8501) — read that file's header first.

import {
  computeDailySalesLedger,
  DailySalesLedgerValidationError,
  type SarahTurnStoreSqlClient,
} from './business-outreach-daily-ledger'
import { methodNotAllowed, noStoreJsonResponse, unauthorized } from './http/responses'
import { defaultMakeSqlClient } from './khala-sync-db-smoke-routes'
import { currentIsoTimestamp } from './runtime-primitives'

type HttpResponse = globalThis.Response

export const ADMIN_OPS_DAILY_SALES_LEDGER_PATH = '/api/admin/ops/daily-sales-ledger'

export type AdminCaller = Readonly<{ userId: string }>

export type DailySalesLedgerRouteDependencies<Bindings> = Readonly<{
  db: (env: Bindings) => D1Database
  /**
   * `env.KHALA_SYNC_DB` — the khala-sync Postgres the Sarah service writes
   * `sarah_transcript_turns` to. Absent binding => conversations reported
   * as `not_measured` (never zero-faked).
   */
  khalaSyncBinding?:
    | ((env: Bindings) => Readonly<{ connectionString: string }> | undefined)
    | undefined
  /** Injectable Postgres client factory (tests). Default: shared pg pool. */
  makeSqlClient?:
    | ((connectionString: string) => Promise<SarahTurnStoreSqlClient>)
    | undefined
  nowIso?: () => string
  requireAdminCaller: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<AdminCaller | undefined>
}>

const isoDateNDaysAgo = (nowIso: string, days: number): string => {
  const date = new Date(nowIso)
  date.setUTCDate(date.getUTCDate() - days)
  return date.toISOString().slice(0, 10)
}

const routeDailySalesLedger = async <Bindings>(
  dependencies: DailySalesLedgerRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Promise<HttpResponse> => {
  if (request.method !== 'GET') return methodNotAllowed(['GET'])

  const url = new URL(request.url)
  const nowIso = (dependencies.nowIso ?? currentIsoTimestamp)()
  const today = nowIso.slice(0, 10)
  // Default window: the trailing 7 days (matching the OB-6 exit bar of "7
  // consecutive daily ledgers").
  const since = url.searchParams.get('since') ?? isoDateNDaysAgo(nowIso, 6)
  const until = url.searchParams.get('until') ?? today

  try {
    const ledger = await computeDailySalesLedger(dependencies.db(env), {
      nowIso: () => nowIso,
      sarahTurnStore: {
        binding: dependencies.khalaSyncBinding?.(env),
        makeSqlClient: dependencies.makeSqlClient ?? defaultMakeSqlClient,
      },
      since,
      until,
    })
    return noStoreJsonResponse({ ledger, ok: true })
  } catch (error) {
    if (error instanceof DailySalesLedgerValidationError) {
      return noStoreJsonResponse({ messageSafe: error.message, ok: false }, { status: 400 })
    }
    throw error
  }
}

export const makeDailySalesLedgerRoutes = <Bindings>(
  dependencies: DailySalesLedgerRouteDependencies<Bindings>,
) => ({
  handleDailySalesLedgerApi: async (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Promise<HttpResponse> => {
    const caller = await dependencies.requireAdminCaller(request, env, ctx)
    if (caller === undefined) return unauthorized()
    return routeDailySalesLedger(dependencies, request, env)
  },
})
