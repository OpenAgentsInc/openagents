import { Effect, Match as M, Schema as S } from 'effect'

import type { AgentRegistrationStore } from './agent-registration'
import { sha256Hex } from './agent-registration'
import {
  type AutopilotWorkOrderRecord,
  type AutopilotWorkReviewAction,
  type AutopilotWorkReviewDecisionRecord,
  type AutopilotWorkStore,
  AutopilotWorkStoreError,
  authenticateAutopilotWorkRequest,
} from './autopilot-work-routes'
import {
  type CodingAutopilotDecisionActionKind,
  type CodingAutopilotDecisionActionProjection,
  type CodingAutopilotDecisionActionRecord,
  projectCodingAutopilotDecisionActionRecord,
} from './coding-autopilot-decision-actions'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  unauthorized,
} from './http/responses'
import { readJsonObject } from './json-boundary'
import { currentIsoTimestamp } from './runtime-primitives'

type HttpResponse = globalThis.Response

type AutopilotDecisionRouteEnv = Readonly<Record<string, unknown>>

type AutopilotDecisionRoutesDependencies<Bindings> = Readonly<{
  agentStore: (env: Bindings) => AgentRegistrationStore
  makeStore: (env: Bindings) => AutopilotWorkStore
  nowIso?: () => string
  requireBrowserSession?: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Readonly<{ user: Readonly<{ userId: string }> }> | undefined>
}>

const AUTOPILOT_DECISION_LIST_LIMIT = 200
const AUTOPILOT_COMPLETED_DECISION_LIMIT = 20

const decisionRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{2,240}$/

const safeDecisionRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref =>
    decisionRefPattern.test(ref)
  ))].sort()

const AutopilotDecisionActRequest = S.Struct({
  action: S.Literals(['accept', 'reject', 'request_changes']),
  decisionRefs: S.optionalKey(S.Array(S.String)),
  rejectionRefs: S.optionalKey(S.Array(S.String)),
  revisionRequestRefs: S.optionalKey(S.Array(S.String)),
})
type AutopilotDecisionActRequest = typeof AutopilotDecisionActRequest.Type

export type AutopilotWorkDecisionContext = Readonly<{
  createdAt: string
  state: AutopilotWorkOrderRecord['state']
  taskRefs: ReadonlyArray<string>
  updatedAt: string
  workOrderRef: string
}>

export type AutopilotDecisionQueueItem = Readonly<{
  decision: CodingAutopilotDecisionActionProjection
  work: AutopilotWorkDecisionContext
}>

const decisionActionRef = (
  workOrderRef: string,
  kind: CodingAutopilotDecisionActionKind,
): string => `decision_action.${workOrderRef}.${kind}`

const workDecisionContext = (
  record: AutopilotWorkOrderRecord,
): AutopilotWorkDecisionContext => ({
  createdAt: record.createdAt,
  state: record.state,
  taskRefs: safeDecisionRefs(record.taskRefs),
  updatedAt: record.updatedAt,
  workOrderRef: record.workOrderRef,
})

const pendingReviewDecisionRecord = (
  record: AutopilotWorkOrderRecord,
): CodingAutopilotDecisionActionRecord =>
  ({
    accountLeaseRefs: [],
    actionKind: 'approve_pr_draft',
    actionRef: decisionActionRef(record.workOrderRef, 'approve_pr_draft'),
    actionSubmissionRefs: [
      `action_submission.review.${record.workOrderRef}`,
    ],
    assignmentRefs: safeDecisionRefs(
      record.executionCloseout?.assignmentRefs ?? [],
    ),
    blockedReasonRefs: [],
    createdAtIso: record.updatedAt,
    customerNextActionRef: 'next_action.review_delivered_work',
    evidenceRefs: safeDecisionRefs([
      ...(record.executionCloseout?.closeoutRefs ?? []),
      ...(record.executionCloseout?.proofRefs ?? []),
      ...(record.executionCloseout?.resultRefs ?? []),
    ]),
    id: decisionActionRef(record.workOrderRef, 'approve_pr_draft'),
    missionRef: `mission.${record.workOrderRef}`,
    prerequisiteRefs: [],
    programRunRef: null,
    receiptRefs: [],
    routeRefs: [],
    safeSummaryRef: 'summary.delivered_work_awaits_review',
    sourceAuthorityRefs: [],
    status: 'available',
    updatedAtIso: record.updatedAt,
    workroomRefs: [],
  })

const blockedCustomerInputDecisionRecord = (
  record: AutopilotWorkOrderRecord,
): CodingAutopilotDecisionActionRecord => {
  const blockedReasonRefs = record.state === 'access_required'
    ? safeDecisionRefs([
        'blocked.access_required',
        ...record.accessRequestRefs,
      ])
    : ['blocked.payment_required']

  return {
    accountLeaseRefs: [],
    actionKind: 'request_customer_input',
    actionRef: decisionActionRef(record.workOrderRef, 'request_customer_input'),
    actionSubmissionRefs: [],
    assignmentRefs: [],
    blockedReasonRefs,
    createdAtIso: record.updatedAt,
    customerNextActionRef: record.state === 'access_required'
      ? 'next_action.grant_required_access'
      : 'next_action.fund_payment_challenge',
    evidenceRefs: [],
    id: decisionActionRef(record.workOrderRef, 'request_customer_input'),
    missionRef: `mission.${record.workOrderRef}`,
    prerequisiteRefs: safeDecisionRefs(record.accessRequestRefs),
    programRunRef: null,
    receiptRefs: [],
    routeRefs: [],
    safeSummaryRef: record.state === 'access_required'
      ? 'summary.customer_access_required'
      : 'summary.customer_payment_required',
    sourceAuthorityRefs: [],
    status: 'blocked',
    updatedAtIso: record.updatedAt,
    workroomRefs: [],
  }
}

const completedReviewDecisionRecord = (
  record: AutopilotWorkOrderRecord,
  reviewDecision: AutopilotWorkReviewDecisionRecord,
): CodingAutopilotDecisionActionRecord =>
  ({
    accountLeaseRefs: [],
    actionKind: 'approve_pr_draft',
    actionRef: decisionActionRef(record.workOrderRef, 'approve_pr_draft'),
    actionSubmissionRefs: [
      `action_submission.review.${record.workOrderRef}`,
    ],
    assignmentRefs: safeDecisionRefs(
      record.executionCloseout?.assignmentRefs ?? [],
    ),
    blockedReasonRefs: [],
    createdAtIso: reviewDecision.recordedAt,
    customerNextActionRef: 'next_action.review_recorded',
    evidenceRefs: safeDecisionRefs([
      ...(record.executionCloseout?.closeoutRefs ?? []),
      ...(record.executionCloseout?.resultRefs ?? []),
    ]),
    id: decisionActionRef(record.workOrderRef, 'approve_pr_draft'),
    missionRef: `mission.${record.workOrderRef}`,
    prerequisiteRefs: [],
    programRunRef: null,
    receiptRefs: safeDecisionRefs([
      `receipt.review.${reviewDecision.action}.${record.workOrderRef}`,
      ...reviewDecision.decisionRefs,
      ...reviewDecision.rejectionRefs,
      ...reviewDecision.revisionRequestRefs,
    ]),
    routeRefs: [],
    safeSummaryRef: 'summary.review_decision_recorded',
    sourceAuthorityRefs: [],
    status: 'completed',
    updatedAtIso: record.updatedAt,
    workroomRefs: [],
  })

export const decisionRecordsForWorkOrder = (
  record: AutopilotWorkOrderRecord,
): ReadonlyArray<CodingAutopilotDecisionActionRecord> => {
  if (record.state === 'delivered' && record.reviewDecision === null) {
    return [pendingReviewDecisionRecord(record)]
  }

  if (
    record.state === 'access_required' ||
    record.state === 'payment_required'
  ) {
    return [blockedCustomerInputDecisionRecord(record)]
  }

  if (record.reviewDecision !== null) {
    return [completedReviewDecisionRecord(record, record.reviewDecision)]
  }

  return []
}

const projectedQueueItems = (
  records: ReadonlyArray<AutopilotWorkOrderRecord>,
  nowIso: string,
): ReadonlyArray<AutopilotDecisionQueueItem> =>
  records.flatMap(record =>
    decisionRecordsForWorkOrder(record).flatMap(decisionRecord => {
      try {
        return [
          {
            decision: projectCodingAutopilotDecisionActionRecord(
              decisionRecord,
              'customer',
              nowIso,
            ),
            work: workDecisionContext(record),
          },
        ]
      } catch {
        return []
      }
    })
  )

const decisionSortKey = (item: AutopilotDecisionQueueItem): number =>
  item.decision.status === 'completed' ? 1 : 0

const orderedQueueItems = (
  items: ReadonlyArray<AutopilotDecisionQueueItem>,
): ReadonlyArray<AutopilotDecisionQueueItem> => {
  const byMostRecent = (
    left: AutopilotDecisionQueueItem,
    right: AutopilotDecisionQueueItem,
  ): number => right.work.updatedAt.localeCompare(left.work.updatedAt)
  const pending = [...items.filter(item => decisionSortKey(item) === 0)]
    .sort(byMostRecent)
  const completed = [...items.filter(item => decisionSortKey(item) === 1)]
    .sort(byMostRecent)
    .slice(0, AUTOPILOT_COMPLETED_DECISION_LIMIT)

  return [...pending, ...completed]
}

const routeNowIso = <Bindings>(
  dependencies: AutopilotDecisionRoutesDependencies<Bindings>,
): string => dependencies.nowIso?.() ?? currentIsoTimestamp()

const routeErrorResponse = (error: AutopilotWorkStoreError): HttpResponse =>
  noStoreJsonResponse(
    { error: `autopilot_decision_${error.kind}`, reason: error.reason },
    {
      status:
        error.kind === 'conflict'
          ? 409
          : error.kind === 'not_found'
            ? 404
            : error.kind === 'storage_error'
              ? 500
              : 400,
    },
  )

const requireIdempotencyHash = (
  request: Request,
): Effect.Effect<string, AutopilotWorkStoreError> => {
  const value = request.headers.get('Idempotency-Key')?.trim()

  if (value === undefined || value === '') {
    return Effect.fail(
      new AutopilotWorkStoreError({
        kind: 'validation_error',
        reason: 'Idempotency-Key header is required.',
      }),
    )
  }

  return Effect.promise(() => sha256Hex(value))
}

const decodeDecisionActRequest = (
  request: Request,
): Effect.Effect<AutopilotDecisionActRequest, AutopilotWorkStoreError> =>
  Effect.tryPromise({
    catch: error =>
      new AutopilotWorkStoreError({
        kind: 'validation_error',
        reason: error instanceof Error ? error.message : String(error),
      }),
    try: async () =>
      S.decodeUnknownSync(AutopilotDecisionActRequest)(
        await readJsonObject(request),
      ),
  })

const validateDecisionActRefs = (
  body: AutopilotDecisionActRequest,
): Effect.Effect<AutopilotDecisionActRequest, AutopilotWorkStoreError> => {
  const provided = [
    ...(body.decisionRefs ?? []),
    ...(body.rejectionRefs ?? []),
    ...(body.revisionRequestRefs ?? []),
  ]

  return provided.every(ref => decisionRefPattern.test(ref.trim()))
    ? Effect.succeed(body)
    : Effect.fail(
        new AutopilotWorkStoreError({
          kind: 'validation_error',
          reason:
            'Autopilot decision actions require public-safe decision refs.',
        }),
      )
}

const reviewStateForAction = (
  action: AutopilotWorkReviewAction,
): 'accepted' | 'rejected' | 'revision_required' =>
  action === 'accept'
    ? 'accepted'
    : action === 'reject'
      ? 'rejected'
      : 'revision_required'

const reviewDecisionForDecisionAct = (
  input: Readonly<{
    actorAgentCredentialId: string
    actorAgentUserId: string
    body: AutopilotDecisionActRequest
    idempotencyKeyHash: string
    nowIso: string
    workOrderRef: string
  }>,
): AutopilotWorkReviewDecisionRecord => {
  const defaultRef =
    `decision.queue.${input.body.action}.${input.workOrderRef}`

  return {
    action: input.body.action,
    actorAgentCredentialId: input.actorAgentCredentialId,
    actorAgentUserId: input.actorAgentUserId,
    decisionRefs: input.body.action === 'accept'
      ? safeDecisionRefs([...(input.body.decisionRefs ?? []), defaultRef])
      : safeDecisionRefs(input.body.decisionRefs ?? []),
    idempotencyKeyHash: input.idempotencyKeyHash,
    recordedAt: input.nowIso,
    rejectionRefs: input.body.action === 'reject'
      ? safeDecisionRefs([...(input.body.rejectionRefs ?? []), defaultRef])
      : safeDecisionRefs(input.body.rejectionRefs ?? []),
    revisionRequestRefs: input.body.action === 'request_changes'
      ? safeDecisionRefs([
          ...(input.body.revisionRequestRefs ?? []),
          defaultRef,
        ])
      : safeDecisionRefs(input.body.revisionRequestRefs ?? []),
  }
}

const decisionActionsRefFromPath = (pathname: string): string | undefined => {
  const match = /^\/api\/autopilot\/decisions\/([^/]+)\/actions$/.exec(pathname)

  return match?.[1] === undefined ? undefined : decodeURIComponent(match[1])
}

const workOrderRefFromDecisionRef = (
  decisionRef: string,
): string | undefined => {
  const match = /^decision_action\.(.+)\.approve_pr_draft$/.exec(decisionRef)

  return match?.[1]
}

const listDecisions = <Bindings extends AutopilotDecisionRouteEnv>(
  dependencies: AutopilotDecisionRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const nowIso = routeNowIso(dependencies)
    const auth = yield* authenticateAutopilotWorkRequest(
      dependencies,
      request,
      env,
      {
        ctx,
        nowIso: () => nowIso,
        requiredScope: 'customer_orders.read',
      },
    )
    const records = yield* Effect.tryPromise({
      catch: error =>
        new AutopilotWorkStoreError({
          kind: 'storage_error',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () =>
        dependencies.makeStore(env).listWorkOrdersForOwner({
          limit: AUTOPILOT_DECISION_LIST_LIMIT,
          ownerUserId: auth.ownerUserId,
        }),
    })
    const decisions = orderedQueueItems(projectedQueueItems(records, nowIso))
    const pendingCount = decisions.filter(
      item => item.decision.status !== 'completed',
    ).length

    return noStoreJsonResponse({
      decisions,
      directEffectPermitted: false,
      generatedAt: nowIso,
      pendingCount,
    })
  }).pipe(
    Effect.catchTag('CustomerOrderAgentAuthFailure', () =>
      Effect.succeed(unauthorized())
    ),
    Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
  )

const actOnDecision = <Bindings extends AutopilotDecisionRouteEnv>(
  dependencies: AutopilotDecisionRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  decisionRef: string,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const nowIso = routeNowIso(dependencies)
    const auth = yield* authenticateAutopilotWorkRequest(
      dependencies,
      request,
      env,
      {
        ctx,
        nowIso: () => nowIso,
        requiredScope: 'customer_orders.write',
      },
    )
    const workOrderRef = workOrderRefFromDecisionRef(decisionRef)

    if (workOrderRef === undefined) {
      return yield* new AutopilotWorkStoreError({
        kind: 'validation_error',
        reason:
          'Only approve_pr_draft decision actions are actionable through the decision queue.',
      })
    }

    const idempotencyKeyHash = yield* requireIdempotencyHash(request)
    const body = yield* Effect.flatMap(
      decodeDecisionActRequest(request),
      validateDecisionActRefs,
    )
    const reviewDecision = reviewDecisionForDecisionAct({
      actorAgentCredentialId: auth.actorAgentCredentialId,
      actorAgentUserId: auth.actorAgentUserId,
      body,
      idempotencyKeyHash,
      nowIso,
      workOrderRef,
    })
    const result = yield* Effect.tryPromise({
      catch: error =>
        error instanceof AutopilotWorkStoreError
          ? error
          : new AutopilotWorkStoreError({
              kind: 'storage_error',
              reason: error instanceof Error ? error.message : String(error),
            }),
      try: () =>
        dependencies.makeStore(env).recordReviewDecision({
          ownerUserId: auth.ownerUserId,
          reviewDecision,
          state: reviewStateForAction(body.action),
          updatedAt: nowIso,
          workOrderRef,
        }),
    })

    if (result === undefined) {
      return noStoreJsonResponse(
        {
          error: 'autopilot_decision_not_found',
          reason: 'Autopilot decision was not found.',
        },
        { status: 404 },
      )
    }

    const items = orderedQueueItems(
      projectedQueueItems([result.record], nowIso),
    )
    const item = items.find(
      candidate => candidate.decision.actionKind === 'approve_pr_draft',
    )

    return noStoreJsonResponse(
      {
        decision: item?.decision ?? null,
        directEffectPermitted: false,
        generatedAt: nowIso,
        idempotent: result.idempotent,
        work: workDecisionContext(result.record),
      },
      { status: result.idempotent ? 200 : 201 },
    )
  }).pipe(
    Effect.catchTag('CustomerOrderAgentAuthFailure', () =>
      Effect.succeed(unauthorized())
    ),
    Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
  )

export const makeAutopilotDecisionRoutes = <
  Bindings extends AutopilotDecisionRouteEnv,
>(
  dependencies: AutopilotDecisionRoutesDependencies<Bindings>,
) => ({
  routeAutopilotDecisionRequest: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (url.pathname === '/api/autopilot/decisions') {
      return M.value(request.method).pipe(
        M.when('GET', () => listDecisions(dependencies, request, env, ctx)),
        M.orElse(() => Effect.succeed(methodNotAllowed(['GET']))),
      )
    }

    const decisionRef = decisionActionsRefFromPath(url.pathname)

    if (decisionRef !== undefined) {
      return M.value(request.method).pipe(
        M.when('POST', () =>
          actOnDecision(dependencies, request, env, ctx, decisionRef)
        ),
        M.orElse(() => Effect.succeed(methodNotAllowed(['POST']))),
      )
    }

    return undefined
  },
})
