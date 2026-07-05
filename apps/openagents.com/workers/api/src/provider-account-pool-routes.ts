import { assertNoProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect } from 'effect'

import type { AgentRegistrationStore } from './agent-registration'
import {
  CustomerOrderAgentAuthFailure,
  authenticateCustomerOrderAgentRequest,
} from './customer-order-agent-auth'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import type { RouteEffect } from './http/route-effects'
import {
  identityAuthMirrorFromEnv,
  type IdentityAuthMirror,
} from './identity-auth-domain-store'
import { optionalString, readJsonObject } from './json-boundary'
import { logWorkerRouteError } from './observability'
import { PROVIDER_ACCOUNT_LEASE_POLICY_VERSION } from './provider-account-lease-policy'
import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { openAgentsDatabase } from './runtime'
import { currentIsoTimestamp } from './runtime-primitives'

type HttpResponse = globalThis.Response

const PROVIDER_ACCOUNT_POOL_COLLECTION = 'provider_account_pool_public'

const PROVIDER_ACCOUNT_POOL_REBUILDS_ON = [
  'provider_account_connected',
  'provider_account_disconnected',
  'provider_account_health_updated',
  'provider_account_lease_acquired',
  'provider_account_lease_released',
  'provider_account_failover_applied',
] as const

export type ProviderAccountPoolReconnect = Readonly<{
  needed: boolean
  reason: string | null
}>

export type ProviderAccountPoolAccount = Readonly<{
  providerAccountRef: string
  provider: string
  accountLabel: string | null
  status: string
  health: string
  eligibility: 'eligible' | 'ineligible'
  eligibilityReasons: ReadonlyArray<string>
  operatorPriority: number
  activeLeaseCount: number
  leaseLimit: number
  cooldownUntil: string | null
  cooldownRemainingSeconds: number | null
  lowCredit: boolean
  recentFailureClass: string | null
  lastSelectedAt: string | null
  lastSanityCheckAt: string | null
  lastSanityCheckResult: string | null
  lastParallelProbeAt: string | null
  lastParallelProbeResult: string | null
  lastSuccessfulLaunchAt: string | null
  lastFailedLaunchAt: string | null
  connectedAt: string | null
  reconnect: ProviderAccountPoolReconnect
}>

export type ProviderAccountPoolLease = Readonly<{
  leaseRef: string
  providerAccountRef: string
  provider: string
  accountLabel: string | null
  requestedAction: string
  runId: string | null
  assignmentId: string | null
  orderId: string | null
  startedAt: string
  expiresAt: string
  lastTouchedAt: string | null
  status: string
}>

export type ProviderAccountPoolNextSelection = Readonly<{
  status: 'selected' | 'none'
  providerAccountRef: string | null
  provider: string | null
  accountLabel: string | null
  selectionReason: string
  activeLeaseCount: number | null
  leaseLimit: number | null
}>

export type ProviderAccountPoolSummary = Readonly<{
  total: number
  eligible: number
  activeLeaseCount: number
  lowCredit: number
  requiresReauth: number
  cooldown: number
  unhealthy: number
}>

export type ProviderAccountPoolResponse = Readonly<{
  generatedAt: string
  provider: 'all_connected_provider_accounts'
  policyVersion: typeof PROVIDER_ACCOUNT_LEASE_POLICY_VERSION
  staleness: PublicProjectionStalenessContract
  accounts: ReadonlyArray<ProviderAccountPoolAccount>
  activeLeases: ReadonlyArray<ProviderAccountPoolLease>
  nextSelection: ProviderAccountPoolNextSelection
  summary: ProviderAccountPoolSummary
}>

export type ProviderAccountPoolManualResetResponse = Readonly<{
  ok: true
  providerAccountRef: string
  resetAt: string
}>

type PoolAccountRow = Readonly<{
  provider_account_ref: string
  provider: string
  account_label: string | null
  operator_label: string | null
  status: string
  health: string
  operator_priority: number
  lease_limit: number
  low_credit_flag: number
  cooldown_until: string | null
  recent_failure_class: string | null
  last_sanity_check_at: string | null
  last_sanity_check_result: string | null
  last_parallel_probe_at: string | null
  last_parallel_probe_result: string | null
  last_selected_at: string | null
  last_successful_launch_at: string | null
  last_failed_launch_at: string | null
  reauth_required_reason: string | null
  connected_at: string | null
  deleted_at: string | null
  has_secret_ref: number
  active_lease_count: number
}>

type PoolLeaseRow = Readonly<{
  lease_ref: string
  provider_account_ref: string
  provider: string
  account_label: string | null
  requested_action: string
  run_id: string | null
  assignment_id: string | null
  order_id: string | null
  started_at: string
  expires_at: string
  last_touched_at: string | null
  status: string
}>

type PoolSelectionRow = Readonly<{
  provider_account_ref: string
  provider: string
  account_label: string | null
  active_lease_count: number
  lease_limit: number
  operator_priority: number
}>

const poolEligibilityReasons = (
  row: PoolAccountRow,
  now: string,
): ReadonlyArray<string> => {
  const reasons: Array<string> = []

  if (row.deleted_at !== null) {
    reasons.push('deleted')
  }
  if (row.status !== 'connected') {
    reasons.push(`status:${row.status}`)
  }
  if (row.health !== 'healthy') {
    reasons.push(`health:${row.health}`)
  }
  if (row.has_secret_ref === 0) {
    reasons.push('missing_server_auth_material')
  }
  if (row.low_credit_flag !== 0) {
    reasons.push('low_credit')
  }
  if (row.reauth_required_reason !== null) {
    reasons.push(`reauth_required:${row.reauth_required_reason}`)
  }
  if (row.cooldown_until !== null && row.cooldown_until > now) {
    reasons.push('cooldown')
  }
  if (row.active_lease_count >= row.lease_limit) {
    reasons.push('lease_limit_reached')
  }

  return reasons
}

const poolReconnect = (row: PoolAccountRow): ProviderAccountPoolReconnect => {
  if (row.health === 'requires_reauth' || row.reauth_required_reason !== null) {
    return { needed: true, reason: 'requires_reauth' }
  }

  if (row.status !== 'connected') {
    return { needed: true, reason: `status:${row.status}` }
  }

  if (row.has_secret_ref === 0) {
    return { needed: true, reason: 'missing_server_auth_material' }
  }

  if (row.health === 'unhealthy') {
    return { needed: true, reason: 'unhealthy' }
  }

  return { needed: false, reason: null }
}

const cooldownRemainingSeconds = (
  cooldownUntil: string | null,
  now: string,
): number | null => {
  if (cooldownUntil === null) {
    return null
  }

  const remainingMs = Date.parse(cooldownUntil) - Date.parse(now)

  return Number.isFinite(remainingMs) && remainingMs > 0
    ? Math.ceil(remainingMs / 1000)
    : null
}

// KS-8.18 follow-up (#8362): deliberately NOT mirrored. This runs inside
// `buildProviderAccountPoolProjection`, on the hot GET-projection read
// path (every pool status fetch), and is a bulk, key-less UPDATE — mirroring
// it here would add an unbounded Postgres write to a read path, which is
// exactly what the identity/auth RUNBOOK section warns against inheriting.
// These status='expired' transitions converge on the next `--restart`
// backfill sweep instead.
const expireStalePoolLeases = async (
  db: D1Database,
  now: string,
): Promise<void> => {
  await db
    .prepare(
      `UPDATE provider_account_leases
          SET status = 'expired',
              terminal_outcome = 'expired_before_release'
        WHERE status = 'active'
          AND expires_at <= ?`,
    )
    .bind(now)
    .run()
}

const listPoolAccounts = async (
  db: D1Database,
  userId: string,
  now: string,
): Promise<ReadonlyArray<PoolAccountRow>> => {
  const rows = await db
    .prepare(
      `SELECT pa.provider_account_ref,
              pa.provider,
              pa.account_label,
              pa.operator_label,
              pa.status,
              pa.health,
              pa.operator_priority,
              COALESCE(pa.lease_limit, 1) AS lease_limit,
              COALESCE(pa.low_credit_flag, 0) AS low_credit_flag,
              pa.cooldown_until,
              pa.recent_failure_class,
              pa.last_sanity_check_at,
              pa.last_sanity_check_result,
              pa.last_parallel_probe_at,
              pa.last_parallel_probe_result,
              pa.last_selected_at,
              pa.last_successful_launch_at,
              pa.last_failed_launch_at,
              pa.reauth_required_reason,
              pa.connected_at,
              pa.deleted_at,
              CASE WHEN pa.secret_ref IS NULL THEN 0 ELSE 1 END AS has_secret_ref,
              COUNT(active_leases.id) AS active_lease_count
         FROM provider_accounts pa
         LEFT JOIN provider_account_leases active_leases
           ON active_leases.provider_account_id = pa.id
          AND active_leases.status = 'active'
          AND active_leases.expires_at > ?
        WHERE pa.user_id = ?
          AND pa.deleted_at IS NULL
        GROUP BY pa.id
        ORDER BY
          CASE WHEN pa.status = 'connected' AND pa.health = 'healthy' THEN 0 ELSE 1 END,
          pa.provider ASC,
          COALESCE(pa.low_credit_flag, 0) ASC,
          pa.operator_priority ASC,
          COALESCE(pa.operator_label, pa.account_label, pa.provider_account_ref) ASC
        LIMIT 200`,
    )
    .bind(now, userId)
    .all<PoolAccountRow>()

  return rows.results
}

const listPoolActiveLeases = async (
  db: D1Database,
  userId: string,
  now: string,
): Promise<ReadonlyArray<PoolLeaseRow>> => {
  const rows = await db
    .prepare(
      `SELECT l.lease_ref,
              l.provider_account_ref,
              pa.provider,
              COALESCE(pa.operator_label, pa.account_label) AS account_label,
              l.requested_action,
              l.run_id,
              l.assignment_id,
              l.order_id,
              l.started_at,
              l.expires_at,
              l.last_touched_at,
              l.status
       FROM provider_account_leases l
       JOIN provider_accounts pa ON pa.id = l.provider_account_id
       WHERE l.user_id = ?
         AND l.status = 'active'
         AND l.expires_at > ?
       ORDER BY l.started_at DESC
       LIMIT 100`,
    )
    .bind(userId, now)
    .all<PoolLeaseRow>()

  return rows.results
}

const readPoolNextSelection = async (
  db: D1Database,
  userId: string,
  now: string,
): Promise<ProviderAccountPoolNextSelection> => {
  const row = await db
    .prepare(
      `SELECT pa.provider_account_ref,
              pa.provider,
              COALESCE(pa.operator_label, pa.account_label) AS account_label,
              COUNT(active_leases.id) AS active_lease_count,
              COALESCE(pa.lease_limit, 1) AS lease_limit,
              pa.operator_priority
       FROM provider_accounts pa
       LEFT JOIN provider_account_leases active_leases
         ON active_leases.provider_account_id = pa.id
        AND active_leases.status = 'active'
        AND active_leases.expires_at > ?
       WHERE pa.user_id = ?
         AND pa.status = 'connected'
         AND pa.health = 'healthy'
         AND pa.secret_ref IS NOT NULL
         AND pa.deleted_at IS NULL
         AND COALESCE(pa.low_credit_flag, 0) = 0
         AND pa.reauth_required_reason IS NULL
         AND (pa.cooldown_until IS NULL OR pa.cooldown_until <= ?)
       GROUP BY pa.id
       HAVING COUNT(active_leases.id) < COALESCE(pa.lease_limit, 1)
       ORDER BY
         COUNT(active_leases.id) ASC,
         pa.provider ASC,
         pa.operator_priority ASC,
         COALESCE(pa.last_selected_at, pa.connected_at, pa.created_at) ASC,
         pa.provider_account_ref ASC
       LIMIT 1`,
    )
    .bind(now, userId, now)
    .first<PoolSelectionRow>()

  if (row === null) {
    return {
      status: 'none',
      providerAccountRef: null,
      provider: null,
      accountLabel: null,
      selectionReason:
        'No connected healthy provider account is currently eligible for lease.',
      activeLeaseCount: null,
      leaseLimit: null,
    }
  }

  return {
    status: 'selected',
    providerAccountRef: row.provider_account_ref,
    provider: row.provider,
    accountLabel: row.account_label,
    selectionReason: `Selected connected healthy account with ${row.active_lease_count} active lease(s), priority ${row.operator_priority}, and no cooldown, reconnect marker, or low-credit flag.`,
    activeLeaseCount: row.active_lease_count,
    leaseLimit: row.lease_limit,
  }
}

export const buildProviderAccountPoolProjection = async (
  db: D1Database,
  userId: string,
  now: string,
): Promise<ProviderAccountPoolResponse> => {
  await expireStalePoolLeases(db, now)

  const [accountRows, leaseRows, nextSelection] = await Promise.all([
    listPoolAccounts(db, userId, now),
    listPoolActiveLeases(db, userId, now),
    readPoolNextSelection(db, userId, now),
  ])

  const accounts = accountRows.map(row => {
    const eligibilityReasons = poolEligibilityReasons(row, now)

    return {
      providerAccountRef: row.provider_account_ref,
      provider: row.provider,
      accountLabel: row.operator_label ?? row.account_label,
      status: row.status,
      health: row.health,
      eligibility:
        eligibilityReasons.length === 0
          ? ('eligible' as const)
          : ('ineligible' as const),
      eligibilityReasons,
      operatorPriority: row.operator_priority,
      activeLeaseCount: row.active_lease_count,
      leaseLimit: row.lease_limit,
      cooldownUntil: row.cooldown_until,
      cooldownRemainingSeconds: cooldownRemainingSeconds(
        row.cooldown_until,
        now,
      ),
      lowCredit: row.low_credit_flag !== 0,
      recentFailureClass: row.recent_failure_class,
      lastSelectedAt: row.last_selected_at,
      lastSanityCheckAt: row.last_sanity_check_at,
      lastSanityCheckResult: row.last_sanity_check_result,
      lastParallelProbeAt: row.last_parallel_probe_at,
      lastParallelProbeResult: row.last_parallel_probe_result,
      lastSuccessfulLaunchAt: row.last_successful_launch_at,
      lastFailedLaunchAt: row.last_failed_launch_at,
      connectedAt: row.connected_at,
      reconnect: poolReconnect(row),
    } satisfies ProviderAccountPoolAccount
  })

  const activeLeases = leaseRows.map(row => ({
    leaseRef: row.lease_ref,
    providerAccountRef: row.provider_account_ref,
    provider: row.provider,
    accountLabel: row.account_label,
    requestedAction: row.requested_action,
    runId: row.run_id,
    assignmentId: row.assignment_id,
    orderId: row.order_id,
    startedAt: row.started_at,
    expiresAt: row.expires_at,
    lastTouchedAt: row.last_touched_at,
    status: row.status,
  }))

  const projection: ProviderAccountPoolResponse = {
    generatedAt: now,
    provider: 'all_connected_provider_accounts',
    policyVersion: PROVIDER_ACCOUNT_LEASE_POLICY_VERSION,
    staleness: liveAtReadStaleness([...PROVIDER_ACCOUNT_POOL_REBUILDS_ON]),
    accounts,
    activeLeases,
    nextSelection,
    summary: {
      total: accounts.length,
      eligible: accounts.filter(account => account.eligibility === 'eligible')
        .length,
      activeLeaseCount: activeLeases.length,
      lowCredit: accounts.filter(account => account.lowCredit).length,
      requiresReauth: accounts.filter(
        account =>
          account.health === 'requires_reauth' ||
          account.eligibilityReasons.some(reason =>
            reason.startsWith('reauth_required:'),
          ),
      ).length,
      cooldown: accounts.filter(account =>
        account.eligibilityReasons.includes('cooldown'),
      ).length,
      unhealthy: accounts.filter(account => account.health === 'unhealthy')
        .length,
    },
  }

  assertNoProviderSecretMaterial(projection, PROVIDER_ACCOUNT_POOL_COLLECTION)

  return projection
}

type ProviderAccountPoolEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>

type ProviderAccountPoolSession = Readonly<{
  user: Readonly<{
    userId: string
  }>
}>

type ProviderAccountPoolDependencies<
  Session extends ProviderAccountPoolSession,
  Bindings extends ProviderAccountPoolEnv,
> = Readonly<{
  agentStore: (env: Bindings) => AgentRegistrationStore
  appendRefreshedSessionCookies: (
    response: HttpResponse,
    session: Session,
  ) => HttpResponse
  nowIso?: () => string
  requireBrowserSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
}>

const hasBearerAuthorization = (request: Request): boolean =>
  request.headers
    .get('authorization')
    ?.trim()
    .toLowerCase()
    .startsWith('bearer ') === true

class ProviderAccountPoolSessionMissing extends Error {
  override readonly name = 'ProviderAccountPoolSessionMissing'
}

class ProviderAccountPoolProjectionError extends Error {
  override readonly name = 'ProviderAccountPoolProjectionError'
}

class ProviderAccountPoolBadRequest extends Error {
  override readonly name = 'ProviderAccountPoolBadRequest'
}

const poolProjectionError = (
  error: unknown,
): ProviderAccountPoolProjectionError =>
  new ProviderAccountPoolProjectionError(
    error instanceof Error ? error.message : String(error),
  )

const readManualResetProviderAccountRef = async (
  request: Request,
): Promise<string> => {
  const body = await readJsonObject(request)
  const providerAccountRef = optionalString(body.providerAccountRef)

  if (providerAccountRef === undefined) {
    throw new ProviderAccountPoolBadRequest()
  }

  return providerAccountRef
}

const resetProviderAccountPoolAccount = async (
  db: D1Database,
  input: Readonly<{
    providerAccountRef: string
    resetAt: string
    userId: string
  }>,
  // KS-8.18 follow-up (#8362): fail-soft identity/auth mirror handle.
  mirror?: IdentityAuthMirror | undefined,
): Promise<boolean> => {
  const result = await db
    .prepare(
      `UPDATE provider_accounts
          SET cooldown_until = NULL,
              recent_failure_class = NULL,
              updated_at = ?
        WHERE user_id = ?
          AND provider_account_ref = ?
          AND deleted_at IS NULL`,
    )
    .bind(input.resetAt, input.userId, input.providerAccountRef)
    .run()

  const changed = (result.meta?.changes ?? 0) > 0
  if (changed && mirror !== undefined) {
    // No `id` in scope — scan-mirror on the composite WHERE predicate
    // (neither column is custody-bearing).
    await mirror.mirrorRowsWhere(
      'provider_accounts',
      ['user_id', 'provider_account_ref'],
      [input.userId, input.providerAccountRef],
    )
  }
  return changed
}

const agentAuthFailureStatus = (
  failure: CustomerOrderAgentAuthFailure,
): number =>
  failure.failureKind === 'under_scoped' ||
  failure.failureKind === 'wrong_owner'
    ? 403
    : 401

export const makeProviderAccountPoolRoutes = <
  Session extends ProviderAccountPoolSession,
  Bindings extends ProviderAccountPoolEnv,
>(
  dependencies: ProviderAccountPoolDependencies<Session, Bindings>,
) => ({
  handleProviderAccountPoolApi: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): RouteEffect =>
    Effect.gen(function* () {
      const pathname = new URL(request.url).pathname
      const isReset = pathname === '/api/provider-accounts/pool/reset'

      if (!isReset && request.method !== 'GET') {
        return methodNotAllowed(['GET'])
      }

      if (isReset && request.method !== 'POST') {
        return methodNotAllowed(['POST'])
      }

      const now = dependencies.nowIso?.() ?? currentIsoTimestamp()

      if (isReset) {
        if (hasBearerAuthorization(request)) {
          return noStoreJsonResponse(
            { error: 'browser_session_required' },
            { status: 401 },
          )
        }

        const session = yield* Effect.tryPromise({
          catch: poolProjectionError,
          try: () => dependencies.requireBrowserSession(request, env, ctx),
        })

        if (session === undefined) {
          return yield* Effect.fail(new ProviderAccountPoolSessionMissing())
        }

        const providerAccountRef = yield* Effect.tryPromise({
          catch: error =>
            error instanceof ProviderAccountPoolBadRequest
              ? error
              : poolProjectionError(error),
          try: () => readManualResetProviderAccountRef(request),
        })
        const reset = yield* Effect.tryPromise({
          catch: poolProjectionError,
          try: () =>
            resetProviderAccountPoolAccount(
              openAgentsDatabase(env),
              {
                providerAccountRef,
                resetAt: now,
                userId: session.user.userId,
              },
              identityAuthMirrorFromEnv(env),
            ),
        })

        if (!reset) {
          return noStoreJsonResponse(
            { error: 'provider_account_not_found' },
            { status: 404 },
          )
        }

        const response: ProviderAccountPoolManualResetResponse = {
          ok: true,
          providerAccountRef,
          resetAt: now,
        }

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse(response),
          session,
        )
      }

      if (hasBearerAuthorization(request)) {
        const auth = yield* authenticateCustomerOrderAgentRequest(
          request,
          dependencies.agentStore(env),
          {
            nowIso: () => now,
            requiredScope: 'customer_orders.read',
          },
        )
        const projection = yield* Effect.tryPromise({
          catch: poolProjectionError,
          try: () =>
            buildProviderAccountPoolProjection(
              openAgentsDatabase(env),
              auth.ownerUserId,
              now,
            ),
        })

        return noStoreJsonResponse(projection)
      }

      const session = yield* Effect.tryPromise({
        catch: poolProjectionError,
        try: () => dependencies.requireBrowserSession(request, env, ctx),
      })

      if (session === undefined) {
        return yield* Effect.fail(new ProviderAccountPoolSessionMissing())
      }

      const projection = yield* Effect.tryPromise({
        catch: poolProjectionError,
        try: () =>
          buildProviderAccountPoolProjection(
            openAgentsDatabase(env),
            session.user.userId,
            now,
          ),
      })

      return dependencies.appendRefreshedSessionCookies(
        noStoreJsonResponse(projection),
        session,
      )
    }).pipe(
      Effect.catchTag('CustomerOrderAgentAuthFailure', failure =>
        Effect.succeed(
          noStoreJsonResponse(
            { error: 'unauthorized', reason: failure.reason },
            { status: agentAuthFailureStatus(failure) },
          ),
        ),
      ),
      Effect.catch(error => {
        if (error instanceof ProviderAccountPoolSessionMissing) {
          return Effect.succeed(
            noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 }),
          )
        }

        if (error instanceof ProviderAccountPoolBadRequest) {
          return Effect.succeed(
            noStoreJsonResponse(
              { error: 'provider_account_ref_required' },
              { status: 400 },
            ),
          )
        }

        logWorkerRouteError('provider_account_pool_projection_failed', error, {
          route: '/api/provider-accounts/pool',
        })

        return Effect.succeed(
          noStoreJsonResponse(
            { error: 'provider_account_pool_projection_failed' },
            { status: 500 },
          ),
        )
      }),
    ),
})
