import { Effect, Schema as S } from 'effect'

import {
  TASSADAR_EXECUTOR_CAPABILITY_REF,
  TASSADAR_EXECUTOR_TRACE_HOMEWORK_JOB_KIND,
  TASSADAR_EXECUTOR_TRACE_JOB_KIND,
} from '@openagentsinc/tassadar-executor'
import {
  selectTassadarCompiledProgramFixture,
  tassadarCompiledProgramCorpus,
  tassadarCompiledProgramCorpusSize,
} from '@openagentsinc/tassadar-executor/compiled-program-corpus'
import {
  TASSADAR_ALM_DENSE_WEIGHT_MODULE_KIND,
  tassadarDenseProgramFixture,
} from '@openagentsinc/tassadar-executor/dense-weight-module'

import { artanisMindComplete } from './artanis-mind'
import {
  type ArtanisLaborPersistedTick,
  runAndPersistArtanisLaborRequestTick,
} from './artanis-labor-tick-driver'
import type {
  ArtanisLaborRequesterDeps,
  ArtanisLaborRequesterOutcome,
} from './artanis-labor-requester'
import type { ArtanisLaborUnattendedReceiptStore } from './artanis-labor-receipt-store'
import { parseJsonStringArray, parseJsonWithSchema } from './json-boundary'
import { epochMillisToIsoTimestamp, randomUuid } from './runtime-primitives'
import { pylonCapabilityRefsEligibleForExecutorDispatch } from './tassadar-capability-admission'
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

const MindAdminAction = S.Union([
  S.Struct({
    kind: S.Literal('dispatch_executor_trace'),
    pylonRef: S.String,
    rationale: S.String,
  }),
  S.Struct({
    kind: S.Literal('no_action'),
    rationale: S.String,
  }),
])

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

export type AdminLaborRequestTickOutcome = Readonly<{
  state: 'placed' | 'refused' | 'skipped' | 'failed'
  reason: string | null
  receiptRef: string | null
  terminalState:
    | 'requested_pending_delivery'
    | 'refused'
    | 'skipped_config_disabled'
    | null
  workRequestId: string | null
}>

export type AdminDispatchFn = (
  body: Record<string, unknown>,
) => Promise<{ ok: boolean; detail: string }>

// The exact PoC dispatch contract the operator script uses, built
// in-worker: transit rename seed_writes -> initialChannelWrites for the
// public-projection scanner, capability mirrored into the lease payload,
// no-spend gates for unpaid_smoke.
export const buildTassadarCorpusDispatchBody = (
  input: Readonly<{ pylonRef: string; assignmentRef: string }>,
): Record<string, unknown> => {
  const fixture = selectTassadarCompiledProgramFixture({
    assignmentRef: input.assignmentRef,
  })
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
  const denseFixture =
    fixture.programId === tassadarDenseProgramFixture.programId
      ? tassadarDenseProgramFixture
      : null
  const expectedModelDigest =
    denseFixture === null ? fixture.expectedModelDigest : denseFixture.denseModuleDigest
  const expectedTraceDigest =
    denseFixture === null ? fixture.expectedTraceDigest : denseFixture.expectedTraceDigest
  const compileReceiptRefs =
    denseFixture === null ? fixture.compileReceiptRefs : denseFixture.compileReceiptRefs

  return {
    acceptanceCriteriaRefs: [
      'acceptance.tassadar_poc.trace_digest_matches_fixture',
      'acceptance.tassadar_corpus.trace_digest_matches_selected_fixture',
      ...(denseFixture === null
        ? []
        : ['acceptance.tassadar_dense_weight_module.trace_digest_matches_fixture']),
      'acceptance.tassadar_poc.closeout_carries_trace_digest',
    ],
    assignmentRef: input.assignmentRef,
    campaignPaused: false,
    campaignPolicyRefs: [
      'policy.tassadar_poc.single_assignment_smoke',
      'policy.tassadar_corpus.c1_no_spend_program_corpus',
      ...(denseFixture === null
        ? []
        : ['policy.tassadar_c3.dense_weight_module_no_spend_replay']),
    ],
    campaignRef: 'campaign.tassadar_corpus.c1.v1',
    closeoutPathRefs: [
      'route:/api/pylons/pylonRef/assignments/leaseRef/closeout',
    ],
    codingAssignment: {
      kind: TASSADAR_EXECUTOR_TRACE_HOMEWORK_JOB_KIND,
      objective: {
        objectiveRef: `goal.tassadar_corpus.execute.${fixture.fixtureId}`,
      },
      requiredCapabilityRefs: [TASSADAR_EXECUTOR_CAPABILITY_REF],
      tassadar: {
        boundedProfileRef: TassadarBoundedProfileRef,
        compileReceiptRefs,
        corpusDigest: tassadarCompiledProgramCorpus.corpusDigest,
        corpusId: tassadarCompiledProgramCorpus.corpusId,
        expectedModelDigest,
        expectedOutputs: fixture.expectedOutputs,
        expectedTraceDigest,
        fixtureId: fixture.fixtureId,
        homework: homeworkPayload,
        model: transitModel,
        modelArtifactKind:
          denseFixture === null
            ? 'tassadar_alm_numeric_model.v1'
            : TASSADAR_ALM_DENSE_WEIGHT_MODULE_KIND,
        ...(denseFixture === null
          ? {}
          : {
              denseModule: denseFixture.denseModule,
              denseModuleDigest: denseFixture.denseModuleDigest,
              denseModuleSourceModelDigest:
                denseFixture.denseModule.sourceModelDigest,
              denseRunArtifactRefs: denseFixture.runArtifactRefs,
            }),
        programDigest: fixture.programDigest,
        programId: fixture.programId,
        steps: fixture.steps,
        verificationClass: TassadarExactTraceReplayVerificationClass,
        workloadKind: fixture.workloadKind,
      },
    },
    forumAutoPublishAllowed: false,
    idempotencyRefs: [`idempotency.tassadar_corpus.${input.assignmentRef}`],
    jobKind: TASSADAR_EXECUTOR_TRACE_JOB_KIND,
    leaseSeconds: 3600,
    noDuplicateAssignmentRefs: [
      'gate.tassadar_poc.no_duplicate',
      'gate.tassadar_corpus.no_duplicate',
    ],
    noForumAutoPublishRefs: ['gate.tassadar_poc.no_forum_auto_publish'],
    operatorPauseRefs: ['gate.tassadar_poc.operator_pause_available'],
    paymentMode: 'unpaid_smoke',
    pylonRef: input.pylonRef,
    requiredCapabilityRefs: [TASSADAR_EXECUTOR_CAPABILITY_REF],
    resultExpectationRefs: [
      `expectation.tassadar_poc.trace_digest.${expectedTraceDigest.slice(0, 16)}`,
      `expectation.tassadar_corpus.program.${fixture.programId}`,
      `expectation.tassadar_corpus.corpus.${tassadarCompiledProgramCorpus.corpusDigest.slice(0, 16)}`,
      ...(denseFixture === null
        ? []
        : [
            `expectation.tassadar_dense_weight_module.${denseFixture.denseModuleDigest.slice(0, 16)}`,
          ]),
    ],
    rollbackRefs: ['gate.tassadar_poc.rollback_cancel_assignment'],
    selectionPolicyRefs: [
      'policy.artanis_admin_tick.mind_selected_pylon',
      'policy.tassadar_corpus.assignment_ref_workload_slot',
    ],
    spendCapRefs: ['gate.tassadar_poc.no_spend'],
    taskRefs: [
      `task.tassadar_corpus.${fixture.fixtureId}`,
      ...(denseFixture === null
        ? []
        : [`task.tassadar_dense_weight_module.${denseFixture.fixtureId}`]),
    ],
  }
}

export const buildTassadarPocDispatchBody = buildTassadarCorpusDispatchBody

const assembleContext = async (db: D1Database, nowIso: string) => {
  const onlinePylons = (
    (
      await db
        .prepare(
          `SELECT pylon_ref, capability_refs_json, latest_heartbeat_at
             FROM pylon_api_registrations
            WHERE latest_heartbeat_at > ?
              AND wallet_ready = 1
              AND status = 'active'
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

  const dispatchedTodayCount = Number(dispatchedToday?.n ?? 0)

  if (dispatchedTodayCount >= ARTANIS_ADMIN_DISPATCH_PER_DAY) {
    return skipped('daily_dispatch_bound_reached')
  }

  const context = await assembleContext(db, deps.nowIso)

  // W4.1 (#4750): executor eligibility requires the receipted capability
  // (claim + self-test receipt ref), not the bare configuration claim.
  const eligible = context.onlinePylons.filter(pylon =>
    pylonCapabilityRefsEligibleForExecutorDispatch(
      parseJsonStringArray(
        typeof pylon.capability_refs_json === 'string'
          ? pylon.capability_refs_json
          : null,
      ),
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
      `Available actions: dispatch one no-spend executor-trace workload from the ${tassadarCompiledProgramCorpusSize}-program compiled corpus to keep an idle eligible device exercised and its capability proven, or take no action if dispatch would not be useful this tick.`,
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

  const workloadIndex = dispatchedTodayCount % tassadarCompiledProgramCorpusSize
  const assignmentRef = `assignment.artanis_admin.${deps.nowIso.replace(/[-:.TZ]/g, '').slice(0, 14)}.w${workloadIndex}`
  const dispatchResult = await deps.dispatch(
    buildTassadarCorpusDispatchBody({
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

const laborOutcomeState = (
  outcome: ArtanisLaborRequesterOutcome,
): AdminLaborRequestTickOutcome['state'] => {
  if (outcome.kind === 'requested') {
    return 'placed'
  }
  if (outcome.kind === 'refused') {
    return 'refused'
  }
  return 'skipped'
}

const laborOutcomeReason = (
  outcome: ArtanisLaborRequesterOutcome,
): string | null => {
  if (outcome.kind === 'requested') {
    return null
  }
  if (outcome.kind === 'refused') {
    return outcome.reason
  }
  return outcome.reason
}

const laborTickOutcomeFromPersisted = (
  persisted: ArtanisLaborPersistedTick,
): AdminLaborRequestTickOutcome => ({
  reason: laborOutcomeReason(persisted.requestOutcome),
  receiptRef: persisted.sealed.receiptRef,
  state: laborOutcomeState(persisted.requestOutcome),
  terminalState:
    persisted.sealed.receipt.terminalState === 'accepted_released' ||
    persisted.sealed.receipt.terminalState === 'rejected_refunded'
      ? null
      : persisted.sealed.receipt.terminalState,
  workRequestId: persisted.sealed.receipt.workRequestId,
})

export const runArtanisAdminLaborRequestTick = (
  input: Readonly<{
    artanisActorRef: string
    requesterDeps: ArtanisLaborRequesterDeps
    store: ArtanisLaborUnattendedReceiptStore
    tickRef: string
  }>,
): Promise<AdminLaborRequestTickOutcome> =>
  runAndPersistArtanisLaborRequestTick(input).then(laborTickOutcomeFromPersisted)

export const runArtanisAdminLaborRequestTickScheduled = (
  input: Readonly<{
    enabled: boolean
    requesterDeps: Omit<ArtanisLaborRequesterDeps, 'enabled'>
    store: ArtanisLaborUnattendedReceiptStore
    artanisActorRef: string
    tickRef: string
  }>,
): Effect.Effect<AdminLaborRequestTickOutcome, never> =>
  Effect.tryPromise({
    catch: () => 'labor_request_tick_error' as const,
    try: () =>
      runArtanisAdminLaborRequestTick({
        artanisActorRef: input.artanisActorRef,
        requesterDeps: {
          ...input.requesterDeps,
          enabled: input.enabled,
        },
        store: input.store,
        tickRef: input.tickRef,
      }),
  }).pipe(
    Effect.catch(reason =>
      Effect.succeed({
        reason,
        receiptRef: null,
        state: 'failed',
        terminalState: null,
        workRequestId: null,
      } satisfies AdminLaborRequestTickOutcome),
    ),
  )

// ---------------------------------------------------------------------------
// Closeout verifier (issue #4697, the verify->accept half of the span).
// For admin-tick-dispatched executor-trace assignments in
// closeout_submitted: extract the claimed trace digest from the closeout
// refs, replay the fixed PoC workload byte-identically in the worker,
// and accept or reject the closeout on the digest predicate alone -
// deterministic acceptance, no judgment calls, exactly why this work
// class is mechanically safe.

export type CloseoutAcceptFn = (input: {
  assignmentRef: string
  accepted: boolean
  refs: ReadonlyArray<string>
}) => Promise<{ ok: boolean; detail: string }>

export type CloseoutVerifierOutcome = Readonly<{
  considered: number
  verified: number
  rejected: number
  unreadable: number
}>

const digestFromRefs = (refsJson: unknown): string | null => {
  const text = String(refsJson ?? '')
  const match = /([a-f0-9]{64})/.exec(text)
  return match === null ? null : match[1]!
}

export const runArtanisCloseoutVerifier = async (
  db: D1Database,
  deps: Readonly<{
    replay: (input: {
      assignmentRef: string
      claimedTraceDigest: string
      pylonDeviceRef: string
      workload: {
        denseModule?: Record<string, unknown>
        model: Record<string, unknown>
        steps: ReadonlyArray<ReadonlyArray<number>>
      }
    }) => Promise<{ outcome: string }>
    accept: CloseoutAcceptFn
    nowIso: string
  }>,
): Promise<CloseoutVerifierOutcome> => {
  const rows = (
    (
      await db
        .prepare(
          `SELECT assignment_ref, pylon_ref, closeout_refs_json, proof_refs_json,
                  artifact_refs_json
             FROM pylon_api_assignments
            WHERE state = 'closeout_submitted'
              AND job_kind = ?
              AND assignment_ref LIKE 'assignment.artanis_admin.%'
              AND assignment_ref NOT IN (
                SELECT assignment_ref FROM artanis_closeout_verdicts
                WHERE outcome != 'unreadable'
              )
            LIMIT 3`,
        )
        .bind(TASSADAR_EXECUTOR_TRACE_JOB_KIND)
        .all()
    ).results ?? []
  ) as Array<Record<string, unknown>>

  let verified = 0
  let rejected = 0
  let unreadable = 0

  for (const row of rows) {
    const assignmentRef = String(row.assignment_ref)
    // The pylon executor puts the full trace digest in the artifact ref
    // (artifact.tassadar_poc.trace_digest.<64-hex>); proof/closeout refs
    // carry truncated prefixes only.
    const claimedDigest =
      digestFromRefs(row.artifact_refs_json) ??
      digestFromRefs(row.proof_refs_json) ??
      digestFromRefs(row.closeout_refs_json)

    if (claimedDigest === null) {
      unreadable += 1
      await db
        .prepare(
          `INSERT INTO artanis_closeout_verdicts
           (id, assignment_ref, outcome, accept_state, detail, created_at)
           VALUES (?, ?, 'unreadable', 'skipped', 'no 64-hex digest in closeout refs', ?)
           ON CONFLICT (assignment_ref) DO NOTHING`,
        )
        .bind(randomUuid(), assignmentRef, deps.nowIso)
        .run()
      continue
    }

    const dispatchBody = buildTassadarCorpusDispatchBody({
      assignmentRef,
      pylonRef: String(row.pylon_ref),
    })
    const tassadar = (
      dispatchBody.codingAssignment as {
        tassadar: {
          denseModule?: Record<string, unknown>
          model: Record<string, unknown>
          steps: ReadonlyArray<ReadonlyArray<number>>
        }
      }
    ).tassadar

    const verdict = await deps.replay({
      assignmentRef,
      claimedTraceDigest: claimedDigest,
      pylonDeviceRef: `device.pylon.${String(row.pylon_ref)}`,
      workload: {
        ...(tassadar.denseModule === undefined
          ? {}
          : { denseModule: tassadar.denseModule }),
        model: tassadar.model,
        steps: tassadar.steps,
      },
    })

    const isVerified = verdict.outcome === 'verified'
    const acceptResult = await deps.accept({
      accepted: isVerified,
      assignmentRef,
      refs: [
        `verdict.artanis_closeout.${verdict.outcome}`,
        `expectation.tassadar_poc.trace_digest.${claimedDigest.slice(0, 16)}`,
      ],
    })

    await db
      .prepare(
        `INSERT INTO artanis_closeout_verdicts
         (id, assignment_ref, outcome, claimed_trace_digest_prefix,
          accept_state, detail, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (assignment_ref) DO UPDATE SET
           outcome = excluded.outcome,
           claimed_trace_digest_prefix = excluded.claimed_trace_digest_prefix,
           accept_state = excluded.accept_state,
           detail = excluded.detail`,
      )
      .bind(
        randomUuid(),
        assignmentRef,
        isVerified ? 'verified' : 'rejected',
        claimedDigest.slice(0, 16),
        acceptResult.ok
          ? isVerified
            ? 'accepted'
            : 'rejected'
          : 'accept_failed',
        acceptResult.detail.slice(0, 200),
        deps.nowIso,
      )
      .run()

    if (isVerified) {
      verified += 1
    } else {
      rejected += 1
    }
  }

  return { considered: rows.length, rejected, unreadable, verified }
}

export const runArtanisCloseoutVerifierScheduled = (
  db: D1Database,
  deps: Readonly<{
    enabled: boolean
    replay: Parameters<typeof runArtanisCloseoutVerifier>[1]['replay']
    accept: CloseoutAcceptFn
    nowIso: string
  }>,
): Effect.Effect<CloseoutVerifierOutcome, never> =>
  deps.enabled
    ? Effect.tryPromise({
        catch: () => null,
        try: () => runArtanisCloseoutVerifier(db, deps),
      }).pipe(
        Effect.catch(() =>
          Effect.succeed({
            considered: 0,
            rejected: 0,
            unreadable: 0,
            verified: 0,
          } satisfies CloseoutVerifierOutcome),
        ),
      )
    : Effect.succeed({ considered: 0, rejected: 0, unreadable: 0, verified: 0 })
