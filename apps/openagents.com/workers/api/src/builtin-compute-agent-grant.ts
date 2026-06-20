// Built-in hosted-compute (Gemini) agent grant broker.
//
// A no-key user's built-in agent needs a hosted-Gemini grant WITHOUT ever
// seeing the shared hosted key and WITHOUT being drainable. This module gates
// a keyless grant on a conservative per-user free-tier daily budget and the
// presence of a configured hosted key. It NEVER returns or logs the raw key:
// the grant it hands back is the same redacted, secret-ref-only materialization
// the existing omega Gemini broker uses, so a runner resolves the actual key
// through the worker-secret broker path, not through this response.
//
// COST/SECURITY-SENSITIVE: this route gates access to a shared hosted Gemini
// key. It is inert by default — if the hosted key env is not set, it grants
// nothing.

import {
  compactRandomId,
  currentDate,
  currentIsoTimestamp,
  isoTimestampAfterIso,
  utcStartOfDayIsoTimestamp,
} from './runtime-primitives'

export const BUILTIN_COMPUTE_AGENT_PRODUCER_SYSTEM =
  'autopilot.builtin_compute_agent'
export const BUILTIN_COMPUTE_AGENT_SOURCE_ROUTE = 'autopilot_builtin_compute'
export const BUILTIN_COMPUTE_AGENT_BUDGET_CLASS = 'free_tier'
export const BUILTIN_COMPUTE_AGENT_PROVIDER = 'google_gemini'

export const BUILTIN_COMPUTE_AGENT_GRANT_ENDPOINT =
  '/api/provider-accounts/google-gemini/grants/builtin'

const GOOGLE_GEMINI_SECRET_REF =
  'provider-account://google-gemini/worker-secret/GEMINI_API_KEY'
const GOOGLE_GEMINI_PROVIDER_ACCOUNT_REF =
  'provider-account_google_gemini_worker_secret'

// Conservative free-tier defaults. Keep these small: this is shared,
// owner-funded hosted compute.
export const BUILTIN_COMPUTE_AGENT_FREE_DAILY_SESSIONS = 3
export const BUILTIN_COMPUTE_AGENT_SESSION_BUDGET_SECONDS = 600
export const BUILTIN_COMPUTE_AGENT_DAILY_TOKEN_CEILING = 1_000_000
// A grant is short-lived; the runner re-requests when it expires.
const BUILTIN_COMPUTE_AGENT_GRANT_TTL_MS = 1000 * 60 * 30

export type BuiltinComputeAgentQuotaPolicy = Readonly<{
  budgetClass: typeof BUILTIN_COMPUTE_AGENT_BUDGET_CLASS
  freeDailySessions: number
  sessionBudgetSeconds: number
  dailyTokenCeiling: number
}>

export const systemBuiltinComputeAgentQuotaPolicy: BuiltinComputeAgentQuotaPolicy =
  {
    budgetClass: BUILTIN_COMPUTE_AGENT_BUDGET_CLASS,
    dailyTokenCeiling: BUILTIN_COMPUTE_AGENT_DAILY_TOKEN_CEILING,
    freeDailySessions: BUILTIN_COMPUTE_AGENT_FREE_DAILY_SESSIONS,
    sessionBudgetSeconds: BUILTIN_COMPUTE_AGENT_SESSION_BUDGET_SECONDS,
  }

export type BuiltinComputeAgentRuntime = Readonly<{
  makeGrantRef: () => string
  makeQuotaEventId: () => string
  makeUsageEventId: () => string
  now: () => Date
  nowIso: () => string
}>

export const systemBuiltinComputeAgentRuntime: BuiltinComputeAgentRuntime = {
  makeGrantRef: () => compactRandomId('builtin_compute_grant'),
  makeQuotaEventId: () => compactRandomId('builtin_compute_quota'),
  makeUsageEventId: () => compactRandomId('builtin_compute_usage'),
  now: currentDate,
  nowIso: currentIsoTimestamp,
}

export type BuiltinComputeAgentSession = Readonly<{
  user: Readonly<{
    id: string
  }>
}>

export type BuiltinComputeAgentQuotaUsage = Readonly<{
  resetAt: string
  sessionsUsed: number
}>

export type BuiltinComputeAgentStore = Readonly<{
  countSessionsSince: (input: {
    actorUserId: string
    sinceIso: string
  }) => Promise<number>
  // Total metered tokens already attributed to this user's built-in compute
  // agent since the given instant. Used to enforce the bounded daily token
  // ceiling — the metered path — not just the daily session count. Returns 0
  // when no metered usage has been recorded yet.
  sumTokensSince: (input: {
    actorUserId: string
    sinceIso: string
  }) => Promise<number>
  recordGrant: (input: {
    quotaEvent: BuiltinComputeAgentQuotaEvent
    usageEvent: BuiltinComputeAgentUsageEvent
  }) => Promise<void>
}>

export type BuiltinComputeAgentQuotaEvent = Readonly<{
  actorUserId: string
  budgetClass: typeof BUILTIN_COMPUTE_AGENT_BUDGET_CLASS
  createdAt: string
  grantRef: string
  id: string
  provider: typeof BUILTIN_COMPUTE_AGENT_PROVIDER
  sessionBudgetSeconds: number
  sessionUnits: number
  tokenCeiling: number
}>

export type BuiltinComputeAgentUsageEvent = Readonly<{
  actorUserId: string
  grantRef: string
  id: string
  idempotencyKey: string
  observedAt: string
}>

export type BuiltinComputeAgentGrant = Readonly<{
  grantRef: string
  provider: typeof BUILTIN_COMPUTE_AGENT_PROVIDER
  providerAccountRef: string
  providerSecretRef: string
  runnerSessionId?: string
  expiresAt: number
  status: 'issued'
  budgetClass: typeof BUILTIN_COMPUTE_AGENT_BUDGET_CLASS
  freeAllowance: Readonly<{
    sessionsRemaining: number
    resetsAt: string
    sessionBudgetSeconds: number
    dailyTokenCeiling: number
    tokensRemaining: number
  }>
  materialization: Readonly<{
    kind: 'probe_gemini_api_key'
    provider: typeof BUILTIN_COMPUTE_AGENT_PROVIDER
    providerSecretRef: string
    target: Readonly<{ kind: 'env'; name: string }>
    homeIsolation: 'per_run'
    scrubAfterCloseout: true
  }>
}>

export type BuiltinComputeAgentGrantResult =
  | Readonly<{ kind: 'granted'; grant: BuiltinComputeAgentGrant }>
  | Readonly<{
      kind: 'not_configured'
    }>
  | Readonly<{
      kind: 'quota_exhausted'
      // Why the grant was denied: the user has spent today's free sessions, or
      // they have burned through today's metered token ceiling. Either denial
      // is a free-tier bound, not a fault.
      reason: 'sessions' | 'tokens'
      resetsAt: string
      sessionsRemaining: number
      dailyTokenCeiling: number
      tokensRemaining: number
    }>

// Result of the bounded daily token-ceiling check — the metered path. This is
// a pure decision over already-observed token usage; it neither issues grants
// nor records anything.
export type BuiltinComputeAgentTokenBudgetResult =
  | Readonly<{
      kind: 'within_budget'
      usedTokensToday: number
      tokensRemaining: number
      dailyTokenCeiling: number
      resetsAt: string
    }>
  | Readonly<{
      kind: 'budget_exhausted'
      usedTokensToday: number
      tokensRemaining: 0
      dailyTokenCeiling: number
      resetsAt: string
    }>

export type BuiltinComputeAgentTokenBudgetInput = Readonly<{
  usedTokensToday: number
  dailyTokenCeiling: number
  resetsAt: string
}>

// Pure, bounded daily token-ceiling decision. A built-in compute session is a
// capability on metered, owner-funded hosted compute — once the user has spent
// the day's token ceiling they are over budget until reset, regardless of how
// many free sessions remain. Negative/NaN inputs are clamped so a malformed
// counter can never silently widen the budget.
export const evaluateBuiltinComputeAgentTokenBudget = (
  input: BuiltinComputeAgentTokenBudgetInput,
): BuiltinComputeAgentTokenBudgetResult => {
  const ceiling = Math.max(
    0,
    Number.isFinite(input.dailyTokenCeiling) ? input.dailyTokenCeiling : 0,
  )
  const used = Math.max(
    0,
    Number.isFinite(input.usedTokensToday) ? input.usedTokensToday : ceiling,
  )
  const remaining = Math.max(0, ceiling - used)

  if (remaining <= 0) {
    return {
      dailyTokenCeiling: ceiling,
      kind: 'budget_exhausted',
      resetsAt: input.resetsAt,
      tokensRemaining: 0,
      usedTokensToday: used,
    }
  }

  return {
    dailyTokenCeiling: ceiling,
    kind: 'within_budget',
    resetsAt: input.resetsAt,
    tokensRemaining: remaining,
    usedTokensToday: used,
  }
}

export type BuiltinComputeAgentGrantInput = Readonly<{
  hostedKeyConfigured: boolean
  policy?: BuiltinComputeAgentQuotaPolicy | undefined
  providerAccountRef?: string | undefined
  runnerSessionId?: string | undefined
  runtime?: BuiltinComputeAgentRuntime | undefined
  session: BuiltinComputeAgentSession
  store: BuiltinComputeAgentStore
}>

const grantMaterialization = (): BuiltinComputeAgentGrant['materialization'] => ({
  homeIsolation: 'per_run',
  kind: 'probe_gemini_api_key',
  provider: BUILTIN_COMPUTE_AGENT_PROVIDER,
  providerSecretRef: GOOGLE_GEMINI_SECRET_REF,
  scrubAfterCloseout: true,
  target: {
    kind: 'env',
    name: 'GOOGLE_GENERATIVE_AI_API_KEY',
  },
})

export const executeBuiltinComputeAgentGrant = async (
  input: BuiltinComputeAgentGrantInput,
): Promise<BuiltinComputeAgentGrantResult> => {
  // Inert by default: with no hosted key configured, grant nothing.
  if (!input.hostedKeyConfigured) {
    return { kind: 'not_configured' }
  }

  const runtime = input.runtime ?? systemBuiltinComputeAgentRuntime
  const policy = input.policy ?? systemBuiltinComputeAgentQuotaPolicy
  const now = runtime.now()
  const nowIso = runtime.nowIso()
  const actorUserId = input.session.user.id
  const startOfDayIso = utcStartOfDayIsoTimestamp(nowIso)
  const resetsAt = isoTimestampAfterIso(
    startOfDayIso,
    24 * 60 * 60 * 1000,
  )

  const sessionsUsed = await input.store.countSessionsSince({
    actorUserId,
    sinceIso: startOfDayIso,
  })

  if (sessionsUsed >= policy.freeDailySessions) {
    return {
      dailyTokenCeiling: policy.dailyTokenCeiling,
      kind: 'quota_exhausted',
      reason: 'sessions',
      resetsAt,
      sessionsRemaining: 0,
      tokensRemaining: 0,
    }
  }

  // Bounded/metered path: even with free sessions left, a user who has burned
  // through today's metered token ceiling is over budget until reset. This
  // gates the shared, owner-funded hosted key on real token consumption, not
  // just session starts.
  const tokensUsed = await input.store.sumTokensSince({
    actorUserId,
    sinceIso: startOfDayIso,
  })
  const tokenBudget = evaluateBuiltinComputeAgentTokenBudget({
    dailyTokenCeiling: policy.dailyTokenCeiling,
    resetsAt,
    usedTokensToday: tokensUsed,
  })

  if (tokenBudget.kind === 'budget_exhausted') {
    return {
      dailyTokenCeiling: tokenBudget.dailyTokenCeiling,
      kind: 'quota_exhausted',
      reason: 'tokens',
      resetsAt,
      sessionsRemaining: Math.max(0, policy.freeDailySessions - sessionsUsed),
      tokensRemaining: 0,
    }
  }

  const grantRef = runtime.makeGrantRef()
  const expiresAt = now.getTime() + BUILTIN_COMPUTE_AGENT_GRANT_TTL_MS
  const sessionsRemaining = Math.max(
    0,
    policy.freeDailySessions - sessionsUsed - 1,
  )

  const quotaEvent: BuiltinComputeAgentQuotaEvent = {
    actorUserId,
    budgetClass: policy.budgetClass,
    createdAt: nowIso,
    grantRef,
    id: runtime.makeQuotaEventId(),
    provider: BUILTIN_COMPUTE_AGENT_PROVIDER,
    sessionBudgetSeconds: policy.sessionBudgetSeconds,
    sessionUnits: 1,
    tokenCeiling: policy.dailyTokenCeiling,
  }

  const usageEvent: BuiltinComputeAgentUsageEvent = {
    actorUserId,
    grantRef,
    id: runtime.makeUsageEventId(),
    idempotencyKey: `builtin_compute:${grantRef}`,
    observedAt: nowIso,
  }

  await input.store.recordGrant({ quotaEvent, usageEvent })

  const grant: BuiltinComputeAgentGrant = {
    budgetClass: policy.budgetClass,
    expiresAt,
    freeAllowance: {
      dailyTokenCeiling: policy.dailyTokenCeiling,
      resetsAt,
      sessionBudgetSeconds: policy.sessionBudgetSeconds,
      sessionsRemaining,
      tokensRemaining: tokenBudget.tokensRemaining,
    },
    grantRef,
    materialization: grantMaterialization(),
    provider: BUILTIN_COMPUTE_AGENT_PROVIDER,
    providerAccountRef:
      input.providerAccountRef ?? GOOGLE_GEMINI_PROVIDER_ACCOUNT_REF,
    providerSecretRef: GOOGLE_GEMINI_SECRET_REF,
    status: 'issued',
    ...(input.runnerSessionId === undefined
      ? {}
      : { runnerSessionId: input.runnerSessionId }),
  }

  return { grant, kind: 'granted' }
}

type BuiltinComputeAgentCountRow = Readonly<{ count: number | null }>

export const makeD1BuiltinComputeAgentStore = (
  db: D1Database,
): BuiltinComputeAgentStore => ({
  countSessionsSince: async input => {
    const row = await db
      .prepare(
        `SELECT COALESCE(SUM(session_units), 0) AS count
           FROM builtin_compute_agent_quota_events
          WHERE actor_user_id = ?
            AND created_at >= ?`,
      )
      .bind(input.actorUserId, input.sinceIso)
      .first<BuiltinComputeAgentCountRow>()

    return row?.count ?? 0
  },
  sumTokensSince: async input => {
    // Sum metered tokens attributed to this user's built-in compute agent
    // today. Scoped to the built-in-compute producer so a user's other,
    // key-bearing Gemini usage is never counted against this free-tier ceiling.
    const row = await db
      .prepare(
        `SELECT COALESCE(SUM(total_tokens), 0) AS count
           FROM token_usage_events
          WHERE actor_user_id = ?
            AND producer_system = ?
            AND source_route = ?
            AND observed_at >= ?`,
      )
      .bind(
        input.actorUserId,
        BUILTIN_COMPUTE_AGENT_PRODUCER_SYSTEM,
        BUILTIN_COMPUTE_AGENT_SOURCE_ROUTE,
        input.sinceIso,
      )
      .first<BuiltinComputeAgentCountRow>()

    return row?.count ?? 0
  },
  recordGrant: async input => {
    const observedAt = input.usageEvent.observedAt
    const safeMetadataJson = JSON.stringify({
      budgetClass: BUILTIN_COMPUTE_AGENT_BUDGET_CLASS,
      grantRef: input.usageEvent.grantRef,
    })

    await db.batch([
      db
        .prepare(
          `INSERT OR IGNORE INTO builtin_compute_agent_quota_events
             (id,
              actor_user_id,
              grant_ref,
              provider,
              budget_class,
              session_units,
              session_budget_seconds,
              token_ceiling,
              created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          input.quotaEvent.id,
          input.quotaEvent.actorUserId,
          input.quotaEvent.grantRef,
          input.quotaEvent.provider,
          input.quotaEvent.budgetClass,
          input.quotaEvent.sessionUnits,
          input.quotaEvent.sessionBudgetSeconds,
          input.quotaEvent.tokenCeiling,
          input.quotaEvent.createdAt,
        ),
      // Canonical token-usage ledger row attributing this grant to the
      // built-in compute agent producer and the free-tier budget class. Zero
      // bucketed token counts at issue time: the broker route records actual
      // tokens per inference. This row is the issue-time evidence that a
      // free-tier hosted-compute session was granted to this user. It carries
      // only safe refs — never the key, prompts, or completions.
      db
        .prepare(
          `INSERT OR IGNORE INTO token_usage_events (
            id,
            idempotency_key,
            observed_at,
            ingested_at,
            producer_system,
            source_route,
            actor_user_id,
            actor_team_id,
            account_ref,
            anonymized_source_ref,
            run_ref,
            session_ref,
            task_ref,
            repository_ref,
            provider,
            model,
            backend_profile,
            input_tokens,
            output_tokens,
            reasoning_tokens,
            cache_read_tokens,
            cache_write_5m_tokens,
            cache_write_1h_tokens,
            total_tokens,
            usage_truth,
            cost_amount,
            currency,
            leaderboard_eligible,
            privacy_opt_out,
            safe_metadata_json
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          input.usageEvent.id,
          input.usageEvent.idempotencyKey,
          observedAt,
          observedAt,
          BUILTIN_COMPUTE_AGENT_PRODUCER_SYSTEM,
          BUILTIN_COMPUTE_AGENT_SOURCE_ROUTE,
          input.usageEvent.actorUserId,
          null,
          GOOGLE_GEMINI_PROVIDER_ACCOUNT_REF,
          `builtin-compute:${input.usageEvent.grantRef}`,
          null,
          null,
          null,
          null,
          BUILTIN_COMPUTE_AGENT_PROVIDER,
          null,
          'worker_secret_gemini_api_key',
          0,
          0,
          0,
          0,
          0,
          0,
          0,
          'unknown',
          null,
          null,
          0,
          0,
          safeMetadataJson,
        ),
    ])
  },
})
