import {
  AgentDefinition as AgentDefinitionSchema,
  AgentDefinitionBudget,
  AgentDefinitionEscalation,
  AgentDefinitionHarness,
  AgentDefinitionLane,
  AgentDefinitionSchemaLiteral,
  AgentDefinitionToolset,
  AgentDefinitionTrigger,
  decodeAgentDefinition,
  type AgentDefinition,
} from '@openagentsinc/agent-runtime-schema'
import { Schema as S } from 'effect'

import { withAgentRateLimitHeaders } from './agent-rate-limit-policy'
import {
  type AgentRegistrationStore,
  type ProgrammaticAgentSession,
  authenticateProgrammaticAgent,
} from './agent-registration'
import {
  readAgentBearerToken as bearerTokenFromRequest,
} from './auth/bearer-token'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  unauthorized,
} from './http/responses'
import {
  decodeUnknownWithSchema,
  parseJsonWithSchema,
  readJsonObject,
} from './json-boundary'
import { currentIsoTimestamp, randomUuid } from './runtime-primitives'
import type { AgentDefinitionTriggerStore } from './agent-definition-trigger-store'

type HttpResponse = globalThis.Response

const TrimmedString = S.Trim
const NonEmptyTrimmedString = TrimmedString.check(S.isNonEmpty())
const AgentDefinitionSlug = TrimmedString.check(
  S.isMinLength(3),
  S.isMaxLength(80),
  S.isPattern(/^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/),
)

const AgentDefinitionCreateRequest = S.Struct({
  name: NonEmptyTrimmedString.check(S.isMaxLength(120)),
  slug: S.optionalKey(AgentDefinitionSlug),
  goal: NonEmptyTrimmedString.check(S.isMaxLength(4000)),
  harness: AgentDefinitionHarness,
  toolset: AgentDefinitionToolset,
  triggers: S.Array(AgentDefinitionTrigger),
  lane: AgentDefinitionLane,
  budget: AgentDefinitionBudget,
  escalation: AgentDefinitionEscalation,
  sourceRefs: S.optionalKey(S.Array(S.String)),
})
type AgentDefinitionCreateRequest = typeof AgentDefinitionCreateRequest.Type

const AgentDefinitionPatchRequest = S.Struct({
  id: NonEmptyTrimmedString,
  name: S.optionalKey(NonEmptyTrimmedString.check(S.isMaxLength(120))),
  slug: S.optionalKey(AgentDefinitionSlug),
  goal: S.optionalKey(NonEmptyTrimmedString.check(S.isMaxLength(4000))),
  harness: S.optionalKey(AgentDefinitionHarness),
  toolset: S.optionalKey(AgentDefinitionToolset),
  triggers: S.optionalKey(S.Array(AgentDefinitionTrigger)),
  lane: S.optionalKey(AgentDefinitionLane),
  budget: S.optionalKey(AgentDefinitionBudget),
  escalation: S.optionalKey(AgentDefinitionEscalation),
  sourceRefs: S.optionalKey(S.Array(S.String)),
})
type AgentDefinitionPatchRequest = typeof AgentDefinitionPatchRequest.Type

type AgentDefinitionRow = Readonly<{
  definition_json: string
}>

export type AgentDefinitionStore = Readonly<{
  createDefinition: (
    ownerAgentUserId: string,
    definition: AgentDefinition,
  ) => Promise<void>
  listDefinitions: (
    ownerAgentUserId: string,
    limit: number,
  ) => Promise<ReadonlyArray<AgentDefinition>>
  readDefinition: (
    ownerAgentUserId: string,
    definitionId: string,
  ) => Promise<AgentDefinition | undefined>
  updateDefinition: (
    ownerAgentUserId: string,
    definition: AgentDefinition,
  ) => Promise<boolean>
}>

export const makeD1AgentDefinitionStore = (
  db: D1Database,
): AgentDefinitionStore => ({
  createDefinition: async (ownerAgentUserId, definition) => {
    await db
      .prepare(
        `INSERT INTO agent_definitions
          (id, owner_agent_user_id, owner_ref, schema_literal, name, slug, goal,
           harness_json, toolset_json, triggers_json, lane, budget_json,
           escalation_json, source_refs_json, definition_json, created_at,
           updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        definition.id,
        ownerAgentUserId,
        definition.ownerRef,
        definition.schema,
        definition.name,
        definition.slug,
        definition.goal,
        JSON.stringify(definition.harness),
        JSON.stringify(definition.toolset),
        JSON.stringify(definition.triggers),
        definition.lane,
        JSON.stringify(definition.budget),
        JSON.stringify(definition.escalation),
        JSON.stringify(definition.sourceRefs),
        JSON.stringify(definition),
        definition.createdAt,
        definition.updatedAt,
      )
      .run()
  },
  listDefinitions: async (ownerAgentUserId, limit) => {
    const rows = await db
      .prepare(
        `SELECT definition_json
           FROM agent_definitions
          WHERE owner_agent_user_id = ?
          ORDER BY updated_at DESC, id ASC
          LIMIT ?`,
      )
      .bind(ownerAgentUserId, limit)
      .all<AgentDefinitionRow>()

    return (rows.results ?? []).map(rowToDefinition)
  },
  readDefinition: async (ownerAgentUserId, definitionId) => {
    const row = await db
      .prepare(
        `SELECT definition_json
           FROM agent_definitions
          WHERE owner_agent_user_id = ?
            AND id = ?
          LIMIT 1`,
      )
      .bind(ownerAgentUserId, definitionId)
      .first<AgentDefinitionRow>()

    return row === null ? undefined : rowToDefinition(row)
  },
  updateDefinition: async (ownerAgentUserId, definition) => {
    const result = await db
      .prepare(
        `UPDATE agent_definitions
            SET owner_ref = ?,
                schema_literal = ?,
                name = ?,
                slug = ?,
                goal = ?,
                harness_json = ?,
                toolset_json = ?,
                triggers_json = ?,
                lane = ?,
                budget_json = ?,
                escalation_json = ?,
                source_refs_json = ?,
                definition_json = ?,
                updated_at = ?
          WHERE owner_agent_user_id = ?
            AND id = ?`,
      )
      .bind(
        definition.ownerRef,
        definition.schema,
        definition.name,
        definition.slug,
        definition.goal,
        JSON.stringify(definition.harness),
        JSON.stringify(definition.toolset),
        JSON.stringify(definition.triggers),
        definition.lane,
        JSON.stringify(definition.budget),
        JSON.stringify(definition.escalation),
        JSON.stringify(definition.sourceRefs),
        JSON.stringify(definition),
        definition.updatedAt,
        ownerAgentUserId,
        definition.id,
      )
      .run()

    return (result.meta?.changes ?? 0) > 0
  },
})

export type AgentDefinitionRouteDependencies = Readonly<{
  agentStore: AgentRegistrationStore
  definitionStore: AgentDefinitionStore
  triggerStore?: AgentDefinitionTriggerStore
  makeId?: () => string
  nowIso?: () => string
}>

const rowToDefinition = (row: AgentDefinitionRow): AgentDefinition =>
  parseJsonWithSchema(AgentDefinitionSchema, row.definition_json)

const errorMessage = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

const isConflictError = (error: unknown): boolean => {
  const message = errorMessage(error).toLowerCase()

  return message.includes('unique constraint') || message.includes('constraint')
}

const ownerRefForSession = (session: ProgrammaticAgentSession): string =>
  `agent:${session.user.id}`

const makeDefinitionId = (dependencies: AgentDefinitionRouteDependencies): string =>
  dependencies.makeId?.() ?? `agent_definition.${randomUuid()}`

const slugFromName = (name: string): string => {
  const rawSlug = name
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
  const boundedSlug = rawSlug.slice(0, 80).replace(/-+$/g, '')

  return boundedSlug.length >= 3
    ? boundedSlug
    : `agent-${boundedSlug === '' ? 'definition' : boundedSlug}`
}

const boundedLimitFromRequest = (request: Request): number => {
  const rawLimit = new URL(request.url).searchParams.get('limit')
  const parsed = rawLimit === null ? 50 : Number(rawLimit)

  return Number.isFinite(parsed)
    ? Math.max(1, Math.min(100, Math.trunc(parsed)))
    : 50
}

const readRouteBody = async (request: Request): Promise<Record<string, unknown>> =>
  readJsonObject(request)

const requireAgentSession = async (
  request: Request,
  dependencies: AgentDefinitionRouteDependencies,
): Promise<ProgrammaticAgentSession | undefined> => {
  const bearerToken = bearerTokenFromRequest(request)

  return bearerToken === undefined
    ? undefined
    : authenticateProgrammaticAgent(
        dependencies.agentStore,
        bearerToken,
        dependencies.nowIso,
      )
}

const buildCreatedDefinition = (
  session: ProgrammaticAgentSession,
  input: AgentDefinitionCreateRequest,
  dependencies: AgentDefinitionRouteDependencies,
): AgentDefinition => {
  const nowIso = dependencies.nowIso?.() ?? currentIsoTimestamp()

  return decodeAgentDefinition({
    schema: AgentDefinitionSchemaLiteral,
    id: makeDefinitionId(dependencies),
    ownerRef: ownerRefForSession(session),
    name: input.name,
    slug: input.slug ?? slugFromName(input.name),
    goal: input.goal,
    harness: input.harness,
    toolset: input.toolset,
    triggers: input.triggers,
    lane: input.lane,
    budget: input.budget,
    escalation: input.escalation,
    sourceRefs: input.sourceRefs ?? [],
    createdAt: nowIso,
    updatedAt: nowIso,
  })
}

const hasPatchFields = (input: AgentDefinitionPatchRequest): boolean =>
  input.name !== undefined ||
  input.slug !== undefined ||
  input.goal !== undefined ||
  input.harness !== undefined ||
  input.toolset !== undefined ||
  input.triggers !== undefined ||
  input.lane !== undefined ||
  input.budget !== undefined ||
  input.escalation !== undefined ||
  input.sourceRefs !== undefined

const buildPatchedDefinition = (
  current: AgentDefinition,
  input: AgentDefinitionPatchRequest,
  dependencies: AgentDefinitionRouteDependencies,
): AgentDefinition =>
  decodeAgentDefinition({
    ...current,
    name: input.name ?? current.name,
    slug: input.slug ?? current.slug,
    goal: input.goal ?? current.goal,
    harness: input.harness ?? current.harness,
    toolset: input.toolset ?? current.toolset,
    triggers: input.triggers ?? current.triggers,
    lane: input.lane ?? current.lane,
    budget: input.budget ?? current.budget,
    escalation: input.escalation ?? current.escalation,
    sourceRefs: input.sourceRefs ?? current.sourceRefs,
    updatedAt: dependencies.nowIso?.() ?? currentIsoTimestamp(),
  })

const invalidDefinitionResponse = (reason: string): HttpResponse =>
  withAgentRateLimitHeaders(
    noStoreJsonResponse(
      { error: 'invalid_agent_definition', reason },
      { status: 400 },
    ),
  )

const notFoundResponse = (): HttpResponse =>
  withAgentRateLimitHeaders(
    noStoreJsonResponse(
      { error: 'agent_definition_not_found' },
      { status: 404 },
    ),
  )

const storageErrorResponse = (): HttpResponse =>
  withAgentRateLimitHeaders(
    noStoreJsonResponse({ error: 'agent_definition_storage_error' }, { status: 500 }),
  )

const conflictResponse = (): HttpResponse =>
  withAgentRateLimitHeaders(
    noStoreJsonResponse({ error: 'agent_definition_conflict' }, { status: 409 }),
  )

const syncDefinitionTriggers = async (
  session: ProgrammaticAgentSession,
  definition: AgentDefinition,
  dependencies: AgentDefinitionRouteDependencies,
  nowIso: string,
): Promise<boolean> => {
  if (dependencies.triggerStore === undefined) {
    return true
  }

  try {
    await dependencies.triggerStore.replaceDefinitionTriggers(
      session.user.id,
      definition,
      nowIso,
    )

    return true
  } catch {
    return false
  }
}

const handleCreate = async (
  request: Request,
  session: ProgrammaticAgentSession,
  dependencies: AgentDefinitionRouteDependencies,
): Promise<HttpResponse> => {
  let input: AgentDefinitionCreateRequest

  try {
    input = decodeUnknownWithSchema(
      AgentDefinitionCreateRequest,
      await readRouteBody(request),
    )
  } catch (error) {
    return invalidDefinitionResponse(errorMessage(error))
  }

  const definition = buildCreatedDefinition(session, input, dependencies)

  try {
    await dependencies.definitionStore.createDefinition(session.user.id, definition)
  } catch (error) {
    return isConflictError(error) ? conflictResponse() : storageErrorResponse()
  }

  if (!await syncDefinitionTriggers(
    session,
    definition,
    dependencies,
    definition.createdAt,
  )) {
    return storageErrorResponse()
  }

  return withAgentRateLimitHeaders(
    noStoreJsonResponse({ definition }, { status: 201 }),
  )
}

const handleListOrRead = async (
  request: Request,
  session: ProgrammaticAgentSession,
  dependencies: AgentDefinitionRouteDependencies,
): Promise<HttpResponse> => {
  const url = new URL(request.url)
  const definitionId = url.searchParams.get('id')?.trim()

  if (definitionId !== undefined && definitionId !== '') {
    const definition = await dependencies.definitionStore.readDefinition(
      session.user.id,
      definitionId,
    )

    return definition === undefined
      ? notFoundResponse()
      : withAgentRateLimitHeaders(noStoreJsonResponse({ definition }))
  }

  const definitions = await dependencies.definitionStore.listDefinitions(
    session.user.id,
    boundedLimitFromRequest(request),
  )

  return withAgentRateLimitHeaders(noStoreJsonResponse({ definitions }))
}

const handlePatch = async (
  request: Request,
  session: ProgrammaticAgentSession,
  dependencies: AgentDefinitionRouteDependencies,
): Promise<HttpResponse> => {
  let input: AgentDefinitionPatchRequest

  try {
    input = decodeUnknownWithSchema(
      AgentDefinitionPatchRequest,
      await readRouteBody(request),
    )
  } catch (error) {
    return invalidDefinitionResponse(errorMessage(error))
  }

  if (!hasPatchFields(input)) {
    return invalidDefinitionResponse('At least one mutable field is required.')
  }

  const current = await dependencies.definitionStore.readDefinition(
    session.user.id,
    input.id,
  )

  if (current === undefined) {
    return notFoundResponse()
  }

  const definition = buildPatchedDefinition(current, input, dependencies)

  try {
    const updated = await dependencies.definitionStore.updateDefinition(
      session.user.id,
      definition,
    )

    if (!updated) {
      return notFoundResponse()
    }

    if (!await syncDefinitionTriggers(
      session,
      definition,
      dependencies,
      definition.updatedAt,
    )) {
      return storageErrorResponse()
    }

    return withAgentRateLimitHeaders(noStoreJsonResponse({ definition }))
  } catch (error) {
    return isConflictError(error) ? conflictResponse() : storageErrorResponse()
  }
}

export const handleAgentDefinitionsApi = async (
  request: Request,
  dependencies: AgentDefinitionRouteDependencies,
): Promise<HttpResponse> => {
  const session = await requireAgentSession(request, dependencies)

  if (session === undefined) {
    return withAgentRateLimitHeaders(unauthorized())
  }

  if (request.method === 'POST') {
    return handleCreate(request, session, dependencies)
  }

  if (request.method === 'GET') {
    return handleListOrRead(request, session, dependencies)
  }

  if (request.method === 'PATCH') {
    return handlePatch(request, session, dependencies)
  }

  return withAgentRateLimitHeaders(methodNotAllowed(['GET', 'POST', 'PATCH']))
}

export const AgentDefinitionRouteSchemas = {
  create: AgentDefinitionCreateRequest,
  durable: AgentDefinitionSchema,
  patch: AgentDefinitionPatchRequest,
} as const
