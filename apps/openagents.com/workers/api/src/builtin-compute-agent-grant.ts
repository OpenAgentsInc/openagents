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

import type { InferenceEntitlementsMirror } from './inference-entitlements-store'
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
      resetsAt: string
      sessionsRemaining: number
      dailyTokenCeiling: number
    }>

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
      resetsAt,
      sessionsRemaining: 0,
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

// KS-8.9 (#8320): optional fire-safe Postgres dual-write mirror for the
// quota-event table. NOTE: the companion `token_usage_events` row written in
// the same batch belongs to the KS-8.2 token-ledger domain and is NOT
// mirrored here (documented in the KS-8.9 decommission follow-up).
export const makeD1BuiltinComputeAgentStore = (
  db: D1Database,
  mirror?: InferenceEntitlementsMirror | undefined,
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
    mirror?.([
      {
        kind: 'write',
        row: {
          actor_user_id: input.quotaEvent.actorUserId,
          budget_class: input.quotaEvent.budgetClass,
          created_at: input.quotaEvent.createdAt,
          grant_ref: input.quotaEvent.grantRef,
          id: input.quotaEvent.id,
          provider: input.quotaEvent.provider,
          session_budget_seconds: input.quotaEvent.sessionBudgetSeconds,
          session_units: input.quotaEvent.sessionUnits,
          token_ceiling: input.quotaEvent.tokenCeiling,
        },
        table: 'builtin_compute_agent_quota_events',
      },
    ])
  },
})
