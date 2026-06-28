import { Effect, Match as M, Schema as S } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { openAgentsDatabase } from './runtime'
import { currentIsoTimestamp } from './runtime-primitives'

type OperatorArtanisDashboardEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>
type HttpResponse = globalThis.Response

type OperatorArtanisDashboardSession = Readonly<{
  user: Readonly<{
    email: string
    userId: string
  }>
}>

export type OperatorArtanisDashboardDependencies<
  Session extends OperatorArtanisDashboardSession,
  Bindings extends OperatorArtanisDashboardEnv,
> = Readonly<{
  appendRefreshedSessionCookies: (
    response: HttpResponse,
    session: Session,
  ) => HttpResponse
  isOpenAgentsAdminEmail: (email: string) => boolean
  requireAdminApiToken?: (
    request: Request,
    env: Bindings,
  ) => Promise<boolean>
  requireBrowserSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
}>

class OperatorArtanisDashboardUnauthorized extends S.TaggedErrorClass<OperatorArtanisDashboardUnauthorized>()(
  'OperatorArtanisDashboardUnauthorized',
  {},
) {}

class OperatorArtanisDashboardForbidden extends S.TaggedErrorClass<OperatorArtanisDashboardForbidden>()(
  'OperatorArtanisDashboardForbidden',
  {},
) {}

class OperatorArtanisDashboardSessionError extends S.TaggedErrorClass<OperatorArtanisDashboardSessionError>()(
  'OperatorArtanisDashboardSessionError',
  { error: S.Defect },
) {}

class OperatorArtanisDashboardStorageError extends S.TaggedErrorClass<OperatorArtanisDashboardStorageError>()(
  'OperatorArtanisDashboardStorageError',
  { error: S.Defect },
) {}

type OperatorArtanisDashboardError =
  | OperatorArtanisDashboardForbidden
  | OperatorArtanisDashboardSessionError
  | OperatorArtanisDashboardStorageError
  | OperatorArtanisDashboardUnauthorized

type ArtanisThreadRow = Readonly<{
  thread_ref: string
  caller_id: string
  caller_kind: string
  subject_agent_ref: string
  subject_agent_kind: string
  title: string
  status: string
  source_ref: string | null
  last_message_at: string
  created_at: string
  updated_at: string
  message_count: number
}>

type ArtanisMessageRow = Readonly<{
  message_ref: string
  thread_ref: string
  caller_id: string
  author_id: string
  author_kind: string
  body: string
  created_at: string
}>

type OperatorAccountUsageRow = Readonly<{
  accountRefHash: string
  provider: string
  isRateLimited: boolean
  cooldownExpiresAt: string | null
  hourlyCap: number | null
  hourlyUsage: number | null
  weeklyCap: number | null
  weeklyUsage: number | null
  manualResetsRemaining: number | null
}>

type OperatorAccountStatusRow = Readonly<{
  provider_account_ref: string
  provider: string
  cooldown_until: string | null
  recent_failure_class: string | null
}>

const boundedPercent = (usage: number | null, cap: number | null): number => {
  if (usage === null || cap === null || cap <= 0) {
    return 0
  }

  return Math.max(0, Math.min(100, Math.round((usage / cap) * 100)))
}

const remainingTokens = (usage: number | null, cap: number | null): number | null =>
  usage === null || cap === null ? null : Math.max(0, cap - usage)

const accountUsageWindow = (
  label: 'hourly' | 'weekly',
  usage: number | null,
  cap: number | null,
) => ({
  cap,
  label,
  percentUsed: boundedPercent(usage, cap),
  remaining: remainingTokens(usage, cap),
  used: usage,
})

export const operatorAccountUsageProjection = (
  rows: ReadonlyArray<OperatorAccountUsageRow>,
  observedAt: string,
) => ({
  accounts: rows.map(row => ({
    accountRefHash: row.accountRefHash,
    cooldownExpiresAt: row.cooldownExpiresAt,
    isRateLimited: row.isRateLimited,
    manualResetsRemaining: row.manualResetsRemaining,
    provider: row.provider,
    windows: [
      accountUsageWindow('hourly', row.hourlyUsage, row.hourlyCap),
      accountUsageWindow('weekly', row.weeklyUsage, row.weeklyCap),
    ],
  })),
  observedAt,
})

const routeErrorResponse = (error: OperatorArtanisDashboardError) =>
  M.value(error).pipe(
    M.tags({
      OperatorArtanisDashboardForbidden: () =>
        noStoreJsonResponse({ error: 'forbidden' }, { status: 403 }),
      OperatorArtanisDashboardSessionError: () =>
        noStoreJsonResponse({ error: 'session_error' }, { status: 500 }),
      OperatorArtanisDashboardStorageError: () =>
        noStoreJsonResponse(
          { error: 'artanis_operator_dashboard_storage_error' },
          { status: 500 },
        ),
      OperatorArtanisDashboardUnauthorized: () =>
        noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 }),
    }),
    M.exhaustive,
  )

const requireAdminSession = <
  Session extends OperatorArtanisDashboardSession,
  Bindings extends OperatorArtanisDashboardEnv,
>(
  dependencies: OperatorArtanisDashboardDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
) =>
  Effect.gen(function* () {
    const requireAdminApiToken = dependencies.requireAdminApiToken

    if (requireAdminApiToken !== undefined) {
      const hasAdminApiToken = yield* Effect.tryPromise({
        catch: error => new OperatorArtanisDashboardSessionError({ error }),
        try: () => requireAdminApiToken(request, env),
      })

      if (hasAdminApiToken) {
        return {
          user: {
            email: 'chris@openagents.com',
            userId: 'github:14167547',
          },
        } as Session
      }
    }

    const session = yield* Effect.tryPromise({
      catch: error => new OperatorArtanisDashboardSessionError({ error }),
      try: () => dependencies.requireBrowserSession(request, env, ctx),
    })

    if (session === undefined) {
      return yield* new OperatorArtanisDashboardUnauthorized({})
    }

    if (!dependencies.isOpenAgentsAdminEmail(session.user.email)) {
      return yield* new OperatorArtanisDashboardForbidden({})
    }

    return session
  })

const cleanQueryValue = (url: URL, key: string): string | undefined => {
  const value = url.searchParams.get(key)?.trim()
  return value === undefined || value === '' ? undefined : value
}

const threadProjection = (row: ArtanisThreadRow) => ({
  callerId: row.caller_id,
  callerKind: row.caller_kind,
  createdAt: row.created_at,
  lastMessageAt: row.last_message_at,
  messageCount: row.message_count,
  sourceRef: row.source_ref,
  status: row.status,
  subjectAgentKind: row.subject_agent_kind,
  subjectAgentRef: row.subject_agent_ref,
  threadRef: row.thread_ref,
  title: row.title,
  updatedAt: row.updated_at,
})

const messageProjection = (row: ArtanisMessageRow) => ({
  authorId: row.author_id,
  authorKind: row.author_kind,
  body: row.body,
  callerId: row.caller_id,
  createdAt: row.created_at,
  messageRef: row.message_ref,
  threadRef: row.thread_ref,
})

const normalizeAccountProvider = (provider: string): string =>
  provider === 'chatgpt_codex' ? 'codex' : provider

const statusRowToUsageRow = (
  row: OperatorAccountStatusRow,
  now: string,
): OperatorAccountUsageRow => {
  const cooldownExpiresAt =
    row.cooldown_until !== null && row.cooldown_until > now
      ? row.cooldown_until
      : null

  return {
    accountRefHash: row.provider_account_ref,
    cooldownExpiresAt,
    hourlyCap: null,
    hourlyUsage: null,
    isRateLimited:
      cooldownExpiresAt !== null || row.recent_failure_class === 'rate_limited',
    manualResetsRemaining: null,
    provider: normalizeAccountProvider(row.provider),
    weeklyCap: null,
    weeklyUsage: null,
  }
}

const listThreads = (
  db: D1Database,
  callerId: string | undefined,
): Promise<ReadonlyArray<ArtanisThreadRow>> => {
  const baseSql = `SELECT
      artanis_threads.thread_ref,
      artanis_threads.caller_id,
      artanis_threads.caller_kind,
      artanis_threads.subject_agent_ref,
      artanis_threads.subject_agent_kind,
      artanis_threads.title,
      artanis_threads.status,
      artanis_threads.source_ref,
      artanis_threads.last_message_at,
      artanis_threads.created_at,
      artanis_threads.updated_at,
      COUNT(artanis_messages.message_ref) AS message_count
    FROM artanis_threads
    LEFT JOIN artanis_messages
      ON artanis_messages.thread_ref = artanis_threads.thread_ref`
  const suffix = ` GROUP BY artanis_threads.thread_ref
    ORDER BY artanis_threads.last_message_at DESC
    LIMIT 100`

  const query =
    callerId === undefined
      ? db.prepare(`${baseSql}${suffix}`)
      : db
          .prepare(`${baseSql} WHERE artanis_threads.caller_id = ?${suffix}`)
          .bind(callerId)

  return query.all<ArtanisThreadRow>().then(rows => rows.results ?? [])
}

const listMessages = (
  db: D1Database,
  threadRef: string | undefined,
): Promise<ReadonlyArray<ArtanisMessageRow>> => {
  if (threadRef === undefined) {
    return Promise.resolve([])
  }

  return db
    .prepare(
      `SELECT message_ref, thread_ref, caller_id, author_id, author_kind, body, created_at
         FROM artanis_messages
        WHERE thread_ref = ?
        ORDER BY created_at ASC
        LIMIT 400`,
    )
    .bind(threadRef)
    .all<ArtanisMessageRow>()
    .then(rows => rows.results ?? [])
}

const listOperatorAccountStatusRows = (
  db: D1Database,
  userId: string,
): Promise<ReadonlyArray<OperatorAccountStatusRow>> =>
  db
    .prepare(
      `SELECT provider_account_ref,
              provider,
              cooldown_until,
              recent_failure_class
         FROM provider_accounts
        WHERE user_id = ?
          AND deleted_at IS NULL
          AND provider IN ('chatgpt_codex', 'claude')
        ORDER BY
          CASE provider WHEN 'chatgpt_codex' THEN 0 ELSE 1 END,
          provider_account_ref ASC
        LIMIT 200`,
    )
    .bind(userId)
    .all<OperatorAccountStatusRow>()
    .then(rows => rows.results ?? [])

const operatorAccountStatusProjection = (
  rows: ReadonlyArray<OperatorAccountStatusRow>,
  observedAt: string,
) =>
  operatorAccountUsageProjection(
    rows.map(row => statusRowToUsageRow(row, observedAt)),
    observedAt,
  )

const readAccountRefHash = (request: Request): Promise<string | undefined> =>
  request
    .json()
    .then(value => {
      if (typeof value !== 'object' || value === null) {
        return undefined
      }

      const raw = (value as Record<string, unknown>).accountRefHash
      return typeof raw === 'string' && raw.trim() !== ''
        ? raw.trim()
        : undefined
    })
    .catch(() => undefined)

const resetOperatorAccountCooldown = async (
  db: D1Database,
  input: Readonly<{
    accountRefHash: string
    now: string
    userId: string
  }>,
): Promise<boolean> => {
  const result = await db
    .prepare(
      `UPDATE provider_accounts
          SET cooldown_until = NULL,
              recent_failure_class = NULL,
              last_failed_launch_at = ?
        WHERE user_id = ?
          AND provider_account_ref = ?
          AND deleted_at IS NULL`,
    )
    .bind(input.now, input.userId, input.accountRefHash)
    .run()

  return (result.meta?.changes ?? 0) > 0
}

export const makeOperatorArtanisDashboardRoutes = <
  Session extends OperatorArtanisDashboardSession,
  Bindings extends OperatorArtanisDashboardEnv,
>(
  dependencies: OperatorArtanisDashboardDependencies<Session, Bindings>,
) => ({
  routeOperatorArtanisDashboardRequest: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Effect.Effect<globalThis.Response> | undefined => {
    const url = new URL(request.url)

    const isDashboard = url.pathname === '/api/operator/artanis/dashboard'
    const isAccountsStatus = url.pathname === '/api/operator/accounts/status'
    const isAccountsReset = url.pathname === '/api/operator/accounts/reset'

    if (!isDashboard && !isAccountsStatus && !isAccountsReset) {
      return undefined
    }

    if ((isDashboard || isAccountsStatus) && request.method !== 'GET') {
      return Effect.succeed(methodNotAllowed(['GET']))
    }

    if (isAccountsReset && request.method !== 'POST') {
      return Effect.succeed(methodNotAllowed(['POST']))
    }

    return Effect.gen(function* () {
      const session = yield* requireAdminSession(dependencies, request, env, ctx)
      const db = openAgentsDatabase(env)
      const now = currentIsoTimestamp()

      if (isAccountsStatus) {
        const rows = yield* Effect.tryPromise({
          catch: error => new OperatorArtanisDashboardStorageError({ error }),
          try: () => listOperatorAccountStatusRows(db, session.user.userId),
        })

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse(operatorAccountStatusProjection(rows, now)),
          session,
        )
      }

      if (isAccountsReset) {
        const accountRefHash = yield* Effect.tryPromise({
          catch: error => new OperatorArtanisDashboardStorageError({ error }),
          try: () => readAccountRefHash(request),
        })

        if (accountRefHash === undefined) {
          return noStoreJsonResponse(
            { error: 'bad_request', reason: 'accountRefHash is required' },
            { status: 400 },
          )
        }

        const didReset = yield* Effect.tryPromise({
          catch: error => new OperatorArtanisDashboardStorageError({ error }),
          try: () =>
            resetOperatorAccountCooldown(db, {
              accountRefHash,
              now,
              userId: session.user.userId,
            }),
        })

        if (!didReset) {
          return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
        }

        const rows = yield* Effect.tryPromise({
          catch: error => new OperatorArtanisDashboardStorageError({ error }),
          try: () => listOperatorAccountStatusRows(db, session.user.userId),
        })

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({
            ok: true,
            resetAt: now,
            status: operatorAccountStatusProjection(rows, now),
          }),
          session,
        )
      }

      const callerId = cleanQueryValue(url, 'caller_id')
      const requestedThreadRef = cleanQueryValue(url, 'thread_ref')

      const threads = yield* Effect.tryPromise({
        catch: error => new OperatorArtanisDashboardStorageError({ error }),
        try: () => listThreads(db, callerId),
      })
      const selectedThread =
        threads.find(thread => thread.thread_ref === requestedThreadRef) ??
        threads[0] ??
        null
      const messages = yield* Effect.tryPromise({
        catch: error => new OperatorArtanisDashboardStorageError({ error }),
        try: () => listMessages(db, selectedThread?.thread_ref),
      })

      return dependencies.appendRefreshedSessionCookies(
        noStoreJsonResponse({
          accountUsage: operatorAccountUsageProjection([], now),
          callerIdFilter: callerId ?? null,
          dashboardRef: 'operator.artanis.dashboard',
          messages: messages.map(messageProjection),
          selectedThread:
            selectedThread === null ? null : threadProjection(selectedThread),
          threads: threads.map(threadProjection),
        }),
        session,
      )
    }).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
  },
})
