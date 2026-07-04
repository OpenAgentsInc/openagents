import {
  decideAgentDefinitionToolAuthority,
  type AgentDefinition,
} from '@openagentsinc/agent-runtime-schema'
import { Schema as S } from 'effect'

import type { AgentDefinitionStore } from './agent-definition-routes'
import type { AgentDefinitionRunStore } from './agent-definition-run-routes'
import { withAgentRateLimitHeaders } from './agent-rate-limit-policy'
import {
  AGENT_TOKEN_PREFIX,
  type AgentRegistrationStore,
  type ProgrammaticAgentSession,
  authenticateProgrammaticAgent,
} from './agent-registration'
import {
  type EventLedgerHandledState,
  EventLedgerHandledState as EventLedgerHandledStateSchema,
  type EventLedgerStore,
  eventLedgerGatewayReadProjectionForDefinition,
} from './event-ledger'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  unauthorized,
} from './http/responses'
import { decodeUnknownWithSchema, readJsonObject } from './json-boundary'
import { currentIsoTimestamp } from './runtime-primitives'

type HttpResponse = globalThis.Response

export const AGENT_DEFINITION_EVENT_LEDGER_READ_TOOL_REF =
  'tool.openagents.event_ledger.read' as const
export const AGENT_DEFINITION_EVENT_LEDGER_HANDLED_STATE_TOOL_REF =
  'tool.openagents.event_ledger.handled_state.write' as const

const TrimmedString = S.Trim
const NonEmptyTrimmedString = TrimmedString.check(S.isNonEmpty())
const PublicSafeRef = NonEmptyTrimmedString.check(
  S.isMinLength(3),
  S.isMaxLength(260),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9_.:/=-]*$/),
)

const EventLedgerHandledStateUpdateRequest = S.Struct({
  entryId: PublicSafeRef,
  handledState: EventLedgerHandledStateSchema,
  reasonRef: S.optionalKey(PublicSafeRef),
  runId: PublicSafeRef,
})
type EventLedgerHandledStateUpdateRequest =
  typeof EventLedgerHandledStateUpdateRequest.Type

type EventLedgerGatewayRouteKind = 'read' | 'handled_state'

export type AgentDefinitionEventLedgerGatewayDependencies = Readonly<{
  agentStore: AgentRegistrationStore
  definitionStore: Pick<AgentDefinitionStore, 'readDefinition'>
  eventLedgerStore: Pick<
    EventLedgerStore,
    'listOwnerEntries' | 'updateHandledState'
  >
  nowIso?: (() => string) | undefined
  runStore: Pick<AgentDefinitionRunStore, 'readRun'>
}>

export const matchAgentDefinitionEventLedgerGatewayRequest = (
  request: Request,
): Readonly<{
  definitionId: string
  route: EventLedgerGatewayRouteKind
}> | undefined => {
  const pathname = new URL(request.url).pathname
  const readMatch = /^\/v1\/agent-definitions\/([^/]+)\/event-ledger$/.exec(
    pathname,
  )
  const handledStateMatch =
    /^\/v1\/agent-definitions\/([^/]+)\/event-ledger\/handled-state$/.exec(
      pathname,
    )
  const match = readMatch ?? handledStateMatch

  if (match === null) {
    return undefined
  }

  try {
    const definitionId = decodeURIComponent(match[1] ?? '').trim()

    return definitionId === ''
      ? undefined
      : {
          definitionId,
          route: readMatch === null ? 'handled_state' : 'read',
        }
  } catch {
    return undefined
  }
}

const bearerTokenFromRequest = (request: Request): string | undefined => {
  const authorization = request.headers.get('authorization')

  if (authorization === null) {
    return undefined
  }

  const [scheme, token] = authorization.split(' ')

  return scheme?.toLowerCase() === 'bearer' &&
    token !== undefined &&
    token.startsWith(AGENT_TOKEN_PREFIX)
    ? token
    : undefined
}

const requireAgentSession = async (
  request: Request,
  dependencies: AgentDefinitionEventLedgerGatewayDependencies,
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

const boundedLimitFromRequest = (request: Request): number => {
  const rawLimit = new URL(request.url).searchParams.get('limit')
  const parsed = rawLimit === null ? 50 : Number(rawLimit)

  return Number.isFinite(parsed)
    ? Math.max(1, Math.min(100, Math.trunc(parsed)))
    : 50
}

const handledStatesFromRequest = (
  request: Request,
): ReadonlyArray<EventLedgerHandledState> | undefined => {
  const params = new URL(request.url).searchParams
  const states = [
    ...params.getAll('state'),
    ...(params.get('states') ?? '')
      .split(',')
      .map(value => value.trim())
      .filter(value => value !== ''),
  ]

  return states.length === 0
    ? undefined
    : decodeUnknownWithSchema(S.Array(EventLedgerHandledStateSchema), states)
}

const subjectRefFromRequest = (request: Request): string | undefined => {
  const subjectRef = new URL(request.url).searchParams.get('subjectRef')?.trim()

  return subjectRef === '' ? undefined : subjectRef
}

const notFoundResponse = (): HttpResponse =>
  withAgentRateLimitHeaders(
    noStoreJsonResponse({ error: 'agent_definition_not_found' }, { status: 404 }),
  )

const storageErrorResponse = (): HttpResponse =>
  withAgentRateLimitHeaders(
    noStoreJsonResponse(
      { error: 'agent_definition_event_ledger_storage_error' },
      { status: 503 },
    ),
  )

const invalidRequestResponse = (reason: string): HttpResponse =>
  withAgentRateLimitHeaders(
    noStoreJsonResponse(
      { error: 'invalid_agent_definition_event_ledger_request', reason },
      { status: 400 },
    ),
  )

const toolAuthorityResponse = (
  definition: AgentDefinition,
  toolRef: string,
): HttpResponse | undefined => {
  const decision = decideAgentDefinitionToolAuthority({
    definition,
    invocationRef: `event_ledger.gateway.${definition.id}`,
    toolRef,
  })

  return decision.allowed
    ? undefined
    : withAgentRateLimitHeaders(
        noStoreJsonResponse(
          {
            error: 'agent_definition_tool_not_authorized',
            blockerRefs: decision.blockerRefs,
            escalation: decision.escalation,
            reasonRef: decision.reasonRef,
            status: decision.status,
            toolRef: decision.toolRef,
          },
          {
            status:
              decision.status === 'operator_escalation_required' ? 409 : 403,
          },
        ),
      )
}

const handleRead = async (
  request: Request,
  definition: AgentDefinition,
  session: ProgrammaticAgentSession,
  dependencies: AgentDefinitionEventLedgerGatewayDependencies,
): Promise<HttpResponse> => {
  const denied = toolAuthorityResponse(
    definition,
    AGENT_DEFINITION_EVENT_LEDGER_READ_TOOL_REF,
  )

  if (denied !== undefined) {
    return denied
  }

  let handledStates: ReadonlyArray<EventLedgerHandledState> | undefined

  try {
    handledStates = handledStatesFromRequest(request)
  } catch (error) {
    return invalidRequestResponse(
      error instanceof Error ? error.message : String(error),
    )
  }

  try {
    const entries = await dependencies.eventLedgerStore.listOwnerEntries({
      handledStates,
      limit: boundedLimitFromRequest(request),
      ownerAgentUserId: session.user.id,
      subjectRef: subjectRefFromRequest(request),
    })

    return withAgentRateLimitHeaders(
      noStoreJsonResponse({
        toolRef: AGENT_DEFINITION_EVENT_LEDGER_READ_TOOL_REF,
        ...eventLedgerGatewayReadProjectionForDefinition(definition, entries),
      }),
    )
  } catch {
    return storageErrorResponse()
  }
}

const handleHandledStateUpdate = async (
  request: Request,
  definition: AgentDefinition,
  session: ProgrammaticAgentSession,
  dependencies: AgentDefinitionEventLedgerGatewayDependencies,
): Promise<HttpResponse> => {
  const denied = toolAuthorityResponse(
    definition,
    AGENT_DEFINITION_EVENT_LEDGER_HANDLED_STATE_TOOL_REF,
  )

  if (denied !== undefined) {
    return denied
  }

  let input: EventLedgerHandledStateUpdateRequest

  try {
    input = decodeUnknownWithSchema(
      EventLedgerHandledStateUpdateRequest,
      await readJsonObject(request),
    )
  } catch (error) {
    return invalidRequestResponse(
      error instanceof Error ? error.message : String(error),
    )
  }

  const run = await dependencies.runStore
    .readRun(session.user.id, input.runId)
    .catch(() => undefined)

  if (run === undefined || run.definitionId !== definition.id) {
    return withAgentRateLimitHeaders(
      noStoreJsonResponse(
        { error: 'agent_definition_run_not_found' },
        { status: 404 },
      ),
    )
  }

  try {
    const updated = await dependencies.eventLedgerStore.updateHandledState({
      entryId: input.entryId,
      handledAt: dependencies.nowIso?.() ?? currentIsoTimestamp(),
      handledByDefinitionId: definition.id,
      handledByRunId: run.runId,
      handledReasonRef: input.reasonRef,
      handledState: input.handledState,
      ownerAgentUserId: session.user.id,
    })

    if (updated === undefined) {
      return withAgentRateLimitHeaders(
        noStoreJsonResponse(
          { error: 'event_ledger_entry_not_found' },
          { status: 404 },
        ),
      )
    }

    return withAgentRateLimitHeaders(
      noStoreJsonResponse({
        toolRef: AGENT_DEFINITION_EVENT_LEDGER_HANDLED_STATE_TOOL_REF,
        ...eventLedgerGatewayReadProjectionForDefinition(definition, [updated]),
      }),
    )
  } catch {
    return storageErrorResponse()
  }
}

export const handleAgentDefinitionEventLedgerGatewayRequest = async (
  request: Request,
  dependencies: AgentDefinitionEventLedgerGatewayDependencies,
): Promise<HttpResponse | undefined> => {
  const matched = matchAgentDefinitionEventLedgerGatewayRequest(request)

  if (matched === undefined) {
    return undefined
  }

  const allowedMethods = matched.route === 'read' ? ['GET'] : ['POST']

  if (!allowedMethods.includes(request.method)) {
    return withAgentRateLimitHeaders(methodNotAllowed(allowedMethods))
  }

  const session = await requireAgentSession(request, dependencies)

  if (session === undefined) {
    return withAgentRateLimitHeaders(unauthorized())
  }

  const definition = await dependencies.definitionStore.readDefinition(
    session.user.id,
    matched.definitionId,
  )

  if (definition === undefined) {
    return notFoundResponse()
  }

  return matched.route === 'read'
    ? handleRead(request, definition, session, dependencies)
    : handleHandledStateUpdate(request, definition, session, dependencies)
}
