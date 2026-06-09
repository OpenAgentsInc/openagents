import { Effect, Match as M, Schema as S } from 'effect'

import {
  type AgentRegistrationStore,
  sha256Hex,
} from './agent-registration'
import {
  authenticateCustomerOrderAgentRequest,
} from './customer-order-agent-auth'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  unauthorized,
} from './http/responses'
import { readJsonObject } from './json-boundary'
import { currentIsoTimestamp, randomUuid } from './runtime-primitives'
import {
  OpenAgentsAutopilotWorkState,
  type OpenAgentsAutopilotWorkRequest,
  type OpenAgentsAutopilotWorkState as OpenAgentsAutopilotWorkStateType,
  decodeOpenAgentsAutopilotWorkRequest,
} from './autopilot-work-request'

type HttpResponse = globalThis.Response

export class AutopilotWorkStoreError extends S.TaggedErrorClass<AutopilotWorkStoreError>()(
  'AutopilotWorkStoreError',
  {
    kind: S.Literals([
      'conflict',
      'not_found',
      'storage_error',
      'validation_error',
    ]),
    reason: S.String,
  },
) {}

export type AutopilotWorkStoreErrorKind = AutopilotWorkStoreError['kind']

export type AutopilotWorkOrderRecord = Readonly<{
  accessRequestRefs: ReadonlyArray<string>
  agentCredentialId: string
  agentUserId: string
  archivedAt: string | null
  clientRequestRef: string
  createdAt: string
  eventStreamRef: string
  id: string
  idempotencyKeyHash: string
  ownerUserId: string
  paymentChallengeRef: string | null
  request: OpenAgentsAutopilotWorkRequest
  state: OpenAgentsAutopilotWorkStateType
  statusUrlRef: string
  taskRefs: ReadonlyArray<string>
  updatedAt: string
  workOrderRef: string
}>

export type AutopilotWorkOrderProjection = Readonly<{
  accessRequestRefs: ReadonlyArray<string>
  clientRequestRef: string
  createdAt: string
  eventStreamRef: string
  idempotent: boolean
  paymentChallengeRef: string | null
  state: OpenAgentsAutopilotWorkStateType
  statusUrlRef: string
  taskRefs: ReadonlyArray<string>
  updatedAt: string
  workOrderRef: string
}>

export type AutopilotWorkEventKind =
  | 'accepted'
  | 'blocked'
  | 'delivered'
  | 'needs_access'
  | 'payment_required'
  | 'queued'
  | 'running'
  | 'settled'

export type AutopilotWorkEventProjection = Readonly<{
  eventKind: AutopilotWorkEventKind
  eventRef: string
  occurredAt: string
  publicSafe: true
  sequence: number
  state: OpenAgentsAutopilotWorkStateType
  taskRefs: ReadonlyArray<string>
  workOrderRef: string
}>

export type AutopilotWorkStore = Readonly<{
  createWorkOrder: (
    record: AutopilotWorkOrderRecord,
  ) => Promise<Readonly<{ idempotent: boolean; record: AutopilotWorkOrderRecord }>>
  readWorkOrder: (
    workOrderRef: string,
  ) => Promise<AutopilotWorkOrderRecord | undefined>
  readWorkOrderByIdempotency: (
    ownerUserId: string,
    idempotencyKeyHash: string,
  ) => Promise<AutopilotWorkOrderRecord | undefined>
}>

type AutopilotWorkRoutesDependencies<Bindings> = Readonly<{
  agentStore: (env: Bindings) => AgentRegistrationStore
  makeId?: () => string
  makeStore: (env: Bindings) => AutopilotWorkStore
  nowIso?: () => string
}>

type AutopilotWorkRouteEnv = Readonly<Record<string, unknown>>

const routeErrorResponse = (error: AutopilotWorkStoreError): HttpResponse =>
  noStoreJsonResponse(
    { error: `autopilot_work_${error.kind}`, reason: error.reason },
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

const idempotencyKeyFromRequest = (request: Request): string | undefined => {
  const value = request.headers.get('Idempotency-Key')?.trim()

  return value === undefined || value === '' ? undefined : value
}

const requireIdempotencyHash = (
  request: Request,
): Effect.Effect<string, AutopilotWorkStoreError> => {
  const idempotencyKey = idempotencyKeyFromRequest(request)

  if (idempotencyKey === undefined) {
    return Effect.fail(
      new AutopilotWorkStoreError({
        kind: 'validation_error',
        reason: 'Idempotency-Key header is required.',
      }),
    )
  }

  return Effect.promise(() => sha256Hex(idempotencyKey))
}

const decodeWorkRequest = (
  request: Request,
): Effect.Effect<OpenAgentsAutopilotWorkRequest, AutopilotWorkStoreError> =>
  Effect.tryPromise({
    catch: error =>
      new AutopilotWorkStoreError({
        kind: 'validation_error',
        reason: error instanceof Error ? error.message : String(error),
      }),
    try: async () =>
      decodeOpenAgentsAutopilotWorkRequest(await readJsonObject(request)),
  })

const routeNowIso = <Bindings>(
  dependencies: AutopilotWorkRoutesDependencies<Bindings>,
): string => dependencies.nowIso?.() ?? currentIsoTimestamp()

const routeMakeId = <Bindings>(
  dependencies: AutopilotWorkRoutesDependencies<Bindings>,
): string => (dependencies.makeId ?? randomUuid)()

const workOrderRefForId = (id: string): string =>
  id.startsWith('autopilot_work_order.')
    ? id
    : `autopilot_work_order.${id}`

const statusUrlRefForWorkOrder = (workOrderRef: string): string =>
  `status.${workOrderRef}`

const eventStreamRefForWorkOrder = (workOrderRef: string): string =>
  `events.${workOrderRef}`

const accessRequestRefsForRequest = (
  request: OpenAgentsAutopilotWorkRequest,
): ReadonlyArray<string> =>
  request.tasks.flatMap(task =>
    task.accessRequests.map(accessRequest =>
      `access_request.${task.taskRef}.${accessRequest.kind}`
    )
  )

const paymentChallengeRefForRequest = (
  request: OpenAgentsAutopilotWorkRequest,
): string | null =>
  request.paymentPolicy.buyerPaymentMode === 'l402' ||
  request.paymentPolicy.buyerPaymentMode === 'mdk_checkout' ||
  request.paymentPolicy.buyerPaymentMode === 'paid_quote_required'
    ? `challenge.autopilot_work.${request.clientRequestRef}`
    : null

const stateForRequest = (
  request: OpenAgentsAutopilotWorkRequest,
): OpenAgentsAutopilotWorkStateType => {
  if (request.tasks.some(task => task.accessRequests.length > 0)) {
    return 'access_required'
  }

  if (paymentChallengeRefForRequest(request) !== null) {
    return 'payment_required'
  }

  return 'accepted_free_slice'
}

const projectionForRecord = (
  record: AutopilotWorkOrderRecord,
  idempotent: boolean,
): AutopilotWorkOrderProjection => ({
  accessRequestRefs: record.accessRequestRefs,
  clientRequestRef: record.clientRequestRef,
  createdAt: record.createdAt,
  eventStreamRef: record.eventStreamRef,
  idempotent,
  paymentChallengeRef: record.paymentChallengeRef,
  state: record.state,
  statusUrlRef: record.statusUrlRef,
  taskRefs: record.taskRefs,
  updatedAt: record.updatedAt,
  workOrderRef: record.workOrderRef,
})

const terminalEventKindForState = (
  state: OpenAgentsAutopilotWorkStateType,
): AutopilotWorkEventKind | undefined => {
  switch (state) {
    case 'access_required':
      return 'needs_access'
    case 'blocked':
    case 'invalid':
      return 'blocked'
    case 'delivered':
      return 'delivered'
    case 'payment_required':
      return 'payment_required'
    case 'queued_or_running':
      return 'running'
    case 'accepted_free_slice':
      return undefined
  }
}

const eventForRecord = (
  record: AutopilotWorkOrderRecord,
  eventKind: AutopilotWorkEventKind,
  sequence: number,
  occurredAt: string,
): AutopilotWorkEventProjection => ({
  eventKind,
  eventRef: `event.${record.workOrderRef}.${sequence}`,
  occurredAt,
  publicSafe: true,
  sequence,
  state: record.state,
  taskRefs: record.taskRefs,
  workOrderRef: record.workOrderRef,
})

export const eventsForRecord = (
  record: AutopilotWorkOrderRecord,
): ReadonlyArray<AutopilotWorkEventProjection> => {
  const events = [
    eventForRecord(record, 'queued', 1, record.createdAt),
  ]
  const terminalKind = terminalEventKindForState(record.state)

  if (terminalKind !== undefined) {
    events.push(
      eventForRecord(record, terminalKind, events.length + 1, record.updatedAt),
    )
  }

  return events
}

const buildWorkOrderRecord = (
  input: Readonly<{
    agentCredentialId: string
    agentUserId: string
    id: string
    idempotencyKeyHash: string
    nowIso: string
    ownerUserId: string
    request: OpenAgentsAutopilotWorkRequest
  }>,
): AutopilotWorkOrderRecord => {
  const workOrderRef = workOrderRefForId(input.id)
  const paymentChallengeRef = paymentChallengeRefForRequest(input.request)

  return {
    accessRequestRefs: accessRequestRefsForRequest(input.request),
    agentCredentialId: input.agentCredentialId,
    agentUserId: input.agentUserId,
    archivedAt: null,
    clientRequestRef: input.request.clientRequestRef,
    createdAt: input.nowIso,
    eventStreamRef: eventStreamRefForWorkOrder(workOrderRef),
    id: input.id,
    idempotencyKeyHash: input.idempotencyKeyHash,
    ownerUserId: input.ownerUserId,
    paymentChallengeRef,
    request: input.request,
    state: stateForRequest(input.request),
    statusUrlRef: statusUrlRefForWorkOrder(workOrderRef),
    taskRefs: input.request.tasks.map(task => task.taskRef),
    updatedAt: input.nowIso,
    workOrderRef,
  }
}

const createWorkOrder = <Bindings extends AutopilotWorkRouteEnv>(
  dependencies: AutopilotWorkRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const nowIso = routeNowIso(dependencies)
    const auth = yield* authenticateCustomerOrderAgentRequest(
      request,
      dependencies.agentStore(env),
      {
        nowIso: () => nowIso,
        requiredScope: 'customer_orders.write',
      },
    )
    const idempotencyKeyHash = yield* requireIdempotencyHash(request)
    const existing = yield* Effect.promise(() =>
      dependencies
        .makeStore(env)
        .readWorkOrderByIdempotency(auth.ownerUserId, idempotencyKeyHash)
    )

    if (existing !== undefined) {
      return noStoreJsonResponse(
        { work: projectionForRecord(existing, true) },
        { status: 200 },
      )
    }

    const workRequest = yield* decodeWorkRequest(request)
    const record = buildWorkOrderRecord({
      agentCredentialId: auth.agent.credential.id,
      agentUserId: auth.agent.user.id,
      id: routeMakeId(dependencies),
      idempotencyKeyHash,
      nowIso,
      ownerUserId: auth.ownerUserId,
      request: workRequest,
    })
    const created = yield* Effect.tryPromise({
      catch: error =>
        new AutopilotWorkStoreError({
          kind: 'storage_error',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () => dependencies.makeStore(env).createWorkOrder(record),
    })

    return noStoreJsonResponse(
      { work: projectionForRecord(created.record, created.idempotent) },
      { status: created.idempotent ? 200 : 202 },
    )
  }).pipe(
    Effect.catchTag('CustomerOrderAgentAuthFailure', () =>
      Effect.succeed(unauthorized())
    ),
    Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
  )

const readWorkOrder = <Bindings extends AutopilotWorkRouteEnv>(
  dependencies: AutopilotWorkRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  workOrderRef: string,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const nowIso = routeNowIso(dependencies)
    const auth = yield* authenticateCustomerOrderAgentRequest(
      request,
      dependencies.agentStore(env),
      {
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
          error: 'autopilot_work_not_found',
          reason: 'Autopilot work order was not found.',
        },
        { status: 404 },
      )
    }

    return noStoreJsonResponse({
      work: projectionForRecord(record, false),
    })
  }).pipe(
    Effect.catchTag('CustomerOrderAgentAuthFailure', () =>
      Effect.succeed(unauthorized())
    ),
    Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
  )

const parseAfterCursor = (request: Request): number => {
  const url = new URL(request.url)
  const headerCursor = request.headers.get('Last-Event-ID')
  const queryCursor = url.searchParams.get('after')
  const rawCursor = headerCursor === null || headerCursor === ''
    ? queryCursor
    : headerCursor

  if (rawCursor === null || rawCursor === undefined || rawCursor === '') {
    return 0
  }

  const cursor = Number(rawCursor)

  return Number.isSafeInteger(cursor) && cursor >= 0 ? cursor : 0
}

const eventStreamPayload = (
  events: ReadonlyArray<AutopilotWorkEventProjection>,
): string => {
  const body = events
    .map(event =>
      [
        `id: ${event.sequence}`,
        `event: ${event.eventKind}`,
        `data: ${JSON.stringify({ event })}`,
        '',
      ].join('\n'),
    )
    .join('\n')

  return body === '' ? ': no events\n\n' : `${body}\n`
}

const eventStreamResponse = (
  events: ReadonlyArray<AutopilotWorkEventProjection>,
) =>
  new globalThis.Response(eventStreamPayload(events), {
    headers: {
      'cache-control': 'no-store',
      'content-type': 'text/event-stream; charset=utf-8',
      'x-accel-buffering': 'no',
    },
  })

const wantsEventStream = (request: Request): boolean => {
  const url = new URL(request.url)

  return (
    request.headers.get('accept')?.includes('text/event-stream') === true ||
    url.searchParams.get('stream') === 'sse'
  )
}

const readWorkOrderEvents = <Bindings extends AutopilotWorkRouteEnv>(
  dependencies: AutopilotWorkRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  workOrderRef: string,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const nowIso = routeNowIso(dependencies)
    const auth = yield* authenticateCustomerOrderAgentRequest(
      request,
      dependencies.agentStore(env),
      {
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
          error: 'autopilot_work_not_found',
          reason: 'Autopilot work order was not found.',
        },
        { status: 404 },
      )
    }

    const after = parseAfterCursor(request)
    const events = eventsForRecord(record).filter(
      event => event.sequence > after,
    )

    if (wantsEventStream(request)) {
      return eventStreamResponse(events)
    }

    return noStoreJsonResponse({
      events,
      nextAfter: events.length === 0
        ? after
        : events[events.length - 1]?.sequence ?? after,
      workOrderRef: record.workOrderRef,
    })
  }).pipe(
    Effect.catchTag('CustomerOrderAgentAuthFailure', () =>
      Effect.succeed(unauthorized())
    ),
    Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
  )

const workOrderRefFromPath = (pathname: string): string | undefined => {
  const match = /^\/api\/autopilot\/work\/([^/]+)$/.exec(pathname)

  return match?.[1]
}

const workOrderEventsRefFromPath = (pathname: string): string | undefined => {
  const match = /^\/api\/autopilot\/work\/([^/]+)\/events$/.exec(pathname)

  return match?.[1]
}

export const makeAutopilotWorkRoutes = <
  Bindings extends AutopilotWorkRouteEnv,
>(
  dependencies: AutopilotWorkRoutesDependencies<Bindings>,
) => ({
  routeAutopilotWorkRequest: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<Response> | undefined => {
    const url = new URL(request.url)

    if (url.pathname === '/api/autopilot/work') {
      return M.value(request.method).pipe(
        M.when('POST', () => createWorkOrder(dependencies, request, env)),
        M.orElse(() => Effect.succeed(methodNotAllowed(['POST']))),
      )
    }

    const workOrderEventsRef = workOrderEventsRefFromPath(url.pathname)

    if (workOrderEventsRef !== undefined) {
      return M.value(request.method).pipe(
        M.when('GET', () =>
          readWorkOrderEvents(
            dependencies,
            request,
            env,
            workOrderEventsRef,
          )
        ),
        M.orElse(() => Effect.succeed(methodNotAllowed(['GET']))),
      )
    }

    const workOrderRef = workOrderRefFromPath(url.pathname)

    if (workOrderRef !== undefined) {
      return M.value(request.method).pipe(
        M.when('GET', () =>
          readWorkOrder(dependencies, request, env, workOrderRef)
        ),
        M.orElse(() => Effect.succeed(methodNotAllowed(['GET']))),
      )
    }

    return undefined
  },
})

const parseJsonArray = (value: string): ReadonlyArray<string> => {
  const parsed = JSON.parse(value)

  return Array.isArray(parsed)
    ? parsed.filter(item => typeof item === 'string')
    : []
}

const recordFromRow = (
  row: Readonly<Record<string, unknown>>,
): AutopilotWorkOrderRecord => ({
  accessRequestRefs: parseJsonArray(String(row.access_request_refs_json)),
  agentCredentialId: String(row.agent_credential_id),
  agentUserId: String(row.agent_user_id),
  archivedAt:
    typeof row.archived_at === 'string' ? row.archived_at : null,
  clientRequestRef: String(row.client_request_ref),
  createdAt: String(row.created_at),
  eventStreamRef: String(row.event_stream_ref),
  id: String(row.id),
  idempotencyKeyHash: String(row.idempotency_key_hash),
  ownerUserId: String(row.owner_user_id),
  paymentChallengeRef:
    typeof row.payment_challenge_ref === 'string'
      ? row.payment_challenge_ref
      : null,
  request: decodeOpenAgentsAutopilotWorkRequest(
    JSON.parse(String(row.request_json)),
  ),
  state: S.decodeUnknownSync(OpenAgentsAutopilotWorkState)(row.state),
  statusUrlRef: String(row.status_url_ref),
  taskRefs: parseJsonArray(String(row.task_refs_json)),
  updatedAt: String(row.updated_at),
  workOrderRef: String(row.work_order_ref),
})

export const makeD1AutopilotWorkStore = (
  db: D1Database,
): AutopilotWorkStore => ({
  createWorkOrder: async record => {
    const existing = await db
      .prepare(
        `SELECT *
         FROM autopilot_work_orders
         WHERE owner_user_id = ?
           AND idempotency_key_hash = ?
           AND archived_at IS NULL
         LIMIT 1`,
      )
      .bind(record.ownerUserId, record.idempotencyKeyHash)
      .first<Record<string, unknown>>()

    if (existing !== null) {
      return { idempotent: true, record: recordFromRow(existing) }
    }

    await db
      .prepare(
        `INSERT INTO autopilot_work_orders (
          id,
          work_order_ref,
          owner_user_id,
          agent_user_id,
          agent_credential_id,
          idempotency_key_hash,
          client_request_ref,
          request_json,
          state,
          task_refs_json,
          access_request_refs_json,
          payment_challenge_ref,
          status_url_ref,
          event_stream_ref,
          created_at,
          updated_at,
          archived_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
      )
      .bind(
        record.id,
        record.workOrderRef,
        record.ownerUserId,
        record.agentUserId,
        record.agentCredentialId,
        record.idempotencyKeyHash,
        record.clientRequestRef,
        JSON.stringify(record.request),
        record.state,
        JSON.stringify(record.taskRefs),
        JSON.stringify(record.accessRequestRefs),
        record.paymentChallengeRef,
        record.statusUrlRef,
        record.eventStreamRef,
        record.createdAt,
        record.updatedAt,
      )
      .run()

    return { idempotent: false, record }
  },
  readWorkOrder: async workOrderRef => {
    const row = await db
      .prepare(
        `SELECT *
         FROM autopilot_work_orders
         WHERE work_order_ref = ?
           AND archived_at IS NULL
         LIMIT 1`,
      )
      .bind(workOrderRef)
      .first<Record<string, unknown>>()

    return row === null ? undefined : recordFromRow(row)
  },
  readWorkOrderByIdempotency: async (ownerUserId, idempotencyKeyHash) => {
    const row = await db
      .prepare(
        `SELECT *
         FROM autopilot_work_orders
         WHERE owner_user_id = ?
           AND idempotency_key_hash = ?
           AND archived_at IS NULL
         LIMIT 1`,
      )
      .bind(ownerUserId, idempotencyKeyHash)
      .first<Record<string, unknown>>()

    return row === null ? undefined : recordFromRow(row)
  },
})
