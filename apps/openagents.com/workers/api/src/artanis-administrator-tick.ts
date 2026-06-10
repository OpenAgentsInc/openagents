import { Effect, Schema as S } from 'effect'

import {
  TASSADAR_EXECUTOR_CAPABILITY_REF,
  TASSADAR_EXECUTOR_TRACE_HOMEWORK_JOB_KIND,
  TASSADAR_EXECUTOR_TRACE_JOB_KIND,
} from '@openagents/tassadar-executor'
import tassadarPocFixture from '../../../../packages/tassadar-executor/fixtures/tassadar-poc-loop-sum-v1.json'

import { artanisMindComplete } from './artanis-mind'
import { parseJsonWithSchema } from './json-boundary'
import { epochMillisToIsoTimestamp, randomUuid } from './runtime-primitives'
import {
  TassadarBoundedProfileRef,
  TassadarExactTraceReplayVerificationClass,
  buildTassadarExecutorTracePayload,
} from './tassadar-executor-trace-homework'

// The Artanis administrator tick (issue #4701): the model-decided tick
// replacing placeholder actions. Each cron tick the mind receives the
// assembled administrator context - capacity funnel, online Pylons with
// capabilities, open executor assignments - and proposes one typed
// action from the bounded vocabulary. Schema-invalid output records a
// blocked decision carrying the raw proposal; valid dispatch proposals
// execute IN-PROCESS through the real operator assignments route in
// unpaid_smoke mode only (the no-spend safe kind). Paid dispatch,
// wallet spend, and training launch keep their gates (#4703 holds the
// spend envelope). Every decision is a row; nothing is silent.

export const ARTANIS_ADMIN_DISPATCH_PER_DAY = 4

const MindAdminAction = S.Union(
  S.Struct({
    kind: S.Literal('dispatch_executor_trace'),
    pylonRef: S.String,
    rationale: S.String,
  }),
  S.Struct({
    kind: S.Literal('no_action'),
    rationale: S.String,
  }),
)

export type AdminTickOutcome = Readonly<{
  state:
    | 'dispatched'
    | 'no_action'
    | 'blocked'
    | 'skipped'
    | 'dispatch_failed'
  decisionId: string | null
  assignmentRef: string | null
  reason: string | null
}>

export type AdminDispatchFn = (
  body: Record<string, unknown>,
) => Promise<{ ok: boolean; detail: string }>

// The exact PoC dispatch contract the operator script uses, built
// in-worker: transit rename seed_writes -> initialChannelWrites for the
// public-projection scanner, capability mirrored into the lease payload,
// no-spend gates for unpaid_smoke.
export const buildTassadarPocDispatchBody = (
  input: Readonly<{ pylonRef: string; assignmentRef: string }>,
): Record<string, unknown> => {
  const fixture = tassadarPocFixture as {
    fixtureId: string
    expectedModelDigest: string
    expectedTraceDigest: string
    steps: number
    model: Record<string, unknown> & { seed_writes?: unknown }
  }
  const { seed_writes: fixtureSeedWrites, ...modelWithoutSeeds } =
    fixture.model
  const transitModel = {
    ...modelWithoutSeeds,
    initialChannelWrites: fixtureSeedWrites,
  }
  const homeworkPayload = buildTassadarExecutorTracePayload({
    assignmentRef: input.assignmentRef,
    workloadFamily: 'kernel_trace',
  })

  return {
    acceptanceCriteriaRefs: [
      'acceptance.tassadar_poc.trace_digest_matches_fixture',
      'acceptance.tassadar_poc.closeout_carries_trace_digest',
    ],
    assignmentRef: input.assignmentRef,
    campaignPaused: false,
    campaignPolicyRefs: ['policy.tassadar_poc.single_assignment_smoke'],
    campaignRef: 'campaign.tassadar_poc.v1',
    closeoutPathRefs: [
      'route:/api/pylons/pylonRef/assignments/leaseRef/closeout',
    ],
    codingAssignment: {
      kind: TASSADAR_EXECUTOR_TRACE_HOMEWORK_JOB_KIND,
      objective: {
        objectiveRef: `goal.tassadar_poc.execute.${fixture.fixtureId}`,
      },
      requiredCapabilityRefs: [TASSADAR_EXECUTOR_CAPABILITY_REF],
      tassadar: {
        boundedProfileRef: TassadarBoundedProfileRef,
        expectedModelDigest: fixture.expectedModelDigest,
        expectedTraceDigest: fixture.expectedTraceDigest,
        fixtureId: fixture.fixtureId,
        homework: homeworkPayload,
        model: transitModel,
        steps: fixture.steps,
        verificationClass: TassadarExactTraceReplayVerificationClass,
      },
    },
    forumAutoPublishAllowed: false,
    idempotencyRefs: [`idempotency.tassadar_poc.${input.assignmentRef}`],
    jobKind: TASSADAR_EXECUTOR_TRACE_JOB_KIND,
    leaseSeconds: 3600,
    noDuplicateAssignmentRefs: ['gate.tassadar_poc.no_duplicate'],
    noForumAutoPublishRefs: ['gate.tassadar_poc.no_forum_auto_publish'],
    operatorPauseRefs: ['gate.tassadar_poc.operator_pause_available'],
    paymentMode: 'unpaid_smoke',
    pylonRef: input.pylonRef,
    requiredCapabilityRefs: [TASSADAR_EXECUTOR_CAPABILITY_REF],
    resultExpectationRefs: [
      `expectation.tassadar_poc.trace_digest.${fixture.expectedTraceDigest.slice(0, 16)}`,
    ],
    rollbackRefs: ['gate.tassadar_poc.rollback_cancel_assignment'],
    selectionPolicyRefs: ['policy.artanis_admin_tick.mind_selected_pylon'],
    spendCapRefs: ['gate.tassadar_poc.no_spend'],
    taskRefs: [`task.tassadar_poc.${fixture.fixtureId}`],
  }
}

const assembleContext = async (db: D1Database, nowIso: string) => {
  const onlinePylons = (
    (
      await db
        .prepare(
          `SELECT pylon_ref, capability_refs_json, latest_heartbeat_at
             FROM pylon_api_registrations
            WHERE latest_heartbeat_at > ?
            ORDER BY latest_heartbeat_at DESC LIMIT 10`,
        )
        .bind(epochMillisToIsoTimestamp(Date.parse(nowIso) - 10 * 60_000))
        .all()
    ).results ?? []
  ) as Array<Record<string, unknown>>

  const openAssignments = (
    (
      await db
        .prepare(
          `SELECT assignment_ref, pylon_ref, state
             FROM pylon_api_assignments
            WHERE state IN ('offered', 'accepted')
              AND job_kind = ?
            LIMIT 10`,
        )
        .bind(TASSADAR_EXECUTOR_TRACE_JOB_KIND)
        .all()
    ).results ?? []
  ) as Array<Record<string, unknown>>

  return { onlinePylons, openAssignments }
}

export const runArtanisAdminTick = async (
  db: D1Database,
  deps: Readonly<{
    geminiApiKey: string | null
    gatewayToken?: string | undefined
    dispatch: AdminDispatchFn
    nowIso: string
  }>,
): Promise<AdminTickOutcome> => {
  const skipped = (reason: string): AdminTickOutcome => ({
    assignmentRef: null,
    decisionId: null,
    reason,
    state: 'skipped',
  })

  if (deps.geminiApiKey === null || deps.geminiApiKey === '') {
    return skipped('mind_unconfigured')
  }

  const dispatchedToday = (await db
    .prepare(
      `SELECT COUNT(*) AS n FROM artanis_admin_tick_decisions
        WHERE state = 'dispatched' AND created_at >= ?`,
    )
    .bind(`${deps.nowIso.slice(0, 10)}T00:00:00.000Z`)
    .first()) as { n: number } | null

  if (Number(dispatchedToday?.n ?? 0) >= ARTANIS_ADMIN_DISPATCH_PER_DAY) {
    return skipped('daily_dispatch_bound_reached')
  }

  const context = await assembleContext(db, deps.nowIso)

  const eligible = context.onlinePylons.filter(pylon =>
    String(pylon.capability_refs_json ?? '').includes(
      TASSADAR_EXECUTOR_CAPABILITY_REF,
    ),
  )

  if (eligible.length === 0) {
    return skipped('no_eligible_online_pylons')
  }

  if (context.openAssignments.length > 0) {
    return skipped('open_assignment_in_flight')
  }

  const mindResult = await artanisMindComplete({
    apiKey: deps.geminiApiKey,
    ...(deps.gatewayToken === undefined || deps.gatewayToken === ''
      ? {}
      : { gatewayToken: deps.gatewayToken }),
    prompt: [
      'Administrator context for this tick:',
      `Eligible online Pylons (executor capability declared, heartbeat within 10 minutes): ${JSON.stringify(eligible.map(p => ({ lastHeartbeatAt: p.latest_heartbeat_at, pylonRef: p.pylon_ref })))}`,
      `Open executor-trace assignments: ${JSON.stringify(context.openAssignments)}`,
      'Available actions: dispatch one no-spend executor-trace workload to keep an idle eligible device exercised and its capability proven, or take no action if dispatch would not be useful this tick.',
      'Output STRICT JSON only: {"kind":"dispatch_executor_trace","pylonRef":"...","rationale":"..."} or {"kind":"no_action","rationale":"..."}',
    ].join('\n'),
    system:
      'You are Artanis, the Nexus administrator: you distribute work to Pylons and keep devices utilized. Safe no-spend dispatch only; you output strict JSON.',
  })

  const decisionId = randomUuid()

  if ('error' in mindResult) {
    return skipped('mind_unavailable')
  }

  let action: typeof MindAdminAction.Type | null = null
  try {
    const cleaned = mindResult.text
      .replace(/^```(?:json)?\s*/m, '')
      .replace(/```\s*$/m, '')
      .trim()
    action = parseJsonWithSchema(MindAdminAction, cleaned)
  } catch {
    action = null
  }

  if (action === null) {
    await db
      .prepare(
        `INSERT INTO artanis_admin_tick_decisions
         (id, state, action_json, created_at)
         VALUES (?, 'blocked', ?, ?)`,
      )
      .bind(
        decisionId,
        JSON.stringify({
          rawProposal: mindResult.text.slice(0, 1000),
          reason: 'schema_invalid_mind_output',
        }),
        deps.nowIso,
      )
      .run()
    return {
      assignmentRef: null,
      decisionId,
      reason: 'schema_invalid_mind_output',
      state: 'blocked',
    }
  }

  if (action.kind === 'no_action') {
    await db
      .prepare(
        `INSERT INTO artanis_admin_tick_decisions
         (id, state, action_json, created_at)
         VALUES (?, 'no_action', ?, ?)`,
      )
      .bind(decisionId, JSON.stringify(action), deps.nowIso)
      .run()
    return {
      assignmentRef: null,
      decisionId,
      reason: action.rationale.slice(0, 200),
      state: 'no_action',
    }
  }

  // The mind may only dispatch to a pylon the CONTEXT proved eligible.
  const target = eligible.find(
    pylon => String(pylon.pylon_ref) === action.pylonRef,
  )
  if (target === undefined) {
    await db
      .prepare(
        `INSERT INTO artanis_admin_tick_decisions
         (id, state, action_json, created_at)
         VALUES (?, 'blocked', ?, ?)`,
      )
      .bind(
        decisionId,
        JSON.stringify({ ...action, reason: 'pylon_not_in_eligible_set' }),
        deps.nowIso,
      )
      .run()
    return {
      assignmentRef: null,
      decisionId,
      reason: 'pylon_not_in_eligible_set',
      state: 'blocked',
    }
  }

  const assignmentRef = `assignment.artanis_admin.${deps.nowIso.replace(/[-:.TZ]/g, '').slice(0, 14)}`
  const dispatchResult = await deps.dispatch(
    buildTassadarPocDispatchBody({
      assignmentRef,
      pylonRef: action.pylonRef,
    }),
  )

  await db
    .prepare(
      `INSERT INTO artanis_admin_tick_decisions
       (id, state, action_json, assignment_ref, created_at)
       VALUES (?, ?, ?, ?, ?)`,
    )
    .bind(
      decisionId,
      dispatchResult.ok ? 'dispatched' : 'dispatch_failed',
      JSON.stringify({ ...action, detail: dispatchResult.detail.slice(0, 200) }),
      assignmentRef,
      deps.nowIso,
    )
    .run()

  return {
    assignmentRef,
    decisionId,
    reason: dispatchResult.ok ? action.rationale.slice(0, 200) : dispatchResult.detail.slice(0, 200),
    state: dispatchResult.ok ? 'dispatched' : 'dispatch_failed',
  }
}

export const runArtanisAdminTickScheduled = (
  db: D1Database,
  deps: Readonly<{
    enabled: boolean
    geminiApiKey: string | null
    gatewayToken?: string | undefined
    dispatch: AdminDispatchFn
    nowIso: string
  }>,
): Effect.Effect<AdminTickOutcome, never> =>
  deps.enabled
    ? Effect.tryPromise({
        catch: () => 'admin_tick_error' as const,
        try: () => runArtanisAdminTick(db, deps),
      }).pipe(
        Effect.catch(reason =>
          Effect.succeed({
            assignmentRef: null,
            decisionId: null,
            reason,
            state: 'skipped',
          } satisfies AdminTickOutcome),
        ),
      )
    : Effect.succeed({
        assignmentRef: null,
        decisionId: null,
        reason: 'admin_tick_disabled',
        state: 'skipped',
      })
