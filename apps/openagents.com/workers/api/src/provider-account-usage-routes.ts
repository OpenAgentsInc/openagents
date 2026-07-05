import { assertNoProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  identityAuthNonGateReadsForEnv,
  type IdentityAuthNonGateReads,
  type IdentityAuthStoreEnv,
} from './identity-auth-domain-store'
import { logWorkerRouteError } from './observability'
import {
  type ProviderAccountBudget,
  type ProviderAccountOverBudgetEvent,
  evaluateProviderAccountBudgetEvents,
} from './provider-account-effective-config'
import { PROVIDER_ACCOUNT_LEASE_POLICY_VERSION } from './provider-account-lease-policy'
import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { openAgentsDatabase } from './runtime'
import { currentIsoTimestamp } from './runtime-primitives'
import { TOKEN_USAGE_UNATTRIBUTED_ACCOUNT_REF } from './token-usage'

type HttpResponse = globalThis.Response

const PROVIDER_ACCOUNT_USAGE_COLLECTION =
  'provider_account_usage_attribution_public'

// This projection composes the token ledger (`token_usage_events`, grouped by
// `account_ref`) with the live provider-account pool state. It rebuilds live at
// read, so the same set of pool transitions that move the pool projection also
// move this one.
const PROVIDER_ACCOUNT_USAGE_REBUILDS_ON = [
  'token_usage_event_ingested',
  'provider_account_connected',
  'provider_account_disconnected',
  'provider_account_health_updated',
  'provider_account_lease_acquired',
  'provider_account_lease_released',
] as const

// Bounded ceiling on the number of attributed accounts returned. The ledger and
// pool are both small operator surfaces; an unbounded join is never served.
const ACCOUNT_USAGE_LIMIT = 200

export type ProviderAccountUsageCounts = Readonly<{
  inputTokens: number
  outputTokens: number
  reasoningTokens: number
  cacheReadTokens: number
  cacheWrite5mTokens: number
  cacheWrite1hTokens: number
  totalTokens: number
  usageEvents: number
}>

export type ProviderAccountUsageRow = Readonly<{
  providerAccountRef: string
  // Whether this ref is a real attributed account or the typed unattributed
  // sentinel. Unattributed rows never join to pool state.
  attributed: boolean
  // Pool state, present only when the ref resolves to a live provider account.
  provider: string | null
  accountLabel: string | null
  status: string | null
  health: string | null
  lowCredit: boolean | null
  cooldownUntil: string | null
  cooldownActive: boolean | null
  poolKnown: boolean
  // Ledger usage attributed to this ref.
  windowTotalTokens: number
  totals: ProviderAccountUsageCounts
}>

export type ProviderAccountUsageResponse = Readonly<{
  generatedAt: string
  policyVersion: typeof PROVIDER_ACCOUNT_LEASE_POLICY_VERSION
  staleness: PublicProjectionStalenessContract
  windowSinceIso: string | null
  accounts: ReadonlyArray<ProviderAccountUsageRow>
  // authorityBoundary: advisory events only. These never gate leases or mutate
  // accounts; they are operator-facing signals surfaced from the optional
  // per-account budget config.
  overBudgetEvents: ReadonlyArray<ProviderAccountOverBudgetEvent>
  summary: Readonly<{
    attributedAccounts: number
    unattributedTotalTokens: number
    overBudgetEvents: number
  }>
}>

type LedgerUsageRow = Readonly<{
  account_ref: string | null
  input_tokens: number | null
  output_tokens: number | null
  reasoning_tokens: number | null
  cache_read_tokens: number | null
  cache_write_5m_tokens: number | null
  cache_write_1h_tokens: number | null
  total_tokens: number | null
  window_total_tokens: number | null
  usage_events: number | null
}>

type PoolStateRow = Readonly<{
  provider_account_ref: string
  provider: string
  account_label: string | null
  operator_label: string | null
  status: string
  health: string
  low_credit_flag: number
  cooldown_until: string | null
}>

const countsFromRow = (row: LedgerUsageRow): ProviderAccountUsageCounts => ({
  inputTokens: row.input_tokens ?? 0,
  outputTokens: row.output_tokens ?? 0,
  reasoningTokens: row.reasoning_tokens ?? 0,
  cacheReadTokens: row.cache_read_tokens ?? 0,
  cacheWrite5mTokens: row.cache_write_5m_tokens ?? 0,
  cacheWrite1hTokens: row.cache_write_1h_tokens ?? 0,
  totalTokens: row.total_tokens ?? 0,
  usageEvents: row.usage_events ?? 0,
})

const listLedgerUsageByAccount = async (
  db: D1Database,
  userId: string,
  windowSinceIso: string | null,
): Promise<ReadonlyArray<LedgerUsageRow>> => {
  // Only accounts owned by this operator are returned. The ledger does not
  // carry an owner column, so account ownership is established through the
  // provider_accounts join; the typed unattributed sentinel is surfaced
  // separately and is never owner-scoped (it carries no account).
  const rows = await db
    .prepare(
      `SELECT events.account_ref AS account_ref,
              COALESCE(SUM(events.input_tokens), 0) AS input_tokens,
              COALESCE(SUM(events.output_tokens), 0) AS output_tokens,
              COALESCE(SUM(events.reasoning_tokens), 0) AS reasoning_tokens,
              COALESCE(SUM(events.cache_read_tokens), 0) AS cache_read_tokens,
              COALESCE(SUM(events.cache_write_5m_tokens), 0) AS cache_write_5m_tokens,
              COALESCE(SUM(events.cache_write_1h_tokens), 0) AS cache_write_1h_tokens,
              COALESCE(SUM(events.total_tokens), 0) AS total_tokens,
              COALESCE(
                SUM(
                  CASE
                    WHEN ?1 IS NULL OR events.observed_at >= ?1
                      THEN events.total_tokens
                    ELSE 0
                  END
                ),
                0
              ) AS window_total_tokens,
              COUNT(*) AS usage_events
         FROM token_usage_events events
         LEFT JOIN provider_accounts pa
           ON pa.provider_account_ref = events.account_ref
          AND pa.user_id = ?2
          AND pa.deleted_at IS NULL
        WHERE events.account_ref IS NOT NULL
          AND (
            pa.id IS NOT NULL
            OR events.account_ref = ?3
          )
        GROUP BY events.account_ref
        ORDER BY total_tokens DESC, events.account_ref ASC
        LIMIT ?4`,
    )
    .bind(
      windowSinceIso,
      userId,
      TOKEN_USAGE_UNATTRIBUTED_ACCOUNT_REF,
      ACCOUNT_USAGE_LIMIT,
    )
    .all<LedgerUsageRow>()

  return rows.results
}

// #8362 follow-up: this admin-only, single-table (no cross-domain JOIN)
// `provider_accounts` pool-state projection is the ONE identity/auth read
// call site (of six originally inventoried) that cleared the conservative
// bar for KHALA_SYNC_IDENTITY_NON_GATE_READS — see
// `identity-auth-domain-store.ts`'s module header for the full audit trail.
// `nonGateReads`, when present, ALREADY implements its own d1/compare/
// postgres routing (`makeRoutedIdentityAuthNonGateReads`) with fail-soft D1
// fallback built in; absent => byte-identical inline D1 behavior.
const listPoolState = async (
  db: D1Database,
  userId: string,
  nonGateReads?:
    | Pick<IdentityAuthNonGateReads, 'providerAccountPoolStateByUserId'>
    | undefined,
): Promise<ReadonlyArray<PoolStateRow>> => {
  if (nonGateReads !== undefined) {
    return nonGateReads.providerAccountPoolStateByUserId(
      userId,
      ACCOUNT_USAGE_LIMIT,
    )
  }
  const rows = await db
    .prepare(
      `SELECT pa.provider_account_ref,
              pa.provider,
              pa.account_label,
              pa.operator_label,
              pa.status,
              pa.health,
              COALESCE(pa.low_credit_flag, 0) AS low_credit_flag,
              pa.cooldown_until
         FROM provider_accounts pa
        WHERE pa.user_id = ?
          AND pa.deleted_at IS NULL
        LIMIT ?`,
    )
    .bind(userId, ACCOUNT_USAGE_LIMIT)
    .all<PoolStateRow>()

  return rows.results
}

export const buildProviderAccountUsageProjection = async (
  db: D1Database,
  input: Readonly<{
    budgets?: ReadonlyArray<ProviderAccountBudget> | undefined
    now: string
    userId: string
    windowSinceIso: string | null
  }>,
  // #8362 follow-up: routed non-gate reads for `listPoolState` only (see the
  // comment on that function). Optional and fail-soft-routed already;
  // absent => byte-identical inline D1 behavior for the whole projection.
  nonGateReads?:
    | Pick<IdentityAuthNonGateReads, 'providerAccountPoolStateByUserId'>
    | undefined,
): Promise<ProviderAccountUsageResponse> => {
  const [ledgerRows, poolRows] = await Promise.all([
    listLedgerUsageByAccount(db, input.userId, input.windowSinceIso),
    listPoolState(db, input.userId, nonGateReads),
  ])

  const poolByRef = new Map(
    poolRows.map(row => [row.provider_account_ref, row]),
  )

  const accounts = ledgerRows.map(row => {
    const accountRef = row.account_ref ?? TOKEN_USAGE_UNATTRIBUTED_ACCOUNT_REF
    const attributed = accountRef !== TOKEN_USAGE_UNATTRIBUTED_ACCOUNT_REF
    const pool = poolByRef.get(accountRef)
    const cooldownActive =
      pool === undefined || pool.cooldown_until === null
        ? pool === undefined
          ? null
          : false
        : pool.cooldown_until > input.now

    return {
      providerAccountRef: accountRef,
      attributed,
      provider: pool?.provider ?? null,
      accountLabel: pool?.operator_label ?? pool?.account_label ?? null,
      status: pool?.status ?? null,
      health: pool?.health ?? null,
      lowCredit:
        pool === undefined ? null : pool.low_credit_flag !== 0,
      cooldownUntil: pool?.cooldown_until ?? null,
      cooldownActive,
      poolKnown: pool !== undefined,
      windowTotalTokens: row.window_total_tokens ?? 0,
      totals: countsFromRow(row),
    } satisfies ProviderAccountUsageRow
  })

  const overBudgetEvents = evaluateProviderAccountBudgetEvents({
    budgets: input.budgets ?? [],
    usageByAccountRef: accounts
      .filter(account => account.attributed)
      .map(account => ({
        providerAccountRef: account.providerAccountRef,
        totalTokens: account.totals.totalTokens,
        windowTokens: account.windowTotalTokens,
      })),
  })

  const unattributedTotalTokens = accounts
    .filter(account => !account.attributed)
    .reduce((sum, account) => sum + account.totals.totalTokens, 0)

  const projection: ProviderAccountUsageResponse = {
    generatedAt: input.now,
    policyVersion: PROVIDER_ACCOUNT_LEASE_POLICY_VERSION,
    staleness: liveAtReadStaleness([...PROVIDER_ACCOUNT_USAGE_REBUILDS_ON]),
    windowSinceIso: input.windowSinceIso,
    accounts,
    overBudgetEvents,
    summary: {
      attributedAccounts: accounts.filter(account => account.attributed)
        .length,
      unattributedTotalTokens,
      overBudgetEvents: overBudgetEvents.length,
    },
  }

  assertNoProviderSecretMaterial(projection, PROVIDER_ACCOUNT_USAGE_COLLECTION)

  return projection
}

// #8362 follow-up: intersected with `IdentityAuthStoreEnv` so
// `identityAuthNonGateReadsForEnv(env)` type-checks at the route handler;
// every field beyond `OPENAGENTS_DB` stays optional, so this is a
// zero-behavior-change widening for any caller that does not carry the
// KHALA_SYNC_* bindings.
type ProviderAccountUsageEnv = IdentityAuthStoreEnv &
  Readonly<{
    OPENAGENTS_DB: D1Database
  }>

type ProviderAccountUsageSession = Readonly<{
  user: Readonly<{
    email: string
    userId: string
  }>
}>

type ProviderAccountUsageDependencies<
  Session extends ProviderAccountUsageSession,
  Bindings extends ProviderAccountUsageEnv,
> = Readonly<{
  appendRefreshedSessionCookies: (
    response: HttpResponse,
    session: Session,
  ) => HttpResponse
  isOpenAgentsAdminEmail: (email: string) => boolean
  nowIso?: () => string
  requireBrowserSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
}>

class ProviderAccountUsageSessionMissing extends S.TaggedErrorClass<ProviderAccountUsageSessionMissing>()(
  'ProviderAccountUsageSessionMissing',
  {},
) {}

class ProviderAccountUsageForbidden extends S.TaggedErrorClass<ProviderAccountUsageForbidden>()(
  'ProviderAccountUsageForbidden',
  {},
) {}

class ProviderAccountUsageProjectionError extends S.TaggedErrorClass<ProviderAccountUsageProjectionError>()(
  'ProviderAccountUsageProjectionError',
  {
    error: S.Defect,
  },
) {}

// Window filter: optional `since` ISO query parameter. Bounded fields only —
// this is a deterministic parse of an already-selected projection surface, not
// intent routing.
const parseWindowSince = (request: Request): string | null => {
  const value = new URL(request.url).searchParams.get('since')?.trim()

  if (value === undefined || value === '') {
    return null
  }

  return Number.isFinite(Date.parse(value)) ? value : null
}

export const makeProviderAccountUsageRoutes = <
  Session extends ProviderAccountUsageSession,
  Bindings extends ProviderAccountUsageEnv,
>(
  dependencies: ProviderAccountUsageDependencies<Session, Bindings>,
) => ({
  handleProviderAccountUsageApi: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> =>
    Effect.gen(function* () {
      if (request.method !== 'GET') {
        return methodNotAllowed(['GET'])
      }

      const now = dependencies.nowIso?.() ?? currentIsoTimestamp()

      const session = yield* Effect.tryPromise({
        catch: (error): ProviderAccountUsageProjectionError =>
          new ProviderAccountUsageProjectionError({ error }),
        try: () => dependencies.requireBrowserSession(request, env, ctx),
      })

      if (session === undefined) {
        return yield* new ProviderAccountUsageSessionMissing()
      }

      // Admin/owner-scoped: only OpenAgents admins read the cross-account
      // attribution surface, and the projection is further scoped to the
      // operator's own owned provider accounts inside the SQL.
      if (!dependencies.isOpenAgentsAdminEmail(session.user.email)) {
        return yield* new ProviderAccountUsageForbidden()
      }

      const projection = yield* Effect.tryPromise({
        catch: (error): ProviderAccountUsageProjectionError =>
          new ProviderAccountUsageProjectionError({ error }),
        try: () =>
          buildProviderAccountUsageProjection(
            openAgentsDatabase(env),
            {
              now,
              userId: session.user.userId,
              windowSinceIso: parseWindowSince(request),
            },
            identityAuthNonGateReadsForEnv(env),
          ),
      })

      return dependencies.appendRefreshedSessionCookies(
        noStoreJsonResponse(projection),
        session,
      )
    }).pipe(
      Effect.catchTags({
        ProviderAccountUsageForbidden: () =>
          Effect.succeed(
            noStoreJsonResponse({ error: 'forbidden' }, { status: 403 }),
          ),
        ProviderAccountUsageProjectionError: error => {
          logWorkerRouteError(
            'provider_account_usage_projection_failed',
            error.error,
            { route: '/api/admin/provider-accounts/usage' },
          )

          return Effect.succeed(
            noStoreJsonResponse(
              { error: 'provider_account_usage_projection_failed' },
              { status: 500 },
            ),
          )
        },
        ProviderAccountUsageSessionMissing: () =>
          Effect.succeed(
            noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 }),
          ),
      }),
    ),
})
