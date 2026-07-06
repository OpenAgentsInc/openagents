// Aiur credits console — admin-gated credit grant/adjust routes (AIUR-2,
// #8500, epic #8467). The first Khala Code mobile MVP build ships WITHOUT
// RevenueCat/IAP (#8481 postponed): credits are assigned manually by the
// owner through the Aiur admin panel (apps/aiur/), which calls these routes.
//
// AUTH: every route requires `dependencies.requireAdminCaller(request, env,
// ctx)` to resolve a `{ userId }` — FAIL CLOSED (401 on `undefined`). The
// caller constructing these routes (index.ts) wires that check as the
// COMPOSITION of the existing mobile/browser user-bearer-session boundary
// (`requireUserBearerSession`) plus the existing admin-email allowlist
// (`isOpenAgentsAdminEmail`) — never a shared static token in a client
// bundle, and never a new auth primitive. Aiur forwards the SAME OpenAuth
// bearer token that authenticated its own owner-only session
// (`apps/aiur/src/auth/`), so this is genuinely the owner acting, not a
// separate credential.
//
// MONEY MOVEMENT: every grant/clawback goes through the exact-only,
// receipted, idempotent primitives in `inference/admin-credit-grant.ts`
// (itself a thin, admin-specific wrapper over the same
// `usd-credit-bridge.ts` / `inference-abuse-controls.ts` primitives the
// rest of Pool B uses) — this file only parses requests, resolves the
// target user, and shapes responses. No ledger logic lives here.

import { Effect } from 'effect'

import {
  type AdminCreditClawbackDeps,
  type AdminCreditGrantDeps,
  type AdminCreditGrantOutcome,
  clawbackAdminCredit,
  grantAdminCredit,
  readAdminCreditGrantsForUser,
  readRecentAdminCreditGrants,
} from './inference/admin-credit-grant'
import {
  readGithubSignupCreditGrantsForUser,
} from './inference/github-signup-credit-grant'
import { agentRefForUser } from './inference/usd-credit-bridge'
import { msatToUsdCentsRound } from './inference/usd-msat-conversion'
import { methodNotAllowed, noStoreJsonResponse, unauthorized } from './http/responses'
import { optionalInteger, optionalString, readJsonObject } from './json-boundary'
import { readAgentBalance } from './payments-ledger'

type HttpResponse = globalThis.Response

export const ADMIN_CREDITS_USERS_PATH = '/api/admin/credits/users'
export const ADMIN_CREDITS_BALANCE_PATH = '/api/admin/credits/balance'
export const ADMIN_CREDITS_HISTORY_PATH = '/api/admin/credits/history'
export const ADMIN_CREDITS_GRANT_PATH = '/api/admin/credits/grant'
export const ADMIN_CREDITS_CLAWBACK_PATH = '/api/admin/credits/clawback'
export const ADMIN_CREDITS_RECENT_GRANTS_PATH =
  '/api/admin/credits/recent-grants'

export type AdminCaller = Readonly<{ userId: string }>

export type AdminCreditsRouteDependencies<Bindings> = Readonly<{
  db: (env: Bindings) => D1Database
  nowIso?: () => string
  requireAdminCaller: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<AdminCaller | undefined>
  // Issue #8505 (Part 2): fail-soft, best-effort per-user credit-balance
  // projection into Khala Sync (`scope.user.<userId>`) — threaded through to
  // `grantAdminCredit`/`clawbackAdminCredit`'s own `recordCreditBalanceProjection`
  // seam. Optional; absent in tests and any deployment without the Khala
  // Sync binding, which grant/claw back exactly as before.
  recordCreditBalanceProjection?: (
    env: Bindings,
  ) => AdminCreditGrantDeps['recordCreditBalanceProjection']
}>

const requireAdmin = async <Bindings>(
  dependencies: AdminCreditsRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Promise<AdminCaller | undefined> =>
  dependencies.requireAdminCaller(request, env, ctx)

// ----------------------------------------------------------------------------
// User resolution — by userId directly, or by GitHub login.
// ----------------------------------------------------------------------------

type ResolvedUserRow = Readonly<{
  user_id: string
  display_name: string
  primary_email: string | null
  github_username: string | null
}>

export const resolveAdminCreditsTargetUser = async (
  db: D1Database,
  input: Readonly<{ userId?: string | undefined; githubLogin?: string | undefined }>,
): Promise<ResolvedUserRow | null> => {
  const userId = input.userId?.trim()
  if (userId !== undefined && userId.length > 0) {
    const row = await db
      .prepare(
        `SELECT users.id AS user_id,
                users.display_name,
                users.primary_email,
                (SELECT auth_identities.provider_username
                   FROM auth_identities
                  WHERE auth_identities.user_id = users.id
                    AND auth_identities.provider = 'github'
                    AND auth_identities.deleted_at IS NULL
                  LIMIT 1) AS github_username
           FROM users
          WHERE users.id = ?
            AND users.deleted_at IS NULL
          LIMIT 1`,
      )
      .bind(userId)
      .first<ResolvedUserRow>()
    return row ?? null
  }

  const githubLogin = input.githubLogin?.trim()
  if (githubLogin !== undefined && githubLogin.length > 0) {
    const row = await db
      .prepare(
        `SELECT users.id AS user_id,
                users.display_name,
                users.primary_email,
                auth_identities.provider_username AS github_username
           FROM auth_identities
           JOIN users ON users.id = auth_identities.user_id
          WHERE auth_identities.provider = 'github'
            AND auth_identities.provider_username = ?
            AND auth_identities.deleted_at IS NULL
            AND users.deleted_at IS NULL
          LIMIT 1`,
      )
      .bind(githubLogin)
      .first<ResolvedUserRow>()
    return row ?? null
  }

  return null
}

const targetUserNotFound = (): HttpResponse =>
  noStoreJsonResponse(
    { error: 'target_user_not_found', messageSafe: 'No user matches the given userId/githubLogin.' },
    { status: 404 },
  )

// ----------------------------------------------------------------------------
// GET /api/admin/credits/users — recent signups + grant status
// ----------------------------------------------------------------------------

type RecentSignupRow = Readonly<{
  user_id: string
  display_name: string
  primary_email: string | null
  github_username: string | null
  created_at: string
  has_signup_credit_grant: number
  has_admin_credit_grant: number
  balance_msat: number | null
}>

const routeListUsers = async <Bindings>(
  dependencies: AdminCreditsRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Promise<HttpResponse> => {
  if (request.method !== 'GET') return methodNotAllowed(['GET'])
  const url = new URL(request.url)
  const limit = Math.max(1, Math.min(100, optionalInteger(url.searchParams.get('limit') ?? undefined) ?? 25))
  const query = optionalString(url.searchParams.get('query') ?? undefined)?.trim()
  const db = dependencies.db(env)

  const like = query === undefined || query.length === 0 ? null : `%${query}%`

  // Single query, no N+1: the balance join reads agent_balances directly by
  // the same `agent:<userId>` actor-ref convention `agentRefForUser` uses.
  const result = await db
    .prepare(
      `SELECT users.id AS user_id,
              users.display_name,
              users.primary_email,
              (SELECT auth_identities.provider_username
                 FROM auth_identities
                WHERE auth_identities.user_id = users.id
                  AND auth_identities.provider = 'github'
                  AND auth_identities.deleted_at IS NULL
                LIMIT 1) AS github_username,
              users.created_at,
              EXISTS (
                SELECT 1 FROM github_signup_credit_grants
                 WHERE github_signup_credit_grants.user_id = users.id
              ) AS has_signup_credit_grant,
              EXISTS (
                SELECT 1 FROM admin_credit_grants
                 WHERE admin_credit_grants.user_id = users.id
              ) AS has_admin_credit_grant,
              agent_balances.balance_msat AS balance_msat
         FROM users
         LEFT JOIN agent_balances
           ON agent_balances.actor_ref = 'agent:' || users.id
        WHERE users.kind = 'human'
          AND users.deleted_at IS NULL
          AND (
            ? IS NULL
            OR users.id LIKE ?
            OR users.display_name LIKE ?
            OR (SELECT auth_identities.provider_username
                  FROM auth_identities
                 WHERE auth_identities.user_id = users.id
                   AND auth_identities.provider = 'github'
                   AND auth_identities.deleted_at IS NULL
                 LIMIT 1) LIKE ?
          )
        ORDER BY users.created_at DESC
        LIMIT ?`,
    )
    .bind(like, like, like, like, limit)
    .all<RecentSignupRow>()

  return noStoreJsonResponse({
    ok: true,
    users: result.results.map(row => ({
      balanceUsdCents: msatToUsdCentsRound(row.balance_msat ?? 0),
      createdAt: row.created_at,
      displayName: row.display_name,
      githubLogin: row.github_username,
      hasAdminCreditGrant: Number(row.has_admin_credit_grant) === 1,
      hasSignupCreditGrant: Number(row.has_signup_credit_grant) === 1,
      primaryEmail: row.primary_email,
      userId: row.user_id,
    })),
  })
}

// ----------------------------------------------------------------------------
// GET /api/admin/credits/balance
// ----------------------------------------------------------------------------

const routeBalance = async <Bindings>(
  dependencies: AdminCreditsRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Promise<HttpResponse> => {
  if (request.method !== 'GET') return methodNotAllowed(['GET'])
  const url = new URL(request.url)
  const db = dependencies.db(env)
  const target = await resolveAdminCreditsTargetUser(db, {
    githubLogin: url.searchParams.get('githubLogin') ?? undefined,
    userId: url.searchParams.get('userId') ?? undefined,
  })
  if (target === null) return targetUserNotFound()

  const balance = await readAgentBalance(db, agentRefForUser(target.user_id))

  return noStoreJsonResponse({
    ok: true,
    user: {
      displayName: target.display_name,
      githubLogin: target.github_username,
      userId: target.user_id,
    },
    balance: {
      availableUsdCents: msatToUsdCentsRound(balance?.availableMsat ?? 0),
      balanceUsdCents: msatToUsdCentsRound(balance?.balanceMsat ?? 0),
      // Display-only USD-cents projection (see usd-msat-conversion.ts) — the
      // exact accounting source of truth stays msat.
      availableMsat: balance?.availableMsat ?? 0,
      balanceMsat: balance?.balanceMsat ?? 0,
      usdCreditMsat: balance?.usdCreditMsat ?? 0,
      bitcoinWithdrawableMsat: balance?.bitcoinWithdrawableMsat ?? 0,
    },
  })
}

// ----------------------------------------------------------------------------
// GET /api/admin/credits/history — merged grant history for one user
// ----------------------------------------------------------------------------

const routeHistory = async <Bindings>(
  dependencies: AdminCreditsRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Promise<HttpResponse> => {
  if (request.method !== 'GET') return methodNotAllowed(['GET'])
  const url = new URL(request.url)
  const db = dependencies.db(env)
  const target = await resolveAdminCreditsTargetUser(db, {
    githubLogin: url.searchParams.get('githubLogin') ?? undefined,
    userId: url.searchParams.get('userId') ?? undefined,
  })
  if (target === null) return targetUserNotFound()
  const limit = Math.max(1, Math.min(200, optionalInteger(url.searchParams.get('limit') ?? undefined) ?? 50))

  const [adminGrants, signupGrants] = await Promise.all([
    readAdminCreditGrantsForUser(db, target.user_id),
    readGithubSignupCreditGrantsForUser(db, target.user_id),
  ])

  type HistoryEntry = Readonly<{
    kind: 'admin_grant' | 'signup_grant'
    amountUsdCents: number
    reason: string
    receiptRef: string
    occurredAt: string
  }>

  const entries: Array<HistoryEntry> = [
    ...adminGrants.map(
      (grant): HistoryEntry => ({
        amountUsdCents: grant.amountUsdCents,
        kind: 'admin_grant',
        occurredAt: grant.createdAt,
        reason: grant.reason,
        receiptRef: grant.creditReceiptRef,
      }),
    ),
    ...signupGrants.map(
      (grant): HistoryEntry => ({
        amountUsdCents: grant.amountUsdCents,
        kind: 'signup_grant',
        occurredAt: grant.createdAt,
        reason: '$10 GitHub signup credit',
        receiptRef: grant.creditReceiptRef,
      }),
    ),
  ]
    .sort((a, b) => b.occurredAt.localeCompare(a.occurredAt))
    .slice(0, limit)

  return noStoreJsonResponse({
    ok: true,
    user: {
      displayName: target.display_name,
      githubLogin: target.github_username,
      userId: target.user_id,
    },
    // Honest gap: this merges receipted grant events only (admin +
    // signup); it does not yet include raw inference-charge activity —
    // that's the same unpaginated `pay_ins` shape `agent-balance-routes.ts`
    // already exposes for the user's own balance view, and AIUR-3's ops
    // view is the natural home for the broader "what did they run and did
    // it charge correctly" question across all users.
    history: entries,
  })
}

// ----------------------------------------------------------------------------
// GET /api/admin/credits/recent-grants — ledger view across all users
// ----------------------------------------------------------------------------

const routeRecentGrants = async <Bindings>(
  dependencies: AdminCreditsRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Promise<HttpResponse> => {
  if (request.method !== 'GET') return methodNotAllowed(['GET'])
  const url = new URL(request.url)
  const limit = Math.max(1, Math.min(200, optionalInteger(url.searchParams.get('limit') ?? undefined) ?? 50))
  const grants = await readRecentAdminCreditGrants(dependencies.db(env), limit)

  return noStoreJsonResponse({
    ok: true,
    grants: grants.map(grant => ({
      amountUsdCents: grant.amountUsdCents,
      createdAt: grant.createdAt,
      grantedByUserId: grant.grantedByUserId,
      grantRef: grant.grantRef,
      reason: grant.reason,
      receiptRef: grant.creditReceiptRef,
      userId: grant.userId,
    })),
  })
}

// ----------------------------------------------------------------------------
// POST /api/admin/credits/grant
// ----------------------------------------------------------------------------

const grantRefusalStatus = (
  reason: Extract<AdminCreditGrantOutcome, { ok: false }>['reason'],
): number => (reason === 'asset_boundary_violation' ? 403 : 400)

const routeGrant = async <Bindings>(
  dependencies: AdminCreditsRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  caller: AdminCaller,
): Promise<HttpResponse> => {
  if (request.method !== 'POST') return methodNotAllowed(['POST'])

  let body: Record<string, unknown>
  try {
    body = await readJsonObject(request)
  } catch {
    return noStoreJsonResponse({ error: 'invalid_request' }, { status: 400 })
  }

  const db = dependencies.db(env)
  const target = await resolveAdminCreditsTargetUser(db, {
    githubLogin: optionalString(body.githubLogin),
    userId: optionalString(body.userId),
  })
  if (target === null) return targetUserNotFound()

  const grantRef = optionalString(body.grantRef)
  const amountUsdCents = optionalInteger(body.amountUsdCents)
  const reason = optionalString(body.reason)

  if (grantRef === undefined || grantRef.length === 0) {
    return noStoreJsonResponse(
      { error: 'invalid_request', reason: 'grantRef is required (client-generated, one per grant attempt).' },
      { status: 400 },
    )
  }
  if (amountUsdCents === undefined) {
    return noStoreJsonResponse(
      { error: 'invalid_request', reason: 'amountUsdCents is required.' },
      { status: 400 },
    )
  }
  if (reason === undefined || reason.trim().length === 0) {
    return noStoreJsonResponse(
      { error: 'invalid_request', reason: 'reason is required.' },
      { status: 400 },
    )
  }

  const deps: AdminCreditGrantDeps = {
    db,
    ...(dependencies.nowIso === undefined ? {} : { nowIso: dependencies.nowIso }),
    ...(dependencies.recordCreditBalanceProjection === undefined
      ? {}
      : { recordCreditBalanceProjection: dependencies.recordCreditBalanceProjection(env) }),
  }

  const outcome = await Effect.runPromise(
    grantAdminCredit(
      {
        amountUsdCents,
        grantedByUserId: caller.userId,
        grantRef,
        reason,
        userId: target.user_id,
      },
      deps,
    ),
  )

  if (!outcome.ok) {
    return noStoreJsonResponse(
      { error: 'grant_refused', ok: false, reason: outcome.reason, messageSafe: outcome.message },
      { status: grantRefusalStatus(outcome.reason) },
    )
  }

  return noStoreJsonResponse(
    {
      ok: true,
      alreadyGranted: outcome.alreadyGranted,
      grantedCents: outcome.grantedCents,
      grantedMsat: outcome.grantedMsat,
      grantRef: outcome.grantRef,
      receiptRef: outcome.receiptRef,
      user: { userId: target.user_id, githubLogin: target.github_username },
    },
    { status: outcome.alreadyGranted ? 200 : 201 },
  )
}

// ----------------------------------------------------------------------------
// POST /api/admin/credits/clawback
// ----------------------------------------------------------------------------

const routeClawback = async <Bindings>(
  dependencies: AdminCreditsRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Promise<HttpResponse> => {
  if (request.method !== 'POST') return methodNotAllowed(['POST'])

  let body: Record<string, unknown>
  try {
    body = await readJsonObject(request)
  } catch {
    return noStoreJsonResponse({ error: 'invalid_request' }, { status: 400 })
  }

  const db = dependencies.db(env)
  const target = await resolveAdminCreditsTargetUser(db, {
    githubLogin: optionalString(body.githubLogin),
    userId: optionalString(body.userId),
  })
  if (target === null) return targetUserNotFound()

  const clawbackRef = optionalString(body.clawbackRef)
  const amountUsdCents = optionalInteger(body.amountUsdCents)
  const reason = optionalString(body.reason)

  if (clawbackRef === undefined || clawbackRef.length === 0) {
    return noStoreJsonResponse(
      { error: 'invalid_request', reason: 'clawbackRef is required (client-generated, one per clawback attempt).' },
      { status: 400 },
    )
  }
  if (amountUsdCents === undefined || amountUsdCents <= 0) {
    return noStoreJsonResponse(
      { error: 'invalid_request', reason: 'amountUsdCents must be a positive number.' },
      { status: 400 },
    )
  }
  if (reason === undefined || reason.trim().length === 0) {
    return noStoreJsonResponse(
      { error: 'invalid_request', reason: 'reason is required.' },
      { status: 400 },
    )
  }

  const deps: AdminCreditClawbackDeps = {
    db,
    ...(dependencies.nowIso === undefined ? {} : { nowIso: dependencies.nowIso }),
    ...(dependencies.recordCreditBalanceProjection === undefined
      ? {}
      : { recordCreditBalanceProjection: dependencies.recordCreditBalanceProjection(env) }),
  }

  const outcome = await Effect.runPromise(
    clawbackAdminCredit(
      { amountUsdCents, clawbackRef, reason, userId: target.user_id },
      deps,
    ),
  )

  return noStoreJsonResponse({
    ok: true,
    clawedBack: outcome.clawedBack,
    insufficientBalance: outcome.insufficientBalance,
    receiptRef: outcome.receiptRef,
    user: { userId: target.user_id, githubLogin: target.github_username },
  })
}

// ----------------------------------------------------------------------------
// Router entry point
// ----------------------------------------------------------------------------

export const makeAdminCreditsRoutes = <Bindings>(
  dependencies: AdminCreditsRouteDependencies<Bindings>,
) => ({
  handleAdminCreditsUsersApi: async (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Promise<HttpResponse> => {
    const caller = await requireAdmin(dependencies, request, env, ctx)
    if (caller === undefined) return unauthorized()
    return routeListUsers(dependencies, request, env)
  },

  handleAdminCreditsBalanceApi: async (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Promise<HttpResponse> => {
    const caller = await requireAdmin(dependencies, request, env, ctx)
    if (caller === undefined) return unauthorized()
    return routeBalance(dependencies, request, env)
  },

  handleAdminCreditsHistoryApi: async (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Promise<HttpResponse> => {
    const caller = await requireAdmin(dependencies, request, env, ctx)
    if (caller === undefined) return unauthorized()
    return routeHistory(dependencies, request, env)
  },

  handleAdminCreditsRecentGrantsApi: async (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Promise<HttpResponse> => {
    const caller = await requireAdmin(dependencies, request, env, ctx)
    if (caller === undefined) return unauthorized()
    return routeRecentGrants(dependencies, request, env)
  },

  handleAdminCreditsGrantApi: async (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Promise<HttpResponse> => {
    const caller = await requireAdmin(dependencies, request, env, ctx)
    if (caller === undefined) return unauthorized()
    return routeGrant(dependencies, request, env, caller)
  },

  handleAdminCreditsClawbackApi: async (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Promise<HttpResponse> => {
    const caller = await requireAdmin(dependencies, request, env, ctx)
    if (caller === undefined) return unauthorized()
    return routeClawback(dependencies, request, env)
  },
})
