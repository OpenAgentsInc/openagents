import { jsonResponse } from '@openagentsinc/sync-worker'
import { Schema as S } from 'effect'

import {
  forbidden,
  methodNotAllowed,
  noStoreJsonResponse,
  unauthorized,
} from './http/responses'
import {
  decodeUnknownWithSchema,
  optionalString,
  parseJsonWithSchema,
  readJsonObject,
} from './json-boundary'
import { openAgentsDatabase } from './runtime'
import { currentIsoTimestamp, isoTimestampToDate, normalizeIsoTimestamp } from './runtime-primitives'

export const OPERATOR_PRO_STATUS_PATH = '/api/operator/pro/status'
export const PYLON_AGENT_RUNNER_STATUS_EVENT_SCHEMA_VERSION =
  'openagents.pylon.agent_runner_status_event.v1'

type OperatorProStatusEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>

type OperatorProSession = Readonly<{
  user: Readonly<{
    email: string
    userId: string
  }>
}>

type HttpResponse = Response

type OperatorProStatusDependencies<
  Session extends OperatorProSession,
  Bindings extends OperatorProStatusEnv,
> = Readonly<{
  appendRefreshedSessionCookies?: (
    response: HttpResponse,
    session: Session,
  ) => HttpResponse
  authenticateAgentToken?: (
    request: Request,
    env: Bindings,
  ) => Promise<{ userId: string } | undefined>
  currentIsoTimestamp?: () => string
  isOpenAgentsAdminEmail?: (email: string) => boolean
  listLinkedAgentsForOpenAuthUser?: (
    openauthUserId: string,
    limit: number,
    env: Bindings,
  ) => Promise<ReadonlyArray<{ agentUserId: string; openauthUserId?: string | null }>>
  requireAdminApiToken?: (request: Request, env: Bindings) => Promise<boolean>
  requireBrowserSession?: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
}>

type ProAgentState = 'working' | 'blocked' | 'waiting' | 'done'

const AgentRunnerNeutralStateSchema = S.Literals([
  'idle',
  'queued',
  'working',
  'waiting',
  'blocked',
  'done',
  'failed',
  'offline',
])

const AgentRunnerControlVerb = S.Literals([
  'status.list',
  'task.list',
  'task.update',
  'task.dispatch',
  'dispatch.cancel',
])

const AgentRunnerStatusHistoryEntry = S.Struct({
  state: AgentRunnerNeutralStateSchema,
  stateStartedAt: S.String,
})

const AgentRunnerStatusEvent = S.Struct({
  schemaVersion: S.Literal(PYLON_AGENT_RUNNER_STATUS_EVENT_SCHEMA_VERSION),
  eventRef: S.String,
  runnerRef: S.String,
  runnerKind: S.String,
  state: AgentRunnerNeutralStateSchema,
  stateStartedAt: S.String,
  updatedAt: S.String,
  assignmentRef: S.optionalKey(S.String),
  taskId: S.optionalKey(S.String),
  dispatchContextId: S.optionalKey(S.String),
  assigneeHandle: S.optionalKey(S.String),
  pylonRef: S.optionalKey(S.String),
  worktreeId: S.optionalKey(S.String),
  worktreeRef: S.optionalKey(S.String),
  capabilityRefs: S.optionalKey(S.Array(S.String)),
  supportedControlVerbs: S.optionalKey(S.Array(AgentRunnerControlVerb)),
  refs: S.optionalKey(S.Array(S.String)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  stateHistory: S.optionalKey(S.Array(AgentRunnerStatusHistoryEntry)),
})
type AgentRunnerStatusEvent = typeof AgentRunnerStatusEvent.Type

type StatusRow = Readonly<{
  event_ref: string
  owner_agent_user_id: string
  runner_ref: string
  runner_kind: string
  pylon_ref: string | null
  assignment_ref: string | null
  state: string
  state_started_at: string
  updated_at: string
  retention_state: 'live' | 'retained'
  event_json: string
}>

type PylonOwnerRow = Readonly<{
  owner_agent_user_id: string
}>

type ReadScope<Session extends OperatorProSession> =
  | Readonly<{ kind: 'admin' }>
  | Readonly<{ kind: 'browser'; ownerAgentUserIds: ReadonlyArray<string>; session: Session }>
  | Readonly<{ kind: 'agent'; userId: string }>

const unsafeProjectionValue =
  /(\/Users\/|[A-Za-z]:\\|raw[_-]?(prompt|trace|payload|event|log)|secret|token|wallet|credential|auth\.json)/i

const publicRefPattern =
  /^(agent|assignment|assignment-event|assignment-status|blocker|capability|capacity|command|dispatch|dispatch-context|dispatch-context-status|event|issue|load|merge-action|merge-wave|pylon|runner|runner-kind|task|task-status|worktree|ref|status)\.[A-Za-z0-9_.:=\-]+$/

const bounded = (value: string | undefined, fallback: string, max: number): string => {
  const trimmed = value?.trim()
  const safe = trimmed === undefined || trimmed === '' ? fallback : trimmed
  return safe.slice(0, max)
}

const isSafeText = (value: string): boolean => !unsafeProjectionValue.test(value)

const safeRefArray = (
  values: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> =>
  (values ?? [])
    .filter(value => publicRefPattern.test(value) && isSafeText(value))
    .slice(0, 32)

const validatePublicSafeEvent = (event: AgentRunnerStatusEvent): string | null => {
  const refs = [
    event.eventRef,
    event.runnerRef,
    event.assignmentRef,
    event.taskId,
    event.dispatchContextId,
    event.pylonRef,
    event.worktreeRef,
    ...(event.refs ?? []),
    ...(event.blockerRefs ?? []),
    ...(event.capabilityRefs ?? []),
  ].filter((value): value is string => value !== undefined)

  const unsafeRef = refs.find(value => !publicRefPattern.test(value) || !isSafeText(value))
  if (unsafeRef !== undefined) {
    return `unsafe public ref: ${unsafeRef}`
  }

  const textFields = [
    event.runnerKind,
    event.assigneeHandle,
    event.worktreeId,
  ].filter((value): value is string => value !== undefined)

  const unsafeText = textFields.find(value => !isSafeText(value))
  return unsafeText === undefined ? null : 'unsafe public projection text'
}

const isoTimestampPattern =
  /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/

const isIsoTimestamp = (value: string): boolean => {
  if (!isoTimestampPattern.test(value)) {
    return false
  }
  const time = isoTimestampToDate(value).getTime()
  return Number.isFinite(time) && normalizeIsoTimestamp(value) === value
}

const validateIsoTimestamps = (event: AgentRunnerStatusEvent): string | null => {
  const fields: ReadonlyArray<readonly [string, string]> = [
    ['stateStartedAt', event.stateStartedAt],
    ['updatedAt', event.updatedAt],
    ...(event.stateHistory ?? []).map((entry, index): readonly [string, string] => [
      `stateHistory[${index}].stateStartedAt`,
      entry.stateStartedAt,
    ]),
  ]
  const invalid = fields.find(([, value]) => !isIsoTimestamp(value))
  return invalid === undefined ? null : `invalid ISO timestamp: ${invalid[0]}`
}

const proStateFromRunnerState = (state: string): ProAgentState =>
  state === 'working'
    ? 'working'
    : state === 'done'
      ? 'done'
      : state === 'blocked' || state === 'failed' || state === 'offline'
        ? 'blocked'
        : 'waiting'

const terminalState = (state: string): boolean =>
  state === 'done' || state === 'failed' || state === 'offline'

const readRows = async <T>(
  db: D1Database,
  sql: string,
  ...bindings: ReadonlyArray<unknown>
): Promise<ReadonlyArray<T>> => {
  const statement = bindings.length === 0
    ? db.prepare(sql)
    : db.prepare(sql).bind(...bindings)
  const result = await statement.all<T>()
  return result.results ?? []
}

const readFirst = async <T>(
  db: D1Database,
  sql: string,
  ...bindings: ReadonlyArray<unknown>
): Promise<T | null> => {
  const statement = bindings.length === 0
    ? db.prepare(sql)
    : db.prepare(sql).bind(...bindings)
  return await statement.first<T>()
}

const runSql = async (
  db: D1Database,
  sql: string,
  ...bindings: ReadonlyArray<unknown>
): Promise<void> => {
  const statement = bindings.length === 0
    ? db.prepare(sql)
    : db.prepare(sql).bind(...bindings)
  await statement.run()
}

const eventFromRow = (row: StatusRow): AgentRunnerStatusEvent | null => {
  try {
    return parseJsonWithSchema(AgentRunnerStatusEvent, row.event_json)
  } catch {
    return null
  }
}

const statusLabel = (state: string): string =>
  state === 'working'
    ? 'Working'
    : state === 'done'
      ? 'Done'
      : state === 'blocked'
        ? 'Blocked'
        : state === 'failed'
          ? 'Failed'
          : state === 'offline'
            ? 'Offline'
            : 'Waiting'

const rowToProEntry = (row: StatusRow) => {
  const event = eventFromRow(row)
  const state = proStateFromRunnerState(row.state)
  const stateHistory = (event?.stateHistory ?? [])
    .slice(-20)
    .map(entry => ({
      at: entry.stateStartedAt,
      label: statusLabel(entry.state),
      state: proStateFromRunnerState(entry.state),
    }))

  return {
    acknowledgedAt: row.retention_state === 'retained'
      ? row.updated_at
      : row.state_started_at,
    agentLabel: bounded(event?.assigneeHandle, row.runner_kind, 80),
    id: row.runner_ref,
    lastAssistantMessage: bounded(
      event?.blockerRefs?.[0] ?? event?.refs?.[0],
      `${statusLabel(row.state)} from runner status spine.`,
      180,
    ),
    prompt: bounded(
      event?.taskId ?? event?.assignmentRef ?? event?.dispatchContextId,
      'Runner status projection',
      180,
    ),
    state,
    stateHistory,
    stateStartedAt: row.state_started_at,
    toolName: bounded(event?.supportedControlVerbs?.[0], row.runner_kind, 80),
    unread: row.retention_state === 'live' && row.state !== 'idle',
    updatedAt: row.updated_at,
    worktreeLabel: bounded(event?.worktreeId ?? event?.worktreeRef, 'owner-scoped runner', 80),
  }
}

const listRows = async (
  db: D1Database,
  scope: ReadScope<OperatorProSession>,
  retentionState: 'live' | 'retained',
): Promise<ReadonlyArray<StatusRow>> => {
  if (scope.kind === 'browser' && scope.ownerAgentUserIds.length === 0) {
    return []
  }
  const ownerAgentUserIds =
    scope.kind === 'admin'
      ? []
      : scope.kind === 'agent'
        ? [scope.userId]
        : scope.ownerAgentUserIds
  const ownerClause = scope.kind === 'admin'
    ? ''
    : `AND owner_agent_user_id IN (${ownerAgentUserIds.map(() => '?').join(', ')})`
  return readRows<StatusRow>(
    db,
    `SELECT event_ref, owner_agent_user_id, runner_ref, runner_kind, pylon_ref,
            assignment_ref, state, state_started_at, updated_at,
            retention_state, event_json
       FROM pylon_agent_runner_status_events
      WHERE archived_at IS NULL
        AND retention_state = ?
        ${ownerClause}
      ORDER BY updated_at DESC
      LIMIT 100`,
    retentionState,
    ...ownerAgentUserIds,
  )
}

const resolveBrowserOwnerAgentUserIds = async <
  Session extends OperatorProSession,
  Bindings extends OperatorProStatusEnv,
>(
  dependencies: OperatorProStatusDependencies<Session, Bindings>,
  env: Bindings,
  session: Session,
): Promise<ReadonlyArray<string>> => {
  const linkedAgents = await dependencies.listLinkedAgentsForOpenAuthUser?.(
    session.user.userId,
    100,
    env,
  )
  if (linkedAgents === undefined) {
    return []
  }
  return [
    ...new Set(
      linkedAgents
        .filter(agent =>
          agent.openauthUserId === undefined ||
          agent.openauthUserId === null ||
          agent.openauthUserId === session.user.userId,
        )
        .map(agent => agent.agentUserId)
        .filter(agentUserId => agentUserId.trim() !== ''),
    ),
  ]
}

const buildSnapshot = async (
  db: D1Database,
  scope: ReadScope<OperatorProSession>,
  generatedAt: string,
) => {
  const [liveRows, retainedRows] = await Promise.all([
    listRows(db, scope, 'live'),
    listRows(db, scope, 'retained'),
  ])

  return {
    generatedAt,
    liveEntries: liveRows.map(rowToProEntry),
    retainedEntries: retainedRows.map(rowToProEntry),
    diffComments: [],
  }
}

const authorizeRead = async <
  Session extends OperatorProSession,
  Bindings extends OperatorProStatusEnv,
>(
  dependencies: OperatorProStatusDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Promise<ReadScope<Session> | HttpResponse> => {
  if (await dependencies.requireAdminApiToken?.(request, env)) {
    return { kind: 'admin' }
  }

  const agent = await dependencies.authenticateAgentToken?.(request, env)
  if (agent !== undefined) {
    return { kind: 'agent', userId: agent.userId }
  }

  const session = await dependencies.requireBrowserSession?.(request, env, ctx)
  if (session === undefined) {
    return unauthorized()
  }

  if (
    dependencies.isOpenAgentsAdminEmail !== undefined &&
    !dependencies.isOpenAgentsAdminEmail(session.user.email)
  ) {
    return forbidden()
  }

  return {
    kind: 'browser',
    ownerAgentUserIds: await resolveBrowserOwnerAgentUserIds(dependencies, env, session),
    session,
  }
}

const ownerForPylon = async (
  db: D1Database,
  pylonRef: string | undefined,
): Promise<string | null> => {
  if (pylonRef === undefined) {
    return null
  }

  const row = await readFirst<PylonOwnerRow>(
    db,
    `SELECT owner_agent_user_id
       FROM pylon_api_registrations
      WHERE pylon_ref = ?
        AND archived_at IS NULL
      LIMIT 1`,
    pylonRef,
  )
  return row?.owner_agent_user_id ?? null
}

const ingestEvent = async (
  db: D1Database,
  event: AgentRunnerStatusEvent,
  ownerUserId: string,
  nowIso: string,
): Promise<void> => {
  const retentionState = terminalState(event.state) ? 'retained' : 'live'

  await runSql(
    db,
    `UPDATE pylon_agent_runner_status_events
        SET retention_state = 'retained',
            retained_at = COALESCE(retained_at, ?)
      WHERE owner_agent_user_id = ?
        AND runner_ref = ?
        AND event_ref <> ?
        AND retention_state = 'live'
        AND archived_at IS NULL`,
    event.stateStartedAt,
    ownerUserId,
    event.runnerRef,
    event.eventRef,
  )

  await runSql(
    db,
    `INSERT INTO pylon_agent_runner_status_events (
       event_ref, owner_agent_user_id, runner_ref, runner_kind, pylon_ref,
       assignment_ref, state, state_started_at, updated_at, retention_state,
       event_json, created_at, retained_at, archived_at
     ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)
     ON CONFLICT(event_ref) DO UPDATE SET
       runner_kind = excluded.runner_kind,
       pylon_ref = excluded.pylon_ref,
       assignment_ref = excluded.assignment_ref,
       state = excluded.state,
       state_started_at = excluded.state_started_at,
       updated_at = excluded.updated_at,
       retention_state = excluded.retention_state,
       event_json = excluded.event_json,
       retained_at = excluded.retained_at
     WHERE owner_agent_user_id = ?`,
    event.eventRef,
    ownerUserId,
    event.runnerRef,
    bounded(event.runnerKind, 'runner', 80),
    event.pylonRef ?? null,
    event.assignmentRef ?? null,
    event.state,
    event.stateStartedAt,
    event.updatedAt,
    retentionState,
    JSON.stringify({
      ...event,
      capabilityRefs: safeRefArray(event.capabilityRefs),
      refs: safeRefArray(event.refs),
      blockerRefs: safeRefArray(event.blockerRefs),
      stateHistory: (event.stateHistory ?? []).slice(-20),
    }),
    nowIso,
    retentionState === 'retained' ? event.updatedAt : null,
    ownerUserId,
  )
}

export const makeOperatorProStatusRoutes = <
  Session extends OperatorProSession,
  Bindings extends OperatorProStatusEnv,
>(
  dependencies: OperatorProStatusDependencies<Session, Bindings>,
) => ({
  handleOperatorProStatusApi: async (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => {
    const db = openAgentsDatabase(env)

    if (request.method === 'GET') {
      const scope = await authorizeRead(dependencies, request, env, ctx)
      if (scope instanceof Response) {
        return scope
      }

      const snapshot = await buildSnapshot(
        db,
        scope,
        dependencies.currentIsoTimestamp?.() ?? currentIsoTimestamp(),
      )
      const response = noStoreJsonResponse(snapshot)

      return scope.kind === 'browser' && dependencies.appendRefreshedSessionCookies !== undefined
        ? dependencies.appendRefreshedSessionCookies(response, scope.session)
        : response
    }

    if (request.method !== 'POST') {
      return methodNotAllowed(['GET', 'POST'])
    }

    const agent = await dependencies.authenticateAgentToken?.(request, env)
    if (agent === undefined) {
      return unauthorized()
    }

    let event: AgentRunnerStatusEvent
    try {
      event = decodeUnknownWithSchema(AgentRunnerStatusEvent, await readJsonObject(request))
    } catch {
      return jsonResponse(
        {
          error: 'invalid_agent_runner_status_event',
          schemaVersion: PYLON_AGENT_RUNNER_STATUS_EVENT_SCHEMA_VERSION,
        },
        { status: 400 },
      )
    }

    const safetyError = validatePublicSafeEvent(event)
    if (safetyError !== null) {
      return jsonResponse(
        { error: 'unsafe_agent_runner_status_projection', message: safetyError },
        { status: 400 },
      )
    }

    const timestampError = validateIsoTimestamps(event)
    if (timestampError !== null) {
      return jsonResponse(
        { error: 'invalid_agent_runner_status_event', message: timestampError },
        { status: 400 },
      )
    }

    const pylonOwner = await ownerForPylon(db, optionalString(event.pylonRef))
    if (pylonOwner !== null && pylonOwner !== agent.userId) {
      return forbidden()
    }

    await ingestEvent(
      db,
      event,
      pylonOwner ?? agent.userId,
      dependencies.currentIsoTimestamp?.() ?? currentIsoTimestamp(),
    )

    return noStoreJsonResponse({
      ok: true,
      eventRef: event.eventRef,
      retentionState: terminalState(event.state) ? 'retained' : 'live',
    })
  },
})
