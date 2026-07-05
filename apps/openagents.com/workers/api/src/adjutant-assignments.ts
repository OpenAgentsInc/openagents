import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Layer, Schema as S } from 'effect'
import * as Context from 'effect/Context'

import {
  type AgentGoalError,
  type AgentGoalRepositoryShape,
  makeD1AgentGoalRepository,
} from './agent-goals'
import {
  makeAgentGoalRepositoryForEnv,
  type AgentRuntimeStoreEnv,
} from './agent-runtime-store'
import { openAgentsDatabase } from './runtime'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'
import {
  makeSupervisionLongtailMirrorForEnv,
  type SupervisionLongtailMirror,
} from './supervision-longtail-domain-store'

type AdjutantAssignmentEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}> &
  AgentRuntimeStoreEnv

export type AdjutantAssignmentRuntime = Readonly<{
  makeAssignmentId: () => string
  makeEventId: () => string
  nowIso: () => string
}>

export const systemAdjutantAssignmentRuntime: AdjutantAssignmentRuntime = {
  makeAssignmentId: () => compactRandomId('adjutant_assignment'),
  makeEventId: () => compactRandomId('adjutant_assignment_event'),
  nowIso: currentIsoTimestamp,
}

const ADJUTANT_ASSIGNMENT_EVENT_PAYLOAD_LIMIT_BYTES = 4096

export const AdjutantAssignmentKind = S.Literals([
  'site_generation',
  'site_adjustment',
  'site_review',
  'site_deployment',
  'general_order_fulfillment',
])
export type AdjutantAssignmentKind = typeof AdjutantAssignmentKind.Type

export const AdjutantAssignmentStatus = S.Literals([
  'draft',
  'preflight_pending',
  'blocked',
  'queued',
  'running',
  'review_needed',
  'deployed',
  'delivered',
  'complete',
  'canceled',
])
export type AdjutantAssignmentStatus = typeof AdjutantAssignmentStatus.Type

export const AdjutantAssignmentVisibility = S.Literals([
  'private',
  'team',
  'public',
])
export type AdjutantAssignmentVisibility =
  typeof AdjutantAssignmentVisibility.Type

export const AdjutantAssignment = S.Struct({
  id: S.String,
  softwareOrderId: S.NullOr(S.String),
  siteId: S.NullOr(S.String),
  goalId: S.NullOr(S.String),
  currentRunId: S.NullOr(S.String),
  teamId: S.NullOr(S.String),
  projectId: S.NullOr(S.String),
  agentId: S.String,
  assignedByUserId: S.NullOr(S.String),
  assignmentKind: AdjutantAssignmentKind,
  status: AdjutantAssignmentStatus,
  visibility: AdjutantAssignmentVisibility,
  taskSpecPath: S.NullOr(S.String),
  commitSha: S.NullOr(S.String),
  objective: S.String,
  createdAt: S.String,
  updatedAt: S.String,
  completedAt: S.NullOr(S.String),
  blockedAt: S.NullOr(S.String),
  archivedAt: S.NullOr(S.String),
})
export type AdjutantAssignment = typeof AdjutantAssignment.Type

export const CreateAdjutantAssignmentInput = S.Struct({
  assignmentKind: AdjutantAssignmentKind,
  objective: S.String,
  agentId: S.optionalKey(S.String),
  assignedByUserId: S.optionalKey(S.String),
  commitSha: S.optionalKey(S.String),
  currentRunId: S.optionalKey(S.String),
  goalId: S.optionalKey(S.String),
  projectId: S.optionalKey(S.String),
  siteId: S.optionalKey(S.String),
  softwareOrderId: S.optionalKey(S.String),
  status: S.optionalKey(AdjutantAssignmentStatus),
  taskSpecPath: S.optionalKey(S.String),
  teamId: S.optionalKey(S.String),
  visibility: S.optionalKey(AdjutantAssignmentVisibility),
})
export type CreateAdjutantAssignmentInput =
  typeof CreateAdjutantAssignmentInput.Type

export const UpdateAdjutantAssignmentInput = S.Struct({
  assignmentId: S.String,
  commitSha: S.optionalKey(S.NullOr(S.String)),
  currentRunId: S.optionalKey(S.NullOr(S.String)),
  goalId: S.optionalKey(S.NullOr(S.String)),
  objective: S.optionalKey(S.String),
  status: S.optionalKey(AdjutantAssignmentStatus),
  taskSpecPath: S.optionalKey(S.NullOr(S.String)),
})
export type UpdateAdjutantAssignmentInput =
  typeof UpdateAdjutantAssignmentInput.Type

export const RecordAdjutantAssignmentEventInput = S.Struct({
  assignmentId: S.String,
  eventType: S.String,
  summary: S.String,
  actorUserId: S.optionalKey(S.NullOr(S.String)),
  payload: S.optionalKey(S.Unknown),
  runId: S.optionalKey(S.NullOr(S.String)),
})
export type RecordAdjutantAssignmentEventInput =
  typeof RecordAdjutantAssignmentEventInput.Type

type AdjutantAssignmentRow = Readonly<{
  agent_id: string
  archived_at: string | null
  assigned_by_user_id: string | null
  assignment_kind: AdjutantAssignmentKind
  blocked_at: string | null
  commit_sha: string | null
  completed_at: string | null
  created_at: string
  current_run_id: string | null
  goal_id: string | null
  id: string
  objective: string
  project_id: string | null
  site_id: string | null
  software_order_id: string | null
  status: AdjutantAssignmentStatus
  task_spec_path: string | null
  team_id: string | null
  updated_at: string
  visibility: AdjutantAssignmentVisibility
}>

type SiteAssignmentSourceRow = Readonly<{
  id: string
  software_order_id: string | null
}>

export class AdjutantAssignmentActiveExists extends S.TaggedErrorClass<AdjutantAssignmentActiveExists>()(
  'AdjutantAssignmentActiveExists',
  {
    assignmentId: S.String,
    siteId: S.NullOr(S.String),
    softwareOrderId: S.NullOr(S.String),
  },
) {}

export class AdjutantAssignmentNotFound extends S.TaggedErrorClass<AdjutantAssignmentNotFound>()(
  'AdjutantAssignmentNotFound',
  {
    assignmentId: S.String,
  },
) {}

export class AdjutantAssignmentGoalNotFound extends S.TaggedErrorClass<AdjutantAssignmentGoalNotFound>()(
  'AdjutantAssignmentGoalNotFound',
  {
    goalId: S.String,
  },
) {}

export class AdjutantAssignmentGoalStorageError extends S.TaggedErrorClass<AdjutantAssignmentGoalStorageError>()(
  'AdjutantAssignmentGoalStorageError',
  {
    operation: S.String,
    error: S.Defect,
  },
) {}

export class AdjutantAssignmentRunGoalRequired extends S.TaggedErrorClass<AdjutantAssignmentRunGoalRequired>()(
  'AdjutantAssignmentRunGoalRequired',
  {
    assignmentId: S.String,
    currentRunId: S.String,
    reason: S.String,
  },
) {}

export class AdjutantAssignmentSiteNotFound extends S.TaggedErrorClass<AdjutantAssignmentSiteNotFound>()(
  'AdjutantAssignmentSiteNotFound',
  {
    siteId: S.String,
  },
) {}

export class AdjutantAssignmentSoftwareOrderNotFound extends S.TaggedErrorClass<AdjutantAssignmentSoftwareOrderNotFound>()(
  'AdjutantAssignmentSoftwareOrderNotFound',
  {
    softwareOrderId: S.String,
  },
) {}

export class AdjutantAssignmentStorageError extends S.TaggedErrorClass<AdjutantAssignmentStorageError>()(
  'AdjutantAssignmentStorageError',
  {
    operation: S.String,
    error: S.Defect,
  },
) {}

export class AdjutantAssignmentUnsafePayload extends S.TaggedErrorClass<AdjutantAssignmentUnsafePayload>()(
  'AdjutantAssignmentUnsafePayload',
  {
    reason: S.String,
  },
) {}

export class AdjutantAssignmentValidationError extends S.TaggedErrorClass<AdjutantAssignmentValidationError>()(
  'AdjutantAssignmentValidationError',
  {
    reason: S.String,
  },
) {}

export type AdjutantAssignmentError =
  | AdjutantAssignmentActiveExists
  | AdjutantAssignmentGoalNotFound
  | AdjutantAssignmentGoalStorageError
  | AdjutantAssignmentNotFound
  | AdjutantAssignmentRunGoalRequired
  | AdjutantAssignmentSiteNotFound
  | AdjutantAssignmentSoftwareOrderNotFound
  | AdjutantAssignmentStorageError
  | AdjutantAssignmentUnsafePayload
  | AdjutantAssignmentValidationError

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, AdjutantAssignmentStorageError> =>
  Effect.tryPromise({
    try: run,
    catch: error => new AdjutantAssignmentStorageError({ operation, error }),
  })

const mapGoalError =
  (operation: string) =>
  (error: AgentGoalError): AdjutantAssignmentError => {
    switch (error._tag) {
      case 'AgentGoalNotFound':
        return new AdjutantAssignmentGoalNotFound({ goalId: error.goalId })
      case 'AgentGoalValidationError':
        return new AdjutantAssignmentValidationError({
          reason: `${error.field}: ${error.message}`,
        })
      default:
        return new AdjutantAssignmentGoalStorageError({ operation, error })
    }
  }

const assignmentFromRow = (row: AdjutantAssignmentRow): AdjutantAssignment => ({
  id: row.id,
  softwareOrderId: row.software_order_id,
  siteId: row.site_id,
  goalId: row.goal_id,
  currentRunId: row.current_run_id,
  teamId: row.team_id,
  projectId: row.project_id,
  agentId: row.agent_id,
  assignedByUserId: row.assigned_by_user_id,
  assignmentKind: row.assignment_kind,
  status: row.status,
  visibility: row.visibility,
  taskSpecPath: row.task_spec_path,
  commitSha: row.commit_sha,
  objective: row.objective,
  createdAt: row.created_at,
  updatedAt: row.updated_at,
  completedAt: row.completed_at,
  blockedAt: row.blocked_at,
  archivedAt: row.archived_at,
})

const nonEmptyOptional = (value: string | undefined): string | null => {
  const text = value?.trim()

  return text === undefined || text === '' ? null : text
}

const nullableInput = (value: string | null | undefined): string | null =>
  typeof value === 'string' ? nonEmptyOptional(value) : null

const assertPayloadSafe = (
  value: unknown,
): Effect.Effect<void, AdjutantAssignmentUnsafePayload> =>
  containsProviderSecretMaterial(JSON.stringify(value))
    ? Effect.fail(
        new AdjutantAssignmentUnsafePayload({
          reason:
            'Autopilot assignment payload contains secret-shaped material.',
        }),
      )
    : Effect.void

const eventPayloadJson = (
  payload: unknown | undefined,
): Effect.Effect<string | null, AdjutantAssignmentUnsafePayload> =>
  Effect.gen(function* () {
    if (payload === undefined) {
      return null
    }

    const json = yield* Effect.try({
      catch: error =>
        new AdjutantAssignmentUnsafePayload({
          reason:
            error instanceof Error ? error.message : 'invalid event payload',
        }),
      try: () => JSON.stringify(payload),
    })

    if (json.length > ADJUTANT_ASSIGNMENT_EVENT_PAYLOAD_LIMIT_BYTES) {
      return yield* new AdjutantAssignmentUnsafePayload({
        reason: 'Autopilot assignment event payload is too large.',
      })
    }

    if (containsProviderSecretMaterial(json)) {
      return yield* new AdjutantAssignmentUnsafePayload({
        reason:
          'Autopilot assignment event payload contains secret-shaped material.',
      })
    }

    return json
  })

const readSoftwareOrderExists = (
  db: D1Database,
  softwareOrderId: string,
): Effect.Effect<boolean, AdjutantAssignmentStorageError> =>
  d1Effect('adjutantAssignments.softwareOrder.exists', () =>
    db
      .prepare(
        `SELECT id
           FROM software_orders
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(softwareOrderId)
      .first<Readonly<{ id: string }>>(),
  ).pipe(Effect.map(row => row !== null))

const readSiteAssignmentSource = (
  db: D1Database,
  siteId: string,
): Effect.Effect<
  SiteAssignmentSourceRow | null,
  AdjutantAssignmentStorageError
> =>
  d1Effect('adjutantAssignments.site.read', () =>
    db
      .prepare(
        `SELECT id, software_order_id
           FROM site_projects
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(siteId)
      .first<SiteAssignmentSourceRow>(),
  )

const readAssignmentById = (
  db: D1Database,
  assignmentId: string,
): Effect.Effect<AdjutantAssignment | null, AdjutantAssignmentStorageError> =>
  d1Effect('adjutantAssignments.readById', () =>
    db
      .prepare(
        `SELECT id,
                software_order_id,
                site_id,
                goal_id,
                current_run_id,
                team_id,
                project_id,
                agent_id,
                assigned_by_user_id,
                assignment_kind,
                status,
                visibility,
                task_spec_path,
                commit_sha,
                objective,
                created_at,
                updated_at,
                completed_at,
                blocked_at,
                archived_at
           FROM adjutant_assignments
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(assignmentId)
      .first<AdjutantAssignmentRow>(),
  ).pipe(Effect.map(row => (row === null ? null : assignmentFromRow(row))))

const readActiveAssignment = (
  db: D1Database,
  input: Readonly<{ siteId: string | null; softwareOrderId: string | null }>,
): Effect.Effect<AdjutantAssignment | null, AdjutantAssignmentStorageError> =>
  d1Effect('adjutantAssignments.active.read', () =>
    db
      .prepare(
        `SELECT id,
                software_order_id,
                site_id,
                goal_id,
                current_run_id,
                team_id,
                project_id,
                agent_id,
                assigned_by_user_id,
                assignment_kind,
                status,
                visibility,
                task_spec_path,
                commit_sha,
                objective,
                created_at,
                updated_at,
                completed_at,
                blocked_at,
                archived_at
           FROM adjutant_assignments
          WHERE archived_at IS NULL
            AND status NOT IN ('complete', 'canceled')
            AND (
              (software_order_id IS NOT NULL AND software_order_id = ?)
              OR
              (site_id IS NOT NULL AND site_id = ?)
            )
          ORDER BY updated_at DESC
          LIMIT 1`,
      )
      .bind(input.softwareOrderId, input.siteId)
      .first<AdjutantAssignmentRow>(),
  ).pipe(Effect.map(row => (row === null ? null : assignmentFromRow(row))))

const listAssignments = (
  db: D1Database,
  limit: number,
): Effect.Effect<
  ReadonlyArray<AdjutantAssignment>,
  AdjutantAssignmentStorageError
> =>
  d1Effect('adjutantAssignments.list', () =>
    db
      .prepare(
        `SELECT id,
                software_order_id,
                site_id,
                goal_id,
                current_run_id,
                team_id,
                project_id,
                agent_id,
                assigned_by_user_id,
                assignment_kind,
                status,
                visibility,
                task_spec_path,
                commit_sha,
                objective,
                created_at,
                updated_at,
                completed_at,
                blocked_at,
                archived_at
           FROM adjutant_assignments
          WHERE archived_at IS NULL
          ORDER BY updated_at DESC
          LIMIT ?`,
      )
      .bind(Math.max(1, Math.min(100, Math.trunc(limit))))
      .all<AdjutantAssignmentRow>(),
  ).pipe(
    Effect.map(result => result.results.map(row => assignmentFromRow(row))),
  )

const recordAssignmentEvent = (
  db: D1Database,
  runtime: AdjutantAssignmentRuntime,
  assignment: AdjutantAssignment,
  input: Readonly<{
    actorUserId?: string | null | undefined
    eventType: string
    payload?: unknown | undefined
    runId?: string | null | undefined
    summary: string
  }>,
  mirror?: SupervisionLongtailMirror | undefined,
): Effect.Effect<
  void,
  AdjutantAssignmentStorageError | AdjutantAssignmentUnsafePayload
> =>
  Effect.gen(function* () {
    const payloadJson = yield* eventPayloadJson(input.payload)
    const eventId = runtime.makeEventId()

    yield* d1Effect('adjutantAssignments.events.insert', () =>
      db
        .prepare(
          `INSERT INTO adjutant_assignment_events
             (id,
              assignment_id,
              software_order_id,
              site_id,
              goal_id,
              run_id,
              event_type,
              visibility,
              summary,
              actor_user_id,
              payload_json,
              created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          eventId,
          assignment.id,
          assignment.softwareOrderId,
          assignment.siteId,
          assignment.goalId,
          input.runId ?? assignment.currentRunId,
          input.eventType,
          assignment.visibility,
          input.summary,
          input.actorUserId ?? assignment.assignedByUserId,
          payloadJson,
          runtime.nowIso(),
        )
        .run(),
    )

    if (mirror !== undefined) {
      yield* Effect.promise(() =>
        mirror.mirrorRowsByKey('adjutant_assignment_events', [[eventId]]),
      )
    }
  })

const recordAssignmentEventById = (
  db: D1Database,
  runtime: AdjutantAssignmentRuntime,
  input: RecordAdjutantAssignmentEventInput,
  mirror?: SupervisionLongtailMirror | undefined,
): Effect.Effect<void, AdjutantAssignmentError> =>
  Effect.gen(function* () {
    const assignment = yield* readAssignmentById(db, input.assignmentId)

    if (assignment === null) {
      return yield* new AdjutantAssignmentNotFound({
        assignmentId: input.assignmentId,
      })
    }

    yield* recordAssignmentEvent(
      db,
      runtime,
      assignment,
      {
        actorUserId: input.actorUserId,
        eventType: input.eventType,
        payload: input.payload,
        runId: input.runId,
        summary: input.summary,
      },
      mirror,
    )
  })

const resolveSource = (
  db: D1Database,
  input: CreateAdjutantAssignmentInput,
): Effect.Effect<
  Readonly<{ siteId: string | null; softwareOrderId: string | null }>,
  AdjutantAssignmentError
> =>
  Effect.gen(function* () {
    const siteId = nonEmptyOptional(input.siteId)
    const explicitSoftwareOrderId = nonEmptyOptional(input.softwareOrderId)

    if (siteId === null && explicitSoftwareOrderId === null) {
      return yield* new AdjutantAssignmentValidationError({
        reason: 'Autopilot assignment requires a software order or Site.',
      })
    }

    const site =
      siteId === null ? null : yield* readSiteAssignmentSource(db, siteId)

    if (siteId !== null && site === null) {
      return yield* new AdjutantAssignmentSiteNotFound({ siteId })
    }

    const softwareOrderId =
      explicitSoftwareOrderId ?? site?.software_order_id ?? null

    if (
      site !== null &&
      explicitSoftwareOrderId !== null &&
      site.software_order_id !== null &&
      site.software_order_id !== explicitSoftwareOrderId
    ) {
      return yield* new AdjutantAssignmentValidationError({
        reason: 'Site is linked to a different software order.',
      })
    }

    if (softwareOrderId !== null) {
      const exists = yield* readSoftwareOrderExists(db, softwareOrderId)

      if (!exists) {
        return yield* new AdjutantAssignmentSoftwareOrderNotFound({
          softwareOrderId,
        })
      }
    }

    return { siteId, softwareOrderId }
  })

const ensureAdjutantGoal = (
  goals: AgentGoalRepositoryShape,
  input: Readonly<{
    agentId: string
    goalId: string | null
    objective: string
    projectId: string | null
    teamId: string | null
    visibility: AdjutantAssignmentVisibility
  }>,
): Effect.Effect<string, AdjutantAssignmentError> =>
  Effect.gen(function* () {
    if (input.goalId !== null) {
      const goal = yield* goals
        .getById(input.goalId)
        .pipe(Effect.mapError(mapGoalError('adjutantAssignments.goal.read')))

      return goal.id
    }

    const scope = {
      agentId: input.agentId,
      projectId: input.projectId,
      teamId: input.teamId,
      userId: null,
    } as const
    const current = yield* goals
      .getCurrent(scope)
      .pipe(
        Effect.mapError(mapGoalError('adjutantAssignments.goal.current.read')),
      )

    if (current !== undefined && current.status !== 'complete') {
      return current.id
    }

    const created = yield* goals
      .setGoal({
        ...scope,
        objective: input.objective,
        visibility: input.visibility,
      })
      .pipe(Effect.mapError(mapGoalError('adjutantAssignments.goal.create')))

    return created.id
  })

const assertRunHasGoal = (
  assignment: Readonly<{
    currentRunId: string | null
    goalId: string | null
    id: string
  }>,
): Effect.Effect<void, AdjutantAssignmentRunGoalRequired> =>
  assignment.currentRunId !== null && assignment.goalId === null
    ? Effect.fail(
        new AdjutantAssignmentRunGoalRequired({
          assignmentId: assignment.id,
          currentRunId: assignment.currentRunId,
          reason:
            'A run-linked Autopilot assignment must be attached to a durable goal.',
        }),
      )
    : Effect.void

const createAssignment = (
  db: D1Database,
  goals: AgentGoalRepositoryShape,
  runtime: AdjutantAssignmentRuntime,
  input: CreateAdjutantAssignmentInput,
  mirror?: SupervisionLongtailMirror | undefined,
): Effect.Effect<AdjutantAssignment, AdjutantAssignmentError> =>
  Effect.gen(function* () {
    yield* assertPayloadSafe(input)

    const objective = input.objective.trim()

    if (objective === '') {
      return yield* new AdjutantAssignmentValidationError({
        reason: 'Autopilot assignment objective is required.',
      })
    }

    const source = yield* resolveSource(db, input)
    const active = yield* readActiveAssignment(db, source)

    if (active !== null) {
      return yield* new AdjutantAssignmentActiveExists({
        assignmentId: active.id,
        siteId: source.siteId,
        softwareOrderId: source.softwareOrderId,
      })
    }

    const now = runtime.nowIso()
    const teamId = nonEmptyOptional(input.teamId) ?? 'team_openagents_core'
    const projectId = nonEmptyOptional(input.projectId) ?? 'project_adjutant'
    const agentId = nonEmptyOptional(input.agentId) ?? 'agent_adjutant'
    const visibility = input.visibility ?? 'team'
    const goalId = yield* ensureAdjutantGoal(goals, {
      agentId,
      goalId: nonEmptyOptional(input.goalId),
      objective,
      projectId,
      teamId,
      visibility,
    })
    const assignment: AdjutantAssignment = {
      id: runtime.makeAssignmentId(),
      softwareOrderId: source.softwareOrderId,
      siteId: source.siteId,
      goalId,
      currentRunId: nonEmptyOptional(input.currentRunId),
      teamId,
      projectId,
      agentId,
      assignedByUserId: nonEmptyOptional(input.assignedByUserId),
      assignmentKind: input.assignmentKind,
      status: input.status ?? 'draft',
      visibility,
      taskSpecPath: nonEmptyOptional(input.taskSpecPath),
      commitSha: nonEmptyOptional(input.commitSha),
      objective,
      createdAt: now,
      updatedAt: now,
      completedAt: null,
      blockedAt: null,
      archivedAt: null,
    }

    yield* d1Effect('adjutantAssignments.insert', () =>
      db
        .prepare(
          `INSERT INTO adjutant_assignments
             (id,
              software_order_id,
              site_id,
              goal_id,
              current_run_id,
              team_id,
              project_id,
              agent_id,
              assigned_by_user_id,
              assignment_kind,
              status,
              visibility,
              task_spec_path,
              commit_sha,
              objective,
              created_at,
              updated_at,
              completed_at,
              blocked_at,
              archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL, NULL)`,
        )
        .bind(
          assignment.id,
          assignment.softwareOrderId,
          assignment.siteId,
          assignment.goalId,
          assignment.currentRunId,
          assignment.teamId,
          assignment.projectId,
          assignment.agentId,
          assignment.assignedByUserId,
          assignment.assignmentKind,
          assignment.status,
          assignment.visibility,
          assignment.taskSpecPath,
          assignment.commitSha,
          assignment.objective,
          assignment.createdAt,
          assignment.updatedAt,
        )
        .run(),
    )

    if (mirror !== undefined) {
      yield* Effect.promise(() =>
        mirror.mirrorRowsByKey('adjutant_assignments', [[assignment.id]]),
      )
    }

    yield* recordAssignmentEvent(
      db,
      runtime,
      assignment,
      {
        eventType: 'adjutant.assignment_created',
        payload: {
          assignmentKind: assignment.assignmentKind,
          status: assignment.status,
          visibility: assignment.visibility,
        },
        summary: 'Autopilot assignment created.',
      },
      mirror,
    )

    return assignment
  })

const updateAssignment = (
  db: D1Database,
  runtime: AdjutantAssignmentRuntime,
  input: UpdateAdjutantAssignmentInput,
  mirror?: SupervisionLongtailMirror | undefined,
): Effect.Effect<AdjutantAssignment, AdjutantAssignmentError> =>
  Effect.gen(function* () {
    yield* assertPayloadSafe(input)

    const current = yield* readAssignmentById(db, input.assignmentId)

    if (current === null) {
      return yield* new AdjutantAssignmentNotFound({
        assignmentId: input.assignmentId,
      })
    }

    const now = runtime.nowIso()
    const status = input.status ?? current.status
    const objective = input.objective?.trim() ?? current.objective
    const goalId =
      input.goalId === undefined ? current.goalId : nullableInput(input.goalId)
    const currentRunId =
      input.currentRunId === undefined
        ? current.currentRunId
        : nullableInput(input.currentRunId)
    const taskSpecPath =
      input.taskSpecPath === undefined
        ? current.taskSpecPath
        : nullableInput(input.taskSpecPath)
    const commitSha =
      input.commitSha === undefined
        ? current.commitSha
        : nullableInput(input.commitSha)

    if (objective === '') {
      return yield* new AdjutantAssignmentValidationError({
        reason: 'Autopilot assignment objective is required.',
      })
    }

    yield* assertRunHasGoal({
      currentRunId,
      goalId,
      id: current.id,
    })

    const completedAt =
      status === 'complete' && current.completedAt === null
        ? now
        : current.completedAt
    const blockedAt =
      status === 'blocked' && current.blockedAt === null
        ? now
        : current.blockedAt

    yield* d1Effect('adjutantAssignments.update', () =>
      db
        .prepare(
          `UPDATE adjutant_assignments
              SET goal_id = ?,
                  current_run_id = ?,
                  status = ?,
                  task_spec_path = ?,
                  commit_sha = ?,
                  objective = ?,
                  updated_at = ?,
                  completed_at = ?,
                  blocked_at = ?
            WHERE id = ?
              AND archived_at IS NULL`,
        )
        .bind(
          goalId,
          currentRunId,
          status,
          taskSpecPath,
          commitSha,
          objective,
          now,
          completedAt,
          blockedAt,
          input.assignmentId,
        )
        .run(),
    )

    if (mirror !== undefined) {
      yield* Effect.promise(() =>
        mirror.mirrorRowsByKey('adjutant_assignments', [[input.assignmentId]]),
      )
    }

    const updated = yield* readAssignmentById(db, input.assignmentId)

    if (updated === null) {
      return yield* new AdjutantAssignmentNotFound({
        assignmentId: input.assignmentId,
      })
    }

    return updated
  })

export const makeAdjutantAssignmentService = (
  db: D1Database,
  runtime: AdjutantAssignmentRuntime = systemAdjutantAssignmentRuntime,
  goals: AgentGoalRepositoryShape = makeD1AgentGoalRepository(db),
  mirror?: SupervisionLongtailMirror | undefined,
) => ({
  createAssignment: Effect.fn('AdjutantAssignmentService.createAssignment')(
    (input: CreateAdjutantAssignmentInput) =>
      createAssignment(db, goals, runtime, input, mirror),
  ),
  listAssignments: Effect.fn('AdjutantAssignmentService.listAssignments')(
    (limit: number) => listAssignments(db, limit),
  ),
  readAssignmentById: Effect.fn('AdjutantAssignmentService.readAssignmentById')(
    (assignmentId: string) => readAssignmentById(db, assignmentId),
  ),
  recordEvent: Effect.fn('AdjutantAssignmentService.recordEvent')(
    (input: RecordAdjutantAssignmentEventInput) =>
      recordAssignmentEventById(db, runtime, input, mirror),
  ),
  updateAssignment: Effect.fn('AdjutantAssignmentService.updateAssignment')(
    (input: UpdateAdjutantAssignmentInput) =>
      updateAssignment(db, runtime, input, mirror),
  ),
})

export class AdjutantAssignmentService extends Context.Service<
  AdjutantAssignmentService,
  ReturnType<typeof makeAdjutantAssignmentService>
>()('@openagentsinc/autopilot-omega/AdjutantAssignmentService') {
  static layer = (
    env: AdjutantAssignmentEnv,
    runtime: AdjutantAssignmentRuntime = systemAdjutantAssignmentRuntime,
  ) =>
    Layer.succeed(
      AdjutantAssignmentService,
      makeAdjutantAssignmentService(
        openAgentsDatabase(env),
        runtime,
        // KS-8.5 (#8316): goal mutations ride the agent-runtime
        // dual-write seam.
        makeAgentGoalRepositoryForEnv(env),
        // KS-8.17 (#8361): adjutant_assignments/adjutant_assignment_events
        // ride the supervision long-tail read-back mirror.
        makeSupervisionLongtailMirrorForEnv(env),
      ),
    )
}
