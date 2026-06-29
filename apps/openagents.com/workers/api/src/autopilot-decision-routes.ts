import { Effect, Match as M, Schema as S } from 'effect'

import type { AgentRegistrationStore } from './agent-registration'
import { sha256Hex } from './agent-registration'
import {
  type AutopilotDecisionActionableKind,
  type AutopilotDecisionActRequest as EvidenceAutopilotDecisionActRequest,
  authorizeAutopilotDecisionAct,
} from './autopilot-decision-act'
import {
  classifyAutopilotDecisionActRoute,
} from './autopilot-decision-act-routing'
import {
  type AutopilotDecisionCloseoutReceipt,
  buildAutopilotDecisionCloseoutReceipt,
  validateAutopilotDecisionCloseoutReceipt,
} from './autopilot-decision-closeout'
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

export const AutopilotDecisionCommandAction = S.Literals([
  'accept',
  'continue',
  'create-follow-up-mission',
  'provide-context',
  'rerun-tests',
  'retry-with-another-account',
  'steer',
  'stop',
])
export type AutopilotDecisionCommandAction =
  typeof AutopilotDecisionCommandAction.Type

const LegacyAutopilotDecisionReviewAction = S.Literals([
  'reject',
  'request_changes',
])

const AutopilotDecisionActRequest = S.Struct({
  action: S.Union([
    AutopilotDecisionCommandAction,
    LegacyAutopilotDecisionReviewAction,
  ]),
  contextRefs: S.optionalKey(S.Array(S.String)),
  decisionRefs: S.optionalKey(S.Array(S.String)),
  ownerApprovalRef: S.optionalKey(S.String),
  rejectionRefs: S.optionalKey(S.Array(S.String)),
  revisionRequestRefs: S.optionalKey(S.Array(S.String)),
})
type AutopilotDecisionActRequest = typeof AutopilotDecisionActRequest.Type

const commandActionToDecisionKind: Record<
  AutopilotDecisionCommandAction,
  AutopilotDecisionActionableKind
> = {
  accept: 'approve_pr_draft',
  continue: 'continue',
  'create-follow-up-mission': 'create_followup_mission',
  'provide-context': 'provide_context',
  'rerun-tests': 'rerun_tests',
  'retry-with-another-account': 'retry_account',
  steer: 'steer',
  stop: 'stop',
}

const ownerApprovalRequiredActions: ReadonlySet<AutopilotDecisionCommandAction> =
  new Set([
    'create-follow-up-mission',
    'retry-with-another-account',
    'stop',
  ])

const evidenceRequestForCommand = (
  body: AutopilotDecisionActRequest,
): EvidenceAutopilotDecisionActRequest | undefined => {
  if (body.action === 'reject' || body.action === 'request_changes') {
    return undefined
  }

  const resolution = commandActionToDecisionKind[body.action]

  if (resolution === 'approve_pr_draft') {
    return undefined
  }

  return {
    resolution,
    verb: 'submit',
    ...(body.contextRefs === undefined
      ? {}
      : { contextRefs: body.contextRefs }),
  }
}

export type AutopilotWorkDecisionContext = Readonly<{
  createdAt: string
  state: AutopilotWorkOrderRecord['state']
  taskRefs: ReadonlyArray<string>
  updatedAt: string
  workOrderRef: string
}>

export type AutopilotDecisionQueueItem = Readonly<{
  closeoutReceipts: ReadonlyArray<AutopilotDecisionCloseoutReceipt>
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
  closeoutReceiptsByWorkOrder: ReadonlyMap<
    string,
    ReadonlyArray<AutopilotDecisionCloseoutReceipt>
  > = new Map(),
): ReadonlyArray<AutopilotDecisionQueueItem> =>
  records.flatMap(record =>
    decisionRecordsForWorkOrder(record).flatMap(decisionRecord => {
      try {
        return [
          {
            closeoutReceipts:
              closeoutReceiptsByWorkOrder.get(record.workOrderRef) ?? [],
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

const closeoutReceiptsForWorkOrder = <Bindings>(
  dependencies: AutopilotDecisionRoutesDependencies<Bindings>,
  env: Bindings,
  input: Readonly<{ ownerUserId: string; workOrderRef: string }>,
): Effect.Effect<
  ReadonlyArray<AutopilotDecisionCloseoutReceipt>,
  AutopilotWorkStoreError
> =>
  Effect.tryPromise({
    catch: error =>
      new AutopilotWorkStoreError({
        kind: 'storage_error',
        reason: error instanceof Error ? error.message : String(error),
      }),
    try: async () =>
      dependencies.makeStore(env).listDecisionCloseoutReceiptsForWorkOrder?.(
        input,
      ) ?? [],
  }).pipe(
    Effect.map(receipts =>
      receipts.filter(receipt =>
        validateAutopilotDecisionCloseoutReceipt(receipt)
      )
    ),
  )

const closeoutReceiptMapForRecords = <Bindings>(
  dependencies: AutopilotDecisionRoutesDependencies<Bindings>,
  env: Bindings,
  ownerUserId: string,
  records: ReadonlyArray<AutopilotWorkOrderRecord>,
): Effect.Effect<
  ReadonlyMap<string, ReadonlyArray<AutopilotDecisionCloseoutReceipt>>,
  AutopilotWorkStoreError
> =>
  Effect.forEach(records, record =>
    Effect.map(
      closeoutReceiptsForWorkOrder(dependencies, env, {
        ownerUserId,
        workOrderRef: record.workOrderRef,
      }),
      receipts => [record.workOrderRef, receipts] as const,
    )
  ).pipe(Effect.map(entries => new Map(entries)))

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
    ...(body.contextRefs ?? []),
    ...(body.decisionRefs ?? []),
    ...(body.ownerApprovalRef === undefined ? [] : [body.ownerApprovalRef]),
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
  const action = reviewActionForCommand(input.body)
  const defaultRef = `decision.queue.${action}.${input.workOrderRef}`

  return {
    action,
    actorAgentCredentialId: input.actorAgentCredentialId,
    actorAgentUserId: input.actorAgentUserId,
    decisionRefs: action === 'accept'
      ? safeDecisionRefs([...(input.body.decisionRefs ?? []), defaultRef])
      : safeDecisionRefs(input.body.decisionRefs ?? []),
    idempotencyKeyHash: input.idempotencyKeyHash,
    recordedAt: input.nowIso,
    rejectionRefs: action === 'reject'
      ? safeDecisionRefs([...(input.body.rejectionRefs ?? []), defaultRef])
      : safeDecisionRefs(input.body.rejectionRefs ?? []),
    revisionRequestRefs: action === 'request_changes'
      ? safeDecisionRefs([
          ...(input.body.revisionRequestRefs ?? []),
          defaultRef,
        ])
      : safeDecisionRefs(input.body.revisionRequestRefs ?? []),
  }
}

const reviewActionForCommand = (
  body: AutopilotDecisionActRequest,
): AutopilotWorkReviewAction => {
  if (body.action === 'reject' || body.action === 'request_changes') {
    return body.action
  }

  return 'accept'
}

const decisionActionsRefFromPath = (pathname: string): string | undefined => {
  const match = /^\/api\/autopilot\/decisions\/([^/]+)\/actions$/.exec(pathname)

  return match?.[1] === undefined ? undefined : decodeURIComponent(match[1])
}

const workOrderRefFromDecisionRef = (
  decisionRef: string,
): string | undefined => {
  const match = /^decision_action\.(.+)\.[A-Za-z0-9_]+$/.exec(decisionRef)

  return match?.[1]
}

const decisionKindFromDecisionRef = (
  decisionRef: string,
): CodingAutopilotDecisionActionKind | undefined => {
  const match = /^decision_action\..+\.([A-Za-z0-9_]+)$/.exec(decisionRef)
  const kind = match?.[1]

  if (
    kind === 'approve_pr_draft' ||
    kind === 'continue' ||
    kind === 'create_followup_mission' ||
    kind === 'mark_unavailable' ||
    kind === 'provide_context' ||
    kind === 'request_customer_input' ||
    kind === 'rerun_tests' ||
    kind === 'retry_account' ||
    kind === 'steer' ||
    kind === 'stop'
  ) {
    return kind
  }

  return undefined
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
    const closeoutReceiptsByWorkOrder = yield* closeoutReceiptMapForRecords(
      dependencies,
      env,
      auth.ownerUserId,
      records,
    )
    const decisions = orderedQueueItems(
      projectedQueueItems(records, nowIso, closeoutReceiptsByWorkOrder),
    )
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

const listDecisionsForWorkOrder = <Bindings extends AutopilotDecisionRouteEnv>(
  dependencies: AutopilotDecisionRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  workOrderRef: string,
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
    const record = yield* Effect.tryPromise({
      catch: error =>
        new AutopilotWorkStoreError({
          kind: 'storage_error',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () => dependencies.makeStore(env).readWorkOrder(workOrderRef),
    })

    if (record === undefined || record.ownerUserId !== auth.ownerUserId) {
      return noStoreJsonResponse(
        {
          error: 'autopilot_decision_not_found',
          reason: 'Autopilot work-order decision projection was not found.',
        },
        { status: 404 },
      )
    }

    const closeoutReceipts = yield* closeoutReceiptsForWorkOrder(
      dependencies,
      env,
      { ownerUserId: auth.ownerUserId, workOrderRef },
    )
    const decisions = orderedQueueItems(
      projectedQueueItems(
        [record],
        nowIso,
        new Map([[workOrderRef, closeoutReceipts]]),
      ),
    )

    return noStoreJsonResponse({
      decisions,
      directEffectPermitted: false,
      generatedAt: nowIso,
      pendingCount: decisions.filter(
        item => item.decision.status !== 'completed',
      ).length,
      work: workDecisionContext(record),
    })
  }).pipe(
    Effect.catchTag('CustomerOrderAgentAuthFailure', () =>
      Effect.succeed(unauthorized())
    ),
    Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
  )

const closeoutRefFromPath = (pathname: string): string | undefined => {
  const match = /^\/api\/autopilot\/decision-closeouts\/([^/]+)$/.exec(
    pathname,
  )

  return match?.[1] === undefined ? undefined : decodeURIComponent(match[1])
}

const decisionsWorkOrderRefFromPath = (pathname: string): string | undefined => {
  const match = /^\/api\/autopilot\/work\/([^/]+)\/decisions$/.exec(pathname)

  return match?.[1] === undefined ? undefined : decodeURIComponent(match[1])
}

const readDecisionCloseout = <Bindings extends AutopilotDecisionRouteEnv>(
  dependencies: AutopilotDecisionRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  closeoutRef: string,
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
    const receipt = yield* Effect.tryPromise({
      catch: error =>
        new AutopilotWorkStoreError({
          kind: 'storage_error',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () =>
        dependencies.makeStore(env).readDecisionCloseoutReceipt?.({
          closeoutRef,
          ownerUserId: auth.ownerUserId,
        }) ?? Promise.resolve(undefined),
    })

    if (
      receipt === undefined ||
      !validateAutopilotDecisionCloseoutReceipt(receipt)
    ) {
      return noStoreJsonResponse(
        {
          error: 'autopilot_decision_closeout_not_found',
          reason: 'Autopilot decision closeout receipt was not found.',
        },
        { status: 404 },
      )
    }

    return noStoreJsonResponse({
      directEffectPermitted: false,
      generatedAt: nowIso,
      receipt,
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
    const decisionKind = decisionKindFromDecisionRef(decisionRef)

    if (workOrderRef === undefined || decisionKind === undefined) {
      return yield* new AutopilotWorkStoreError({
        kind: 'validation_error',
        reason:
          'Autopilot decision action refs must name a known decision kind.',
      })
    }

    const idempotencyKeyHash = yield* requireIdempotencyHash(request)
    const body = yield* Effect.flatMap(
      decodeDecisionActRequest(request),
      validateDecisionActRefs,
    )
    const expectedKind = body.action === 'reject' || body.action === 'request_changes'
      ? 'approve_pr_draft'
      : commandActionToDecisionKind[body.action]

    if (expectedKind !== decisionKind) {
      return yield* new AutopilotWorkStoreError({
        kind: 'validation_error',
        reason:
          `Decision command ${body.action} cannot resolve ${decisionKind}.`,
      })
    }

    if (
      body.action !== 'reject' &&
      body.action !== 'request_changes' &&
      ownerApprovalRequiredActions.has(body.action) &&
      (body.ownerApprovalRef ?? '').trim() === ''
    ) {
      return noStoreJsonResponse(
        {
          authorityBoundary: 'owner_approval_required',
          directEffectPermitted: false,
          error: 'autopilot_decision_owner_approval_required',
          generatedAt: nowIso,
          idempotent: false,
          reason:
            `Decision command ${body.action} requires ownerApprovalRef before it can be applied.`,
        },
        { status: 403 },
      )
    }

    if (decisionKind !== 'approve_pr_draft') {
      const evidenceRequest = evidenceRequestForCommand(body)
      const routing = classifyAutopilotDecisionActRoute({
        actionKind: decisionKind,
        actionRef: decisionRef,
        status: 'available',
      })

      if (routing.route !== 'evidence_command' || evidenceRequest === undefined) {
        return yield* new AutopilotWorkStoreError({
          kind: 'validation_error',
          reason:
            `Decision kind ${decisionKind} is not actionable through this command API.`,
        })
      }

      const authorized = authorizeAutopilotDecisionAct({
        request: evidenceRequest,
        target: routing.target,
      })

      if (!authorized.ok) {
        return noStoreJsonResponse(
          {
            directEffectPermitted: false,
            error: 'autopilot_decision_command_rejected',
            errors: authorized.errors,
            generatedAt: nowIso,
            idempotent: false,
          },
          { status: 400 },
        )
      }

      return noStoreJsonResponse(
        {
          command: {
            ...authorized.command,
            ownerApprovalRef: body.ownerApprovalRef ?? null,
          },
          directEffectPermitted: false,
          generatedAt: nowIso,
          idempotent: false,
          receipt: {
            closeoutRef: authorized.command.closeoutRef,
            outcome: 'accepted_for_evidence',
          },
        },
        { status: 202 },
      )
    }

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
          state: reviewStateForAction(reviewDecision.action),
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
    // Receipt-backed closeout for the live review path: a canonical,
    // tamper-verifiable artifact a later audit can dereference. The closeoutRef
    // is identical across an idempotent replay, so a downstream ledger records
    // exactly one closeout per resolved decision.
    const closeout = buildAutopilotDecisionCloseoutReceipt({
      decisionRef,
      idempotent: result.idempotent,
      decidedAt: nowIso,
      reviewDecision,
      workOrderRef,
    })
    yield* Effect.tryPromise({
      catch: error =>
        new AutopilotWorkStoreError({
          kind: 'storage_error',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () =>
        dependencies.makeStore(env).recordDecisionCloseoutReceipt?.(closeout) ??
        Promise.resolve(),
    })

    return noStoreJsonResponse(
      {
        closeout,
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

    const workOrderRef = decisionsWorkOrderRefFromPath(url.pathname)

    if (workOrderRef !== undefined) {
      return M.value(request.method).pipe(
        M.when('GET', () =>
          listDecisionsForWorkOrder(
            dependencies,
            request,
            env,
            ctx,
            workOrderRef,
          )
        ),
        M.orElse(() => Effect.succeed(methodNotAllowed(['GET']))),
      )
    }

    const closeoutRef = closeoutRefFromPath(url.pathname)

    if (closeoutRef !== undefined) {
      return M.value(request.method).pipe(
        M.when('GET', () =>
          readDecisionCloseout(dependencies, request, env, ctx, closeoutRef)
        ),
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
