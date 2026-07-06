// Aiur ops views — read-only owner-gated day-one operating view (AIUR-3,
// #8501, epic #8467). "Who signed up, what did they run, did it charge
// correctly, and is the executor/push/inference stack up" — without
// shelling into D1.
//
// HONESTY DISCIPLINE: every field here is either a real read from an exact
// source table, or an explicit `not_measured` sentinel — never a fabricated
// or guessed value. The runs view reads `token_usage_events` directly
// (filtered to `demand_source = 'khala_mobile_org_cloud_runtime'`, the
// exact tag the org-cloud runtime-usage ingest route — #8473's
// `khala-cloud-runtime-usage-routes.ts` — already writes), so "recent runs"
// and "exact usage receipts per turn" are the SAME real rows, not a mock.
//
// PIN (documented gap, see the closing comment on #8501): the issue asks
// for the runs view to also be "live via the same Khala Sync scopes the
// mobile app renders (runtime_turn/runtime_event)". Today's Khala Sync
// scope taxonomy is owner-scoped (`scope.user.<id>`) or thread-scoped
// (`scope.thread.<id>`) — there is no cross-user "admin sees every user's
// threads" scope, and adding one is a sync-engine authorization change (out
// of this lane's safe scope: it would widen what a caller can read across
// owner boundaries, which is exactly the kind of change this repo's
// invariants require an explicit, reviewed decision for). This v1 instead
// reads the same underlying exact ledger the mobile app's turns are
// eventually billed from, polled rather than pushed live. Read-only in v1
// — no mutations here beyond navigation into AIUR-2's credits page.

import { methodNotAllowed, noStoreJsonResponse, unauthorized } from './http/responses'
import { optionalInteger } from './json-boundary'
import { currentIsoTimestamp } from './runtime-primitives'

type HttpResponse = globalThis.Response

export const ADMIN_OPS_RUNS_PATH = '/api/admin/ops/runs'
export const ADMIN_OPS_HEALTH_PATH = '/api/admin/ops/health'

export type AdminCaller = Readonly<{ userId: string }>

export type AdminOpsRouteDependencies<Bindings> = Readonly<{
  db: (env: Bindings) => D1Database
  nowIso?: () => string
  /** Self-fetch used for the live Khala public-stats reachability check —
   * injectable so tests never make a real network call. */
  fetch?: (input: string, init?: RequestInit) => Promise<HttpResponse>
  requireAdminCaller: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<AdminCaller | undefined>
}>

const requireAdmin = async <Bindings>(
  dependencies: AdminOpsRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Promise<AdminCaller | undefined> =>
  dependencies.requireAdminCaller(request, env, ctx)

// The exact demand-source tag the org-cloud runtime-usage ingest route
// writes (khala-cloud-runtime-usage-routes.ts,
// KHALA_CLOUD_RUNTIME_DEMAND_SOURCE). Kept as a literal here rather than
// importing the constant, to avoid coupling this read-only ops module to
// that ingest route's internals — if that constant ever changes, update
// both call sites deliberately.
const ORG_CLOUD_RUNTIME_DEMAND_SOURCE = 'khala_mobile_org_cloud_runtime'

// ----------------------------------------------------------------------------
// GET /api/admin/ops/runs — recent org-cloud coding turns, exact usage
// ----------------------------------------------------------------------------

type RunRow = Readonly<{
  observed_at: string
  actor_user_id: string | null
  run_ref: string | null
  task_ref: string | null
  provider: string | null
  model: string | null
  total_tokens: number
  cost_amount: number | null
  currency: string | null
  usage_truth: string
}>

const routeRuns = async <Bindings>(
  dependencies: AdminOpsRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Promise<HttpResponse> => {
  if (request.method !== 'GET') return methodNotAllowed(['GET'])
  const url = new URL(request.url)
  const limit = Math.max(
    1,
    Math.min(200, optionalInteger(url.searchParams.get('limit') ?? undefined) ?? 50),
  )
  const db = dependencies.db(env)

  const result = await db
    .prepare(
      `SELECT observed_at, actor_user_id, run_ref, task_ref, provider, model,
              total_tokens, cost_amount, currency, usage_truth
         FROM token_usage_events
        WHERE demand_source = ?
        ORDER BY observed_at DESC
        LIMIT ?`,
    )
    .bind(ORG_CLOUD_RUNTIME_DEMAND_SOURCE, limit)
    .all<RunRow>()

  return noStoreJsonResponse({
    ok: true,
    runs: result.results.map(row => ({
      costAmount: row.cost_amount,
      currency: row.currency,
      model: row.model,
      observedAt: row.observed_at,
      provider: row.provider,
      // "threadId"/"turnId" naming matches the mobile wire contract
      // (khala_runtime_control_intent.v1) even though the underlying
      // ledger columns are the generic run_ref/task_ref.
      threadId: row.run_ref,
      turnId: row.task_ref,
      totalTokens: row.total_tokens,
      usageTruth: row.usage_truth,
      userId: row.actor_user_id,
    })),
    // Honest pin: not yet a live Khala Sync feed — see the module header.
    liveViaKhalaSync: false,
  })
}

// ----------------------------------------------------------------------------
// GET /api/admin/ops/health — a simple green/red strip
// ----------------------------------------------------------------------------

type HealthCheckResult =
  | Readonly<{ status: 'ok'; value: string; checkedAt: string }>
  | Readonly<{ status: 'not_measured'; reasonRef: string }>
  | Readonly<{ status: 'error'; messageSafe: string; checkedAt: string }>

const readLastOrgCloudTurnCompletedAt = async (
  db: D1Database,
): Promise<HealthCheckResult> => {
  const row = await db
    .prepare(
      `SELECT MAX(observed_at) AS last_observed_at
         FROM token_usage_events
        WHERE demand_source = ?`,
    )
    .bind(ORG_CLOUD_RUNTIME_DEMAND_SOURCE)
    .first<{ last_observed_at: string | null }>()

  if (row?.last_observed_at === undefined || row.last_observed_at === null) {
    return {
      reasonRef: 'reason.admin_ops.no_org_cloud_turns_yet',
      status: 'not_measured',
    }
  }

  return { checkedAt: row.last_observed_at, status: 'ok', value: row.last_observed_at }
}

const readPushDeviceTokenCount = async (
  db: D1Database,
  nowIso: () => string,
): Promise<HealthCheckResult> => {
  const row = await db
    .prepare(`SELECT COUNT(*) AS token_count FROM push_device_tokens`)
    .first<{ token_count: number }>()
  const count = row?.token_count ?? 0

  return { checkedAt: nowIso(), status: 'ok', value: String(count) }
}

/**
 * A live, no-spend reachability check against the PUBLIC Khala tokens-served
 * stats endpoint — a real round trip through the same Worker/D1 path the
 * public `/khala` and homepage counters use, proving the Khala surface is
 * responding. This is deliberately NOT a real completions/generation call
 * (that would spend real inference cost during a routine health check);
 * label it precisely rather than overclaiming it proves the completions
 * path specifically.
 */
const checkKhalaPublicStatsReachable = async <Bindings>(
  dependencies: AdminOpsRouteDependencies<Bindings>,
  env: Bindings,
  origin: string,
): Promise<HealthCheckResult> => {
  const fetchImpl = dependencies.fetch ?? globalThis.fetch.bind(globalThis)
  const checkedAt = (dependencies.nowIso ?? currentIsoTimestamp)()
  try {
    const response = await fetchImpl(`${origin}/api/public/khala-tokens-served`)
    if (!response.ok) {
      return {
        checkedAt,
        messageSafe: `Khala public stats endpoint returned HTTP ${response.status}.`,
        status: 'error',
      }
    }
    return { checkedAt, status: 'ok', value: 'reachable' }
  } catch (error) {
    return {
      checkedAt,
      messageSafe: error instanceof Error ? error.message : 'Reachability check failed.',
      status: 'error',
    }
  }
}

const routeHealth = async <Bindings>(
  dependencies: AdminOpsRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Promise<HttpResponse> => {
  if (request.method !== 'GET') return methodNotAllowed(['GET'])
  const db = dependencies.db(env)
  const origin = new URL(request.url).origin

  const nowIso = dependencies.nowIso ?? currentIsoTimestamp
  const [lastOrgCloudTurn, pushDeviceTokens, khalaPublicStats] = await Promise.all([
    readLastOrgCloudTurnCompletedAt(db),
    readPushDeviceTokenCount(db, nowIso),
    checkKhalaPublicStatsReachable(dependencies, env, origin),
  ])

  return noStoreJsonResponse({
    ok: true,
    checks: {
      // Real: the exact last time an org-cloud coding turn was recorded, or
      // an honest not_measured if none exist yet.
      lastOrgCloudTurnCompletedAt: lastOrgCloudTurn,
      // Real: how many device tokens are registered — a readiness signal,
      // not a delivery-success signal (no send-log table exists yet to
      // measure the latter honestly).
      pushDeviceTokensRegistered: pushDeviceTokens,
      // Real, live: a no-spend round trip through the public Khala stats
      // surface.
      khalaPublicStatsReachable: khalaPublicStats,
    },
  })
}

// ----------------------------------------------------------------------------
// Router entry point
// ----------------------------------------------------------------------------

export const makeAdminOpsRoutes = <Bindings>(
  dependencies: AdminOpsRouteDependencies<Bindings>,
) => ({
  handleAdminOpsRunsApi: async (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Promise<HttpResponse> => {
    const caller = await requireAdmin(dependencies, request, env, ctx)
    if (caller === undefined) return unauthorized()
    return routeRuns(dependencies, request, env)
  },

  handleAdminOpsHealthApi: async (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Promise<HttpResponse> => {
    const caller = await requireAdmin(dependencies, request, env, ctx)
    if (caller === undefined) return unauthorized()
    return routeHealth(dependencies, request, env)
  },
})
