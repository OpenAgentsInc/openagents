import { Effect, Match as M, Schema as S } from 'effect'

import {
  ArtanisApprovalGateLedgerRecord,
  ArtanisApprovalGateRecord,
  projectArtanisApprovalGateLedger,
} from './artanis-approval-gates'
import {
  ArtanisForumPublicationIntentRecord,
  ArtanisForumPublicationQueueRecord,
  projectArtanisForumPublicationQueue,
} from './artanis-forum-publication'
import {
  ArtanisHealthSignalProjection,
  ArtanisHealthSnapshotRecord,
  projectArtanisHealthSnapshot,
} from './artanis-health'
import {
  ARTANIS_LOOP_READ_ONLY_AUTHORITY,
  ArtanisLoopRecord,
  ArtanisLoopTickRecord,
} from './artanis-loop'
import {
  exampleArtanisOperatorSteeringWorkspace,
  projectArtanisOperatorSteeringWorkspace,
} from './artanis-operator-steering'
import {
  type ArtanisPersistenceError,
  ArtanisPersistenceRecordKind,
  type ArtanisPersistenceStoredRow,
  readLatestArtanisPersistedRows,
  saveArtanisApprovalGate,
} from './artanis-persistence'
import {
  ArtanisRuntimeRecord,
  projectArtanisRuntime,
} from './artanis-runtime'
import {
  ARTANIS_WORK_ROUTING_NO_DIRECT_AUTHORITY,
  ArtanisWorkRoutingLedgerRecord,
  ArtanisWorkRoutingProposalRecord,
  projectArtanisWorkRoutingLedger,
} from './artanis-work-routing'
import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { decodeUnknownWithSchema } from './json-boundary'
import {
  currentEpochMillis,
  epochMillisToIsoTimestamp,
} from './runtime-primitives'
import { openAgentsDatabase } from './runtime'

type OperatorArtanisConsoleEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>
type HttpResponse = globalThis.Response

type OperatorArtanisConsoleSession = Readonly<{
  user: Readonly<{
    email: string
    userId: string
  }>
}>

type OperatorArtanisConsoleDependencies<
  Session extends OperatorArtanisConsoleSession,
  Bindings extends OperatorArtanisConsoleEnv,
> = Readonly<{
  appendRefreshedSessionCookies: (
    response: HttpResponse,
    session: Session,
  ) => HttpResponse
  currentEpochMillis?: () => number
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

class OperatorArtanisConsoleUnauthorized extends S.TaggedErrorClass<OperatorArtanisConsoleUnauthorized>()(
  'OperatorArtanisConsoleUnauthorized',
  {},
) {}

class OperatorArtanisConsoleForbidden extends S.TaggedErrorClass<OperatorArtanisConsoleForbidden>()(
  'OperatorArtanisConsoleForbidden',
  {},
) {}

class OperatorArtanisConsoleNotFound extends S.TaggedErrorClass<OperatorArtanisConsoleNotFound>()(
  'OperatorArtanisConsoleNotFound',
  {},
) {}

class OperatorArtanisConsoleSessionError extends S.TaggedErrorClass<OperatorArtanisConsoleSessionError>()(
  'OperatorArtanisConsoleSessionError',
  {
    error: S.Defect,
  },
) {}

class OperatorArtanisConsoleStorageError extends S.TaggedErrorClass<OperatorArtanisConsoleStorageError>()(
  'OperatorArtanisConsoleStorageError',
  {
    error: S.Defect,
  },
) {}

type OperatorArtanisConsoleError =
  | ArtanisPersistenceError
  | OperatorArtanisConsoleForbidden
  | OperatorArtanisConsoleNotFound
  | OperatorArtanisConsoleSessionError
  | OperatorArtanisConsoleStorageError
  | OperatorArtanisConsoleUnauthorized

const routeErrorResponse = (error: OperatorArtanisConsoleError) =>
  M.value(error).pipe(
    M.tags({
      OperatorArtanisConsoleForbidden: () =>
        noStoreJsonResponse({ error: 'forbidden' }, { status: 403 }),
      OperatorArtanisConsoleNotFound: () =>
        noStoreJsonResponse({ error: 'not_found' }, { status: 404 }),
      OperatorArtanisConsoleSessionError: () =>
        noStoreJsonResponse({ error: 'session_error' }, { status: 500 }),
      OperatorArtanisConsoleStorageError: () =>
        noStoreJsonResponse(
          { error: 'artanis_operator_console_storage_error' },
          { status: 500 },
        ),
      OperatorArtanisConsoleUnauthorized: () =>
        noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 }),
      ArtanisPersistenceError: persistenceError =>
        noStoreJsonResponse(
          {
            error: `artanis_persistence_${persistenceError.kind}`,
            reason: persistenceError.reason,
          },
          { status: 500 },
        ),
    }),
    M.exhaustive,
  )

const requireAdminSession = <
  Session extends OperatorArtanisConsoleSession,
  Bindings extends OperatorArtanisConsoleEnv,
>(
  dependencies: OperatorArtanisConsoleDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
) =>
  Effect.gen(function* () {
    const requireAdminApiToken = dependencies.requireAdminApiToken

    if (requireAdminApiToken !== undefined) {
      const hasAdminApiToken = yield* Effect.tryPromise({
        catch: error => new OperatorArtanisConsoleSessionError({ error }),
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
      catch: error => new OperatorArtanisConsoleSessionError({ error }),
      try: () => dependencies.requireBrowserSession(request, env, ctx),
    })

    if (session === undefined) {
      return yield* new OperatorArtanisConsoleUnauthorized({})
    }

    if (!dependencies.isOpenAgentsAdminEmail(session.user.email)) {
      return yield* new OperatorArtanisConsoleForbidden({})
    }

    return session
  })

const readRows = (
  db: D1Database,
  kind: ArtanisPersistenceRecordKind,
  limit = 12,
) => readLatestArtanisPersistedRows(db, kind, limit)

const decodeRows = <A>(
  rows: ReadonlyArray<ArtanisPersistenceStoredRow>,
  schema: S.Decoder<A>,
): ReadonlyArray<A> =>
  rows.map(row => decodeUnknownWithSchema(schema, row.record))

const maybeDecodeLatest = <A>(
  rows: ReadonlyArray<ArtanisPersistenceStoredRow>,
  schema: S.Decoder<A>,
): A | null =>
  rows[0] === undefined
    ? null
    : decodeUnknownWithSchema(schema, rows[0].record)

const rowSummary = (
  row: ArtanisPersistenceStoredRow,
  nowIso: string,
) => ({
  kind: row.kind,
  recordRef: row.recordRef,
  state: row.state,
  updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
    row.updatedAtIso,
    nowIso,
  ),
})

const publicationLagSignal = (
  signals: ReadonlyArray<ArtanisHealthSignalProjection>,
): ArtanisHealthSignalProjection | null =>
  signals.find(signal => signal.kind === 'forum_publication_lag') ?? null

const consoleSnapshot = (input: {
  approvalRows: ReadonlyArray<ArtanisPersistenceStoredRow>
  forumRows: ReadonlyArray<ArtanisPersistenceStoredRow>
  healthRows: ReadonlyArray<ArtanisPersistenceStoredRow>
  loopRows: ReadonlyArray<ArtanisPersistenceStoredRow>
  runtimeRows: ReadonlyArray<ArtanisPersistenceStoredRow>
  tickRows: ReadonlyArray<ArtanisPersistenceStoredRow>
  workRows: ReadonlyArray<ArtanisPersistenceStoredRow>
  nowIso: string
}) => {
  const runtimeRecord = maybeDecodeLatest(
    input.runtimeRows,
    ArtanisRuntimeRecord,
  )
  const loopRecord = maybeDecodeLatest(input.loopRows, ArtanisLoopRecord)
  const tickRecords = decodeRows(input.tickRows, ArtanisLoopTickRecord)
  const healthRecord = maybeDecodeLatest(
    input.healthRows,
    ArtanisHealthSnapshotRecord,
  )
  const approvalRecords = decodeRows(
    input.approvalRows,
    ArtanisApprovalGateRecord,
  )
  const workRecords = decodeRows(
    input.workRows,
    ArtanisWorkRoutingProposalRecord,
  )
  const resolvedApprovalActionRefs = new Set(
    approvalRecords
      .filter(gate => gate.state !== 'pending')
      .map(gate => gate.actionRef),
  )
  const visibleApprovalRecords = approvalRecords.filter(
    gate =>
      gate.state !== 'pending' ||
      !resolvedApprovalActionRefs.has(gate.actionRef),
  )
  const forumRecords = decodeRows(
    input.forumRows,
    ArtanisForumPublicationIntentRecord,
  )
  const runtime =
    runtimeRecord === null
      ? null
      : projectArtanisRuntime(runtimeRecord, 'operator', input.nowIso)
  const loop =
    loopRecord === null
      ? null
      : {
          active: loopRecord.active,
          authority: ARTANIS_LOOP_READ_ONLY_AUTHORITY,
          blockerRefs: loopRecord.blockerRefs,
          caveatRefs: loopRecord.caveatRefs,
          lastTick:
            tickRecords[0] === undefined
              ? null
              : {
                  actionProposalRefs: tickRecords[0].actionProposals.map(
                    action => action.actionRef,
                  ),
                  approvalRequirementRefs: tickRecords[0].approvalRequirements.map(
                    approval => approval.approvalRef,
                  ),
                  blockerRefs: tickRecords[0].blockerRefs,
                  forumPublicationIntentRefs:
                    tickRecords[0].forumPublicationIntentRefs,
                  nextTickDisplay:
                    tickRecords[0].nextTickAtIso === null
                      ? null
                      : friendlyBlueprintMissionBriefingTime(
                          tickRecords[0].nextTickAtIso,
                          input.nowIso,
                        ),
                  selectedContextRefs: tickRecords[0].selectedContextRefs,
                  state: tickRecords[0].state,
                  tickRef: tickRecords[0].tickRef,
                  updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
                    tickRecords[0].updatedAtIso,
                    input.nowIso,
                  ),
                },
          loopRef: loopRecord.loopRef,
          scopeRef: loopRecord.scopeRef,
          state: loopRecord.state,
          tickCount: tickRecords.length,
          updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
            loopRecord.updatedAtIso,
            input.nowIso,
          ),
        }
  const health =
    healthRecord === null
      ? null
      : projectArtanisHealthSnapshot(healthRecord, 'operator', input.nowIso)
  const approvalGates =
    visibleApprovalRecords.length === 0
      ? null
      : projectArtanisApprovalGateLedger(
          new ArtanisApprovalGateLedgerRecord({
            agentId: 'agent_artanis',
            caveatRefs: visibleApprovalRecords.flatMap(gate => gate.caveatRefs),
            createdAtIso:
              visibleApprovalRecords[visibleApprovalRecords.length - 1]!
                .createdAtIso,
            gates: visibleApprovalRecords,
            ledgerRef: 'ledger.operator.artanis.console.approval_gates',
            updatedAtIso: visibleApprovalRecords[0]!.updatedAtIso,
          }),
          'operator',
          input.nowIso,
        )
  const workRouting =
    workRecords.length === 0
      ? null
      : projectArtanisWorkRoutingLedger(
          new ArtanisWorkRoutingLedgerRecord({
            agentId: 'agent_artanis',
            authority: ARTANIS_WORK_ROUTING_NO_DIRECT_AUTHORITY,
            caveatRefs: workRecords.flatMap(proposal => proposal.publicCaveatRefs),
            createdAtIso: workRecords[workRecords.length - 1]!.createdAtIso,
            ledgerRef: 'ledger.operator.artanis.console.work_routing',
            proposals: workRecords,
            publicStatusRefs: ['work_routing.public.artanis.operator_console'],
            updatedAtIso: workRecords[0]!.updatedAtIso,
          }),
          'operator',
          input.nowIso,
        )
  const publicationQueue =
    forumRecords.length === 0
      ? null
      : projectArtanisForumPublicationQueue(
          new ArtanisForumPublicationQueueRecord({
            agentId: 'agent_artanis',
            caveatRefs: forumRecords.flatMap(intent => intent.caveatRefs),
            createdAtIso: forumRecords[forumRecords.length - 1]!.createdAtIso,
            intents: forumRecords,
            queueRef: 'queue.operator.artanis.console.forum_publications',
            redactionPolicyRef:
              forumRecords[0]?.redactionPolicyRef ??
              'redaction.forum.public.artanis',
            updatedAtIso: forumRecords[0]!.updatedAtIso,
          }),
          input.nowIso,
        )
  const publicationLag =
    health === null ? null : publicationLagSignal(health.signals)
  const steering = projectArtanisOperatorSteeringWorkspace(
    exampleArtanisOperatorSteeringWorkspace,
    'operator',
    input.nowIso,
  )
  const latestRows = [
    ...input.runtimeRows,
    ...input.loopRows,
    ...input.tickRows,
    ...input.healthRows,
    ...input.approvalRows,
    ...input.workRows,
    ...input.forumRows,
  ].map(row => rowSummary(row, input.nowIso))

  return {
    agentId: 'agent_artanis',
    consoleRef: 'operator.artanis.console',
    latestRows,
    status: {
      blockerRefs: [
        ...(runtime?.blockerRefs ?? []),
        ...(loop?.blockerRefs ?? []),
        ...(health?.blockerRefs ?? []),
      ],
      healthState: health?.overallState ?? 'missing',
      lastTickRef: loop?.lastTick?.tickRef ?? null,
      loopState: loop?.state ?? 'missing',
      nextTickDisplay: loop?.lastTick?.nextTickDisplay ?? null,
      pendingApprovalCount:
        health?.pendingApprovalCount ?? approvalGates?.gateCount ?? 0,
      publicationLagLabel: publicationLag?.label ?? 'Forum lag not recorded',
      publicationLagState: publicationLag?.state ?? 'missing',
      runtimeState: runtime?.state ?? 'missing',
    },
    runtime,
    loop,
    health,
    approvalGates,
    workRouting,
    publicationQueue,
    steering,
  }
}

type OperatorApprovalAction = 'approve' | 'reject'

const routeApprovalActionPattern =
  /^\/api\/operator\/artanis\/approval-gates\/([^/]+)\/(approve|reject)$/

const refSuffix = (ref: string): string => {
  const suffix = ref
    .replace(/[^A-Za-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '')
    .toLowerCase()
    .slice(0, 120)

  return suffix === '' ? 'approval_gate' : suffix
}

const decisionGateRef = (
  source: ArtanisApprovalGateRecord,
  action: OperatorApprovalAction,
): string => `${source.gateRef}.${action === 'approve' ? 'approved' : 'denied'}`

const decisionState = (action: OperatorApprovalAction) =>
  action === 'approve' ? 'approved' as const : 'denied' as const

const approvalActionRecord = (input: {
  action: OperatorApprovalAction
  nowIso: string
  source: ArtanisApprovalGateRecord
}): ArtanisApprovalGateRecord => {
  const suffix = refSuffix(input.source.gateRef)
  const state = decisionState(input.action)

  return new ArtanisApprovalGateRecord({
    actionRef: input.source.actionRef,
    authorityReceiptRefs: [
      `authority.public.artanis.operator_${input.action}.${suffix}`,
    ],
    authoritySourceKinds: ['operator_approval', 'operator_policy'],
    caveatRefs: [
      ...input.source.caveatRefs,
      'caveat.public.operator_decision_not_execution_authority',
    ],
    createdAtIso: input.nowIso,
    expiresAtIso: input.source.expiresAtIso,
    gateRef: decisionGateRef(input.source, input.action),
    idempotencyKey: `artanis-operator-console:${input.action}:${input.source.gateRef}`,
    kind: input.source.kind,
    operatorReceiptRefs: [
      `receipt.operator.artanis.${input.action}_${suffix}`,
    ],
    policyRefs: input.source.policyRefs,
    privateEvidenceRefs: input.source.privateEvidenceRefs,
    publicStatusRefs: [
      `approval.public.artanis.${input.action}_${suffix}`,
    ],
    resolvedAtIso: input.nowIso,
    rollbackPosture: input.source.rollbackPosture,
    rollbackRefs: input.source.rollbackRefs,
    sourceRefs: [...input.source.sourceRefs, input.source.gateRef],
    state,
    supersededByGateRef: null,
    updatedAtIso: input.nowIso,
  })
}

const loadConsole = (
  db: D1Database,
  nowIso: string,
) =>
  Effect.gen(function* () {
    const rows = yield* Effect.all({
      approvalRows: readRows(db, 'approval_gate'),
      forumRows: readRows(db, 'forum_publication_intent'),
      healthRows: readRows(db, 'health_snapshot'),
      loopRows: readRows(db, 'loop_record'),
      runtimeRows: readRows(db, 'runtime_snapshot'),
      tickRows: readRows(db, 'loop_tick'),
      workRows: readRows(db, 'work_routing_proposal'),
    })

    return consoleSnapshot({ ...rows, nowIso })
  }).pipe(
    Effect.catch(error =>
      error instanceof Error
        ? Effect.fail(new OperatorArtanisConsoleStorageError({ error }))
        : Effect.fail(error),
    ),
  )

const applyApprovalAction = (
  db: D1Database,
  encodedGateRef: string,
  action: OperatorApprovalAction,
  nowIso: string,
) =>
  Effect.gen(function* () {
    const gateRef = decodeURIComponent(encodedGateRef)
    const rows = yield* readRows(db, 'approval_gate', 50)
    const records = decodeRows(rows, ArtanisApprovalGateRecord)
    const existingDecisionRef = (source: ArtanisApprovalGateRecord) =>
      decisionGateRef(source, action)
    const existingDecision = records.find(
      record =>
        record.gateRef === `${gateRef}.approved` ||
        record.gateRef === `${gateRef}.denied`,
    )

    if (existingDecision !== undefined) {
      return yield* loadConsole(db, nowIso)
    }

    const source = records.find(record => record.gateRef === gateRef)

    if (source === undefined || source.state !== 'pending') {
      return yield* new OperatorArtanisConsoleNotFound({})
    }

    if (
      records.some(record => record.gateRef === existingDecisionRef(source))
    ) {
      return yield* loadConsole(db, nowIso)
    }

    yield* saveArtanisApprovalGate(
      db,
      approvalActionRecord({ action, nowIso, source }),
      nowIso,
    )

    return yield* loadConsole(db, nowIso)
  })

export const makeOperatorArtanisConsoleRoutes = <
  Session extends OperatorArtanisConsoleSession,
  Bindings extends OperatorArtanisConsoleEnv,
>(
  dependencies: OperatorArtanisConsoleDependencies<Session, Bindings>,
) => ({
  routeOperatorArtanisConsoleRequest: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Effect.Effect<globalThis.Response> | undefined => {
    const url = new URL(request.url)
    const approvalActionMatch = routeApprovalActionPattern.exec(url.pathname)

    if (
      url.pathname !== '/api/operator/artanis/console' &&
      approvalActionMatch === null
    ) {
      return undefined
    }

    if (
      url.pathname === '/api/operator/artanis/console' &&
      request.method !== 'GET'
    ) {
      return Effect.succeed(methodNotAllowed(['GET']))
    }

    if (approvalActionMatch !== null && request.method !== 'POST') {
      return Effect.succeed(methodNotAllowed(['POST']))
    }

    return Effect.gen(function* () {
      const session = yield* requireAdminSession(
        dependencies,
        request,
        env,
        ctx,
      )
      const nowEpochMillis =
        dependencies.currentEpochMillis ?? currentEpochMillis
      const nowIso = epochMillisToIsoTimestamp(nowEpochMillis())
      const db = openAgentsDatabase(env)
      const snapshot =
        approvalActionMatch === null
          ? yield* loadConsole(db, nowIso)
          : yield* applyApprovalAction(
              db,
              approvalActionMatch[1]!,
              approvalActionMatch[2]! as OperatorApprovalAction,
              nowIso,
            )

      return dependencies.appendRefreshedSessionCookies(
        noStoreJsonResponse(snapshot),
        session,
      )
    }).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
  },
})
