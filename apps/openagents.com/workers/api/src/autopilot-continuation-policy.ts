import { Effect, Match as M, Schema as S } from 'effect'

import { isoTimestampAfterIso, randomUuid } from './runtime-primitives'

export const AUTOPILOT_CONTINUATION_DEFAULT_MAX_PER_RUN = 2
export const AUTOPILOT_CONTINUATION_DEFAULT_MAX_PER_DAY = 10
export const AUTOPILOT_CONTINUATION_MAX_PER_RUN_LIMIT = 10
export const AUTOPILOT_CONTINUATION_MAX_PER_DAY_LIMIT = 50
export const AUTOPILOT_CONTINUATION_LOOKBACK_HOURS = 24

export class AutopilotContinuationPolicyError extends S.TaggedErrorClass<AutopilotContinuationPolicyError>()(
  'AutopilotContinuationPolicyError',
  {
    kind: S.Literals(['storage_error', 'validation_error']),
    reason: S.String,
  },
) {}

export type AutopilotContinuationPolicyRecord = Readonly<{
  createdAt: string
  enabled: boolean
  maxContinuationsPerDay: number
  maxContinuationsPerRun: number
  updatedAt: string
  userId: string
}>

export type AutopilotContinuationMode =
  | 'follow_up_turn'
  | 'goal_continuation'

export type AutopilotContinuationDecision =
  | 'dispatched'
  | 'failed'
  | 'skipped'

export type AutopilotContinuationEventRecord = Readonly<{
  attempt: number
  createdAt: string
  decision: AutopilotContinuationDecision
  goalId: string | null
  id: string
  mode: AutopilotContinuationMode
  reasonRef: string
  runId: string
  userId: string
}>

export type AutopilotContinuationStore = Readonly<{
  claimContinuationAttempt: (
    record: AutopilotContinuationEventRecord,
  ) => Promise<Readonly<{ claimed: boolean }>>
  countAttemptsForRun: (runId: string) => Promise<number>
  countAttemptsForUserSince: (
    userId: string,
    sinceIso: string,
  ) => Promise<number>
  listEnabledPolicies: (
    limit: number,
  ) => Promise<ReadonlyArray<AutopilotContinuationPolicyRecord>>
  listEventsForUserSince: (
    userId: string,
    sinceIso: string,
    limit: number,
  ) => Promise<ReadonlyArray<AutopilotContinuationEventRecord>>
  markContinuationAttemptFailed: (
    id: string,
    reasonRef: string,
  ) => Promise<void>
  readPolicy: (
    userId: string,
  ) => Promise<AutopilotContinuationPolicyRecord | undefined>
  upsertPolicy: (
    record: AutopilotContinuationPolicyRecord,
  ) => Promise<AutopilotContinuationPolicyRecord>
}>

export type AutopilotContinuationPolicyProjection = Readonly<{
  budgetGateRefs: ReadonlyArray<string>
  enabled: boolean
  generatedAt: string
  maxContinuationsPerDay: number
  maxContinuationsPerRun: number
  policyRef: 'openagents.autopilot_continuation_policy.v1'
  spendAuthority: false
  updatedAt: string | null
}>

export const defaultAutopilotContinuationPolicy = (
  userId: string,
  nowIso: string,
): AutopilotContinuationPolicyRecord => ({
  createdAt: nowIso,
  enabled: false,
  maxContinuationsPerDay: AUTOPILOT_CONTINUATION_DEFAULT_MAX_PER_DAY,
  maxContinuationsPerRun: AUTOPILOT_CONTINUATION_DEFAULT_MAX_PER_RUN,
  updatedAt: nowIso,
  userId,
})

export const autopilotContinuationPolicyProjection = (
  record: AutopilotContinuationPolicyRecord | undefined,
  generatedAt: string,
): AutopilotContinuationPolicyProjection => ({
  budgetGateRefs: [
    'budget_gate.billing.minimum_run_credits',
    'budget_gate.goal.token_budget',
    'budget_gate.continuation.max_per_run',
    'budget_gate.continuation.max_per_day',
  ],
  enabled: record?.enabled ?? false,
  generatedAt,
  maxContinuationsPerDay:
    record?.maxContinuationsPerDay ??
    AUTOPILOT_CONTINUATION_DEFAULT_MAX_PER_DAY,
  maxContinuationsPerRun:
    record?.maxContinuationsPerRun ??
    AUTOPILOT_CONTINUATION_DEFAULT_MAX_PER_RUN,
  policyRef: 'openagents.autopilot_continuation_policy.v1',
  spendAuthority: false,
  updatedAt: record?.updatedAt ?? null,
})

const policyRecordFromRow = (
  row: Readonly<Record<string, unknown>>,
): AutopilotContinuationPolicyRecord => ({
  createdAt: String(row.created_at),
  enabled: Number(row.enabled) === 1,
  maxContinuationsPerDay: Number(row.max_continuations_per_day),
  maxContinuationsPerRun: Number(row.max_continuations_per_run),
  updatedAt: String(row.updated_at),
  userId: String(row.user_id),
})

const eventRecordFromRow = (
  row: Readonly<Record<string, unknown>>,
): AutopilotContinuationEventRecord => ({
  attempt: Number(row.attempt),
  createdAt: String(row.created_at),
  decision: S.decodeUnknownSync(
    S.Literals(['dispatched', 'failed', 'skipped']),
  )(row.decision),
  goalId: typeof row.goal_id === 'string' ? row.goal_id : null,
  id: String(row.id),
  mode: S.decodeUnknownSync(
    S.Literals(['follow_up_turn', 'goal_continuation']),
  )(row.mode),
  reasonRef: String(row.reason_ref),
  runId: String(row.run_id),
  userId: String(row.user_id),
})

export const makeD1AutopilotContinuationStore = (
  db: D1Database,
): AutopilotContinuationStore => ({
  claimContinuationAttempt: async record => {
    const result = await db
      .prepare(
        `INSERT OR IGNORE INTO autopilot_continuation_events (
          id, user_id, run_id, goal_id, mode, decision, reason_ref, attempt, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        record.id,
        record.userId,
        record.runId,
        record.goalId,
        record.mode,
        record.decision,
        record.reasonRef,
        record.attempt,
        record.createdAt,
      )
      .run()

    return { claimed: (result.meta.changes ?? 0) > 0 }
  },
  countAttemptsForRun: async runId => {
    const row = await db
      .prepare(
        `SELECT COUNT(*) AS attempts
         FROM autopilot_continuation_events
         WHERE run_id = ?`,
      )
      .bind(runId)
      .first<Readonly<{ attempts: number }>>()

    return Number(row?.attempts ?? 0)
  },
  countAttemptsForUserSince: async (userId, sinceIso) => {
    const row = await db
      .prepare(
        `SELECT COUNT(*) AS attempts
         FROM autopilot_continuation_events
         WHERE user_id = ?
           AND created_at >= ?`,
      )
      .bind(userId, sinceIso)
      .first<Readonly<{ attempts: number }>>()

    return Number(row?.attempts ?? 0)
  },
  listEnabledPolicies: async limit => {
    const rows = await db
      .prepare(
        `SELECT *
         FROM autopilot_continuation_policies
         WHERE enabled = 1
         ORDER BY updated_at DESC
         LIMIT ?`,
      )
      .bind(limit)
      .all<Record<string, unknown>>()

    return (rows.results ?? []).map(policyRecordFromRow)
  },
  listEventsForUserSince: async (userId, sinceIso, limit) => {
    const rows = await db
      .prepare(
        `SELECT *
         FROM autopilot_continuation_events
         WHERE user_id = ?
           AND created_at >= ?
         ORDER BY created_at DESC
         LIMIT ?`,
      )
      .bind(userId, sinceIso, limit)
      .all<Record<string, unknown>>()

    return (rows.results ?? []).map(eventRecordFromRow)
  },
  markContinuationAttemptFailed: async (id, reasonRef) => {
    await db
      .prepare(
        `UPDATE autopilot_continuation_events
         SET decision = 'failed',
             reason_ref = ?
         WHERE id = ?`,
      )
      .bind(reasonRef, id)
      .run()
  },
  readPolicy: async userId => {
    const row = await db
      .prepare(
        `SELECT *
         FROM autopilot_continuation_policies
         WHERE user_id = ?
         LIMIT 1`,
      )
      .bind(userId)
      .first<Record<string, unknown>>()

    return row === null ? undefined : policyRecordFromRow(row)
  },
  upsertPolicy: async record => {
    await db
      .prepare(
        `INSERT INTO autopilot_continuation_policies (
          user_id, enabled, max_continuations_per_run, max_continuations_per_day, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?)
        ON CONFLICT (user_id) DO UPDATE SET
          enabled = excluded.enabled,
          max_continuations_per_run = excluded.max_continuations_per_run,
          max_continuations_per_day = excluded.max_continuations_per_day,
          updated_at = excluded.updated_at`,
      )
      .bind(
        record.userId,
        record.enabled ? 1 : 0,
        record.maxContinuationsPerRun,
        record.maxContinuationsPerDay,
        record.createdAt,
        record.updatedAt,
      )
      .run()

    return record
  },
})

export type AutopilotContinuationRunCandidate = Readonly<{
  goalId: string | null
  runId: string
  status: 'failed' | 'waiting_for_input'
  updatedAt: string
  userId: string
}>

export const listAutopilotContinuationRunCandidates = async (
  db: D1Database,
  input: Readonly<{ limit: number; sinceIso: string; userId: string }>,
): Promise<ReadonlyArray<AutopilotContinuationRunCandidate>> => {
  const rows = await db
    .prepare(
      `SELECT id, user_id, goal_id, status, updated_at
       FROM agent_runs
       WHERE user_id = ?
         AND status IN ('failed', 'waiting_for_input')
         AND updated_at >= ?
       ORDER BY updated_at DESC
       LIMIT ?`,
    )
    .bind(input.userId, input.sinceIso, input.limit)
    .all<Record<string, unknown>>()

  return (rows.results ?? []).map(row => ({
    goalId: typeof row.goal_id === 'string' ? row.goal_id : null,
    runId: String(row.id),
    status: S.decodeUnknownSync(S.Literals(['failed', 'waiting_for_input']))(
      row.status,
    ),
    updatedAt: String(row.updated_at),
    userId: String(row.user_id),
  }))
}

export type AutopilotContinuationDispatchResult = Readonly<{
  ok: boolean
  reasonRef: string
}>

export type AutopilotContinuationSweepDependencies = Readonly<{
  billingAllowsContinuation: (
    userId: string,
  ) => Promise<AutopilotContinuationDispatchResult>
  dispatchFollowUpTurn: (
    candidate: AutopilotContinuationRunCandidate,
  ) => Promise<AutopilotContinuationDispatchResult>
  dispatchGoalContinuation: (
    candidate: AutopilotContinuationRunCandidate,
  ) => Promise<AutopilotContinuationDispatchResult>
  listStoppedRunsForUser: (
    userId: string,
    sinceIso: string,
    limit: number,
  ) => Promise<ReadonlyArray<AutopilotContinuationRunCandidate>>
  makeId?: () => string
  nowIso: string
  store: AutopilotContinuationStore
}>

export type AutopilotContinuationSweepReport = Readonly<{
  continuedRunIds: ReadonlyArray<string>
  generatedAt: string
  skipped: ReadonlyArray<Readonly<{ reasonRef: string; runId: string }>>
}>

const continuationModeForCandidate = (
  candidate: AutopilotContinuationRunCandidate,
): AutopilotContinuationMode =>
  candidate.status === 'waiting_for_input'
    ? 'follow_up_turn'
    : 'goal_continuation'

const lookbackSinceIso = (nowIso: string): string =>
  isoTimestampAfterIso(
    nowIso,
    -AUTOPILOT_CONTINUATION_LOOKBACK_HOURS * 60 * 60_000,
  )

export const runAutopilotContinuationSweep = (
  dependencies: AutopilotContinuationSweepDependencies,
): Effect.Effect<AutopilotContinuationSweepReport> =>
  Effect.promise(async () => {
    const nowIso = dependencies.nowIso
    const sinceIso = lookbackSinceIso(nowIso)
    const makeId = dependencies.makeId ?? randomUuid
    const continuedRunIds: Array<string> = []
    const skipped: Array<{ reasonRef: string; runId: string }> = []
    const policies = await dependencies.store.listEnabledPolicies(100)

    for (const policy of policies) {
      const candidates = await dependencies.listStoppedRunsForUser(
        policy.userId,
        sinceIso,
        25,
      )

      if (candidates.length === 0) {
        continue
      }

      let attemptsToday = await dependencies.store.countAttemptsForUserSince(
        policy.userId,
        sinceIso,
      )
      const billing = await dependencies.billingAllowsContinuation(
        policy.userId,
      )

      for (const candidate of candidates) {
        const mode = continuationModeForCandidate(candidate)

        if (mode === 'goal_continuation' && candidate.goalId === null) {
          skipped.push({
            reasonRef: 'continuation.skipped.run_goal_required',
            runId: candidate.runId,
          })
          continue
        }

        if (attemptsToday >= policy.maxContinuationsPerDay) {
          skipped.push({
            reasonRef: 'continuation.skipped.max_per_day_reached',
            runId: candidate.runId,
          })
          continue
        }

        if (!billing.ok) {
          skipped.push({
            reasonRef: billing.reasonRef,
            runId: candidate.runId,
          })
          continue
        }

        const attempts = await dependencies.store.countAttemptsForRun(
          candidate.runId,
        )

        if (attempts >= policy.maxContinuationsPerRun) {
          skipped.push({
            reasonRef: 'continuation.skipped.max_per_run_reached',
            runId: candidate.runId,
          })
          continue
        }

        const claimId = makeId()
        const claim = await dependencies.store.claimContinuationAttempt({
          attempt: attempts + 1,
          createdAt: nowIso,
          decision: 'dispatched',
          goalId: candidate.goalId,
          id: claimId,
          mode,
          reasonRef: 'continuation.policy_dispatch',
          runId: candidate.runId,
          userId: candidate.userId,
        })

        if (!claim.claimed) {
          skipped.push({
            reasonRef: 'continuation.skipped.already_claimed',
            runId: candidate.runId,
          })
          continue
        }

        attemptsToday += 1
        const dispatch = await M.value(mode).pipe(
          M.when('follow_up_turn', () =>
            dependencies.dispatchFollowUpTurn(candidate)
          ),
          M.when('goal_continuation', () =>
            dependencies.dispatchGoalContinuation(candidate)
          ),
          M.exhaustive,
        ).catch(error => ({
          ok: false,
          reasonRef: `continuation.failed.${
            error instanceof Error ? 'dispatch_error' : 'unknown_error'
          }`,
        }))

        if (!dispatch.ok) {
          await dependencies.store.markContinuationAttemptFailed(
            claimId,
            dispatch.reasonRef,
          )
          skipped.push({
            reasonRef: dispatch.reasonRef,
            runId: candidate.runId,
          })
          continue
        }

        continuedRunIds.push(candidate.runId)
      }
    }

    return { continuedRunIds, generatedAt: nowIso, skipped }
  })
