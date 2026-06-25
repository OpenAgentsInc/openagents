import {
  filterOpenAgentsMcpDescriptorsByGrantSet,
  openAgentsMcpDescriptorIsGranted,
  openAgentsMcpGrantedAuthoritySet,
  projectOpenAgentsMcpOutput,
  type OpenAgentsMcpAuthorityClass,
  type OpenAgentsMcpGrant,
  type OpenAgentsMcpToolDescriptor,
} from '@openagentsinc/mcp-contract'

import type {
  AgentRegistrationStore,
  LinkedAgentOwnerRecord,
  ProgrammaticAgentSession,
} from './agent-registration'
import type {
  CrmMcpCatalog,
  McpPrincipal,
  McpResourceListing,
  McpResourceReadOutcome,
  McpToolCallOutcome,
  McpToolListing,
} from './crm-mcp-routes'
import {
  delegateCodingWorkflow,
  estimatedDelegatedCodingUsage,
  type CodingDelegationInput,
} from './inference/coding-workflow-delegation'
import type {
  CodingWorkflowClass,
  CodingWorkflowClassification,
} from './inference/coding-workflow-classifier'
import type { ServedTokensRecorderInput } from './inference/served-tokens-recorder'
import {
  pylonCodingServiceCapacityProjection,
  type PylonApiRegistrationRecord,
  type PylonApiStore,
} from './pylon-api'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

const SOURCE =
  'docs/khala/2026-06-25-pylon-cli-mcp-steerable-khala-network-audit.md'

class KhalaMcpToolError extends Error {}

type KhalaMcpEnv = Readonly<Record<string, unknown>>

const KHALA_MCP_AUTHORITY_CLASSES = [
  'private_account_read',
  'coding_session_control',
] as const satisfies ReadonlyArray<OpenAgentsMcpAuthorityClass>

const grantsForAgent = (
  agentUserId: string,
  openauthUserId: string | null | undefined,
  nowIso: string,
): ReadonlyArray<OpenAgentsMcpGrant> => {
  const scopeRefs = [
    `agent_user:${agentUserId}`,
    ...(openauthUserId === null || openauthUserId === undefined
      ? []
      : [`openauth_user:${openauthUserId}`]),
    'scope.khala.own_capacity_only',
  ]

  return KHALA_MCP_AUTHORITY_CLASSES.map(authorityClass => ({
    authorityClass,
    decision: 'granted' as const,
    grantRef: `grant.khala_mcp.${agentUserId}.${authorityClass}`,
    grantedAt: nowIso,
    scopeRefs,
    sourceRefs: ['agent_credentials', SOURCE],
    subjectRef: `agent:${agentUserId}`,
  }))
}

export const khalaMcpAgentPrincipal = (
  session: ProgrammaticAgentSession,
  nowIso: string = currentIsoTimestamp(),
): McpPrincipal => ({
  grants: grantsForAgent(
    session.user.id,
    session.credential.openauthUserId,
    nowIso,
  ),
  subjectRef: `agent:${session.user.id}`,
  tenantRef: `agent:${session.user.id}`,
})

const readTool = (
  name: string,
  title: string,
  description: string,
): OpenAgentsMcpToolDescriptor => ({
  description,
  inputSchemaRef: `${name}.input`,
  name,
  outputSchemaRef: `${name}.output`,
  progressBehavior: 'none',
  publicSummary: title,
  receiptBehavior: 'none',
  requiredAuthorities: ['private_account_read'],
  riskClass: 'read_only',
  sourceRefs: [SOURCE],
  title,
})

const requestTool = (): OpenAgentsMcpToolDescriptor => ({
  description:
    'Issue a streamed Khala coding workflow through caller-owned linked Pylon capacity and return a durable resume handle.',
  inputSchemaRef: 'khala.request.input',
  name: 'khala.request',
  outputSchemaRef: 'khala.request.output',
  progressBehavior: 'streaming',
  publicSummary: 'Request Khala coding work',
  receiptBehavior: 'mutation_receipt',
  requiredAuthorities: ['coding_session_control'],
  riskClass: 'low',
  sourceRefs: [SOURCE],
  title: 'Request Khala coding work',
})

export const KHALA_MCP_TOOLS: ReadonlyArray<OpenAgentsMcpToolDescriptor> = [
  requestTool(),
  readTool(
    'khala.resume',
    'Resume Khala stream',
    'Read a durable Khala stream suffix from an offset. This is read-only and never meters.',
  ),
  readTool(
    'khala.capacity',
    'Read Khala capacity',
    'Read the caller-owned linked Pylon coding capacity projection.',
  ),
  readTool(
    'khala.status',
    'Read Khala stream status',
    'Read durable Khala stream headers/status. This is read-only and never meters.',
  ),
]

const REQUEST_SCHEMA: Record<string, unknown> = {
  additionalProperties: false,
  properties: {
    objective: { description: 'Objective text alias for prompt.', type: 'string' },
    prompt: { description: 'Coding request prompt/objective.', type: 'string' },
    pylonRef: { description: 'Caller-owned target Pylon ref.', type: 'string' },
    targetPylonRef: {
      description: 'Caller-owned target Pylon ref.',
      type: 'string',
    },
    workflow: {
      description: 'Typed coding workflow class.',
      enum: ['cloud_coding_session', 'codex_agent_task'],
      type: 'string',
    },
  },
  type: 'object',
}

const DURABLE_READ_SCHEMA: Record<string, unknown> = {
  additionalProperties: false,
  properties: {
    durableRequestId: { description: 'Durable request id.', type: 'string' },
    offset: {
      description: 'Resume byte offset.',
      type: ['integer', 'string'],
    },
  },
  required: ['durableRequestId'],
  type: 'object',
}

const KHALA_MCP_INPUT_SCHEMAS: Readonly<Record<string, Record<string, unknown>>> =
  {
    'khala.capacity': { additionalProperties: false, properties: {}, type: 'object' },
    'khala.request': REQUEST_SCHEMA,
    'khala.resume': DURABLE_READ_SCHEMA,
    'khala.status': DURABLE_READ_SCHEMA,
  }

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

const argsRecord = (value: unknown): Record<string, unknown> =>
  isRecord(value) ? value : {}

const optionalString = (
  args: Record<string, unknown>,
  key: string,
): string | undefined => {
  const value = args[key]
  return typeof value === 'string' && value.trim() !== ''
    ? value.trim()
    : undefined
}

const requiredString = (args: Record<string, unknown>, key: string): string => {
  const value = optionalString(args, key)
  if (value === undefined) {
    throw new KhalaMcpToolError(`${key} is required`)
  }
  return value
}

const optionalOffset = (
  args: Record<string, unknown>,
): number | string | undefined => {
  const value = args.offset
  if (typeof value === 'number' && Number.isFinite(value)) {
    return Math.max(0, Math.trunc(value))
  }
  return optionalString(args, 'offset')
}

const workflowFromArgs = (args: Record<string, unknown>): Exclude<CodingWorkflowClass, 'none'> => {
  const value = optionalString(args, 'workflow') ?? 'codex_agent_task'
  if (value === 'cloud_coding_session' || value === 'codex_agent_task') {
    return value
  }
  throw new KhalaMcpToolError(
    'workflow must be cloud_coding_session or codex_agent_task',
  )
}

const grantScopeRefs = (principal: McpPrincipal): ReadonlyArray<string> =>
  principal.grants
    .filter(grant => grant.decision === 'granted')
    .flatMap(grant => grant.scopeRefs)

const scopedOpenAuthUserIds = (principal: McpPrincipal): ReadonlyArray<string> =>
  [
    ...new Set(
      grantScopeRefs(principal)
        .filter(ref => ref.startsWith('openauth_user:'))
        .map(ref => ref.slice('openauth_user:'.length))
        .filter(ref => ref.trim() !== ''),
    ),
  ]

const scopedAgentUserIds = (principal: McpPrincipal): ReadonlyArray<string> => {
  const scoped = grantScopeRefs(principal)
    .filter(ref => ref.startsWith('agent_user:'))
    .map(ref => ref.slice('agent_user:'.length))
    .filter(ref => ref.trim() !== '')
  const subjectAgent = principal.subjectRef.startsWith('agent:')
    ? principal.subjectRef.slice('agent:'.length)
    : ''
  return [
    ...new Set([
      ...scoped,
      ...(subjectAgent === '' ? [] : [subjectAgent]),
    ]),
  ]
}

const linkedAgentsForPrincipal = async (
  store: AgentRegistrationStore,
  principal: McpPrincipal,
): Promise<ReadonlyArray<LinkedAgentOwnerRecord>> => {
  const linked: LinkedAgentOwnerRecord[] = []
  for (const openauthUserId of scopedOpenAuthUserIds(principal)) {
    if (store.listLinkedAgentsForOpenAuthUser === undefined) {
      continue
    }
    linked.push(
      ...(await store.listLinkedAgentsForOpenAuthUser(openauthUserId, 100)),
    )
  }

  for (const agentUserId of scopedAgentUserIds(principal)) {
    linked.push({
      agentUserId,
      credentialId: null,
      displayName: agentUserId,
      linkKind: 'credential_anchor',
      openauthUserId: scopedOpenAuthUserIds(principal)[0] ?? agentUserId,
      tokenPrefix: null,
    })
  }

  const seen = new Set<string>()
  return linked.filter(agent => {
    if (seen.has(agent.agentUserId)) return false
    seen.add(agent.agentUserId)
    return true
  })
}

const linkedRegistrations = async (
  pylonStore: PylonApiStore,
  linkedAgents: ReadonlyArray<LinkedAgentOwnerRecord>,
): Promise<ReadonlyArray<PylonApiRegistrationRecord>> => {
  const ownerIds = linkedAgents.map(agent => agent.agentUserId)
  return pylonStore.listRegistrationsForOwnerAgentUserIds === undefined
    ? (await pylonStore.listRegistrations(200)).filter(registration =>
        ownerIds.includes(registration.ownerAgentUserId),
      )
    : pylonStore.listRegistrationsForOwnerAgentUserIds(ownerIds, 200)
}

const projectToolOutcome = (
  name: string,
  data: unknown,
  isError = false,
): McpToolCallOutcome => {
  const text = JSON.stringify(data, null, 2)
  const projection = projectOpenAgentsMcpOutput({
    maxTextBytes: 131072,
    outputRef: `mcp.khala.${name}`,
    safetyClass: 'operator',
    sourceRefs: [`tool.${name}`, SOURCE],
    text,
  })
  if (projection.text === undefined) {
    return {
      content: [{ text: projection.summary, type: 'text' }],
      isError: true,
      structuredContent: data,
    }
  }
  return {
    content: [{ text: projection.text, type: 'text' }],
    isError,
    structuredContent: data,
  }
}

const toolErrorOutcome = (
  name: string,
  error: string,
  extra: Record<string, unknown> = {},
): McpToolCallOutcome =>
  projectToolOutcome(
    name,
    {
      error,
      ok: false,
      schema: 'openagents.khala_mcp.tool_error.v1',
      ...extra,
    },
    true,
  )

const toolListing = (descriptor: OpenAgentsMcpToolDescriptor): McpToolListing => ({
  annotations: {
    readOnlyHint: descriptor.receiptBehavior === 'none',
    requiredAuthorities: descriptor.requiredAuthorities,
  },
  description: descriptor.description,
  inputSchema: KHALA_MCP_INPUT_SCHEMAS[descriptor.name] ?? { type: 'object' },
  name: descriptor.name,
  title: descriptor.title,
})

const classificationForWorkflow = (
  workflowClass: Exclude<CodingWorkflowClass, 'none'>,
): CodingWorkflowClassification => ({
  confidence: 1,
  evidenceRefs: ['evidence.coding_workflow.mcp_tool_contract'],
  workflowClass,
})

const rawBodyForRequest = (
  prompt: string,
  workflowClass: Exclude<CodingWorkflowClass, 'none'>,
  targetPylonRef: string | undefined,
): Record<string, unknown> => ({
  messages: [{ content: prompt, role: 'user' }],
  model: 'openagents/khala',
  openagents: {
    coding:
      targetPylonRef === undefined
        ? {}
        : { targetPylonRef },
    workflowClass,
  },
  stream: true,
})

const requestPayload = (
  args: Record<string, unknown>,
): Readonly<{
  rawBody: Record<string, unknown>
  workflowClass: Exclude<CodingWorkflowClass, 'none'>
}> => {
  const prompt = optionalString(args, 'prompt') ?? optionalString(args, 'objective')
  if (prompt === undefined) {
    throw new KhalaMcpToolError('khala.request requires prompt or objective')
  }
  const workflowClass = workflowFromArgs(args)
  const targetPylonRef =
    optionalString(args, 'targetPylonRef') ?? optionalString(args, 'pylonRef')
  return {
    rawBody: rawBodyForRequest(prompt, workflowClass, targetPylonRef),
    workflowClass,
  }
}

const durableReadUrl = (
  requestUrl: string,
  durableRequestId: string,
  offset: number | string | undefined,
): string => {
  const url = new URL(
    `/v1/chat/completions/durable/${encodeURIComponent(durableRequestId)}`,
    requestUrl,
  )
  if (offset !== undefined) url.searchParams.set('offset', String(offset))
  return url.toString()
}

const durableReadProjection = async (
  input: Readonly<{
    fetcher: typeof fetch
    request: Request
    durableRequestId: string
    offset?: number | string | undefined
  }>,
): Promise<unknown> => {
  const url = durableReadUrl(
    input.request.url,
    input.durableRequestId,
    input.offset,
  )
  const response = await input.fetcher(
    new Request(url, {
      headers:
        input.request.headers.get('authorization') === null
          ? {}
          : { authorization: input.request.headers.get('authorization')! },
      method: 'GET',
    }),
  )
  const text = await response.text()
  return {
    durableRequestId: input.durableRequestId,
    nextOffset: response.headers.get('stream-next-offset'),
    ok: response.ok,
    schema: 'openagents.khala_mcp.durable_read.v1',
    status: response.status,
    streamClosed: response.headers.get('stream-closed') === 'true',
    streamUpToDate: response.headers.get('stream-up-to-date') === 'true',
    text,
    url: new URL(url).pathname + new URL(url).search,
  }
}

export type KhalaMcpCatalogDeps<Bindings extends KhalaMcpEnv> = Readonly<{
  agentStore: (env: Bindings) => AgentRegistrationStore
  durableFetch?: typeof fetch
  makeId?: () => string
  nowIso?: () => string
  pylonStore: (env: Bindings) => PylonApiStore
  recordTokensServed?: (
    env: Bindings,
  ) => (input: ServedTokensRecorderInput) => Promise<void>
}>

export const makeKhalaMcpCatalog = <Bindings extends KhalaMcpEnv>(
  deps: KhalaMcpCatalogDeps<Bindings>,
): CrmMcpCatalog<Bindings> => ({
  callTool: async (env, request, principal, name, callArgs) => {
    const grantedAuthorities = openAgentsMcpGrantedAuthoritySet(principal.grants)
    const descriptor = KHALA_MCP_TOOLS.find(tool => tool.name === name)
    if (
      descriptor === undefined ||
      !openAgentsMcpDescriptorIsGranted(descriptor, grantedAuthorities)
    ) {
      throw new KhalaMcpToolError('unknown_tool')
    }

    const args = argsRecord(callArgs)
    try {
      if (name === 'khala.capacity') {
        const linkedAgents = await linkedAgentsForPrincipal(
          deps.agentStore(env),
          principal,
        )
        const registrations = await linkedRegistrations(
          deps.pylonStore(env),
          linkedAgents,
        )
        return projectToolOutcome(name, {
          linkedAgentCount: linkedAgents.length,
          pylons: registrations.map(registration => ({
            codingCapacity: pylonCodingServiceCapacityProjection(registration),
            latestCapacityRefs: registration.latestCapacityRefs,
            latestHeartbeatAt: registration.latestHeartbeatAt,
            latestHeartbeatStatus: registration.latestHeartbeatStatus,
            latestHealthRefs: registration.latestHealthRefs,
            latestLoadRefs: registration.latestLoadRefs,
            ownerAgentUserId: registration.ownerAgentUserId,
            pylonRef: registration.pylonRef,
            status: registration.status,
          })),
          schema: 'openagents.khala_mcp.capacity.v1',
        })
      }

      if (name === 'khala.request') {
        const payload = requestPayload(args)
        const linkedAgents = await linkedAgentsForPrincipal(
          deps.agentStore(env),
          principal,
        )
        const makeId = deps.makeId ?? (() => compactRandomId('chatcmpl'))
        const requestId = makeId()
        const nowIso = (deps.nowIso ?? currentIsoTimestamp)()
        const delegationInput: CodingDelegationInput = {
          classification: classificationForWorkflow(payload.workflowClass),
          linkedAgents,
          makeId,
          nowIso,
          pylonStore: deps.pylonStore(env),
          rawBody: payload.rawBody,
          requestId,
        }
        const delegation = await delegateCodingWorkflow(delegationInput)
        if (delegation?.kind === 'rejected') {
          return toolErrorOutcome(name, delegation.error, {
            evidenceRefs: delegation.evidenceRefs,
            reason: delegation.reason,
            requestedPylonRef: delegation.requestedPylonRef,
            statusCode: delegation.statusCode,
          })
        }
        if (delegation === null) {
          return toolErrorOutcome(name, 'linked_pylon_capacity_unavailable', {
            reason:
              'No linked, heartbeat-fresh, Codex-capable Pylon capacity is available for this account.',
          })
        }
        const recordTokensServed = deps.recordTokensServed?.(env)
        if (recordTokensServed !== undefined) {
          const messages = Array.isArray(payload.rawBody.messages)
            ? payload.rawBody.messages
            : []
          await recordTokensServed({
            accountRef: principal.subjectRef,
            adapterId: 'pylon-codex-own-capacity',
            requestAttribution: {
              demandKind: 'own_capacity',
              demandSource: 'khala_mcp_request',
            },
            requestId,
            requestedModel: 'openagents/khala',
            servedModel: 'openagents/pylon-codex',
            streamed: true,
            usage: estimatedDelegatedCodingUsage(messages),
          }).catch(() => undefined)
        }
        return projectToolOutcome(name, {
          assignmentRef: delegation.assignment.assignmentRef,
          durableRequestId: requestId,
          durableStreamUrl: delegation.durableStreamUrl,
          evidenceRefs: delegation.evidenceRefs,
          ok: true,
          pylonRef: delegation.pylon.pylonRef,
          schema: 'openagents.khala_mcp.request.v1',
          stream: true,
          workflow: payload.workflowClass,
        })
      }

      if (name === 'khala.resume' || name === 'khala.status') {
        const durableRequestId = requiredString(args, 'durableRequestId')
        const data = await durableReadProjection({
          durableRequestId,
          fetcher: deps.durableFetch ?? fetch,
          offset: optionalOffset(args),
          request,
        })
        const failed =
          isRecord(data) &&
          typeof data.status === 'number' &&
          data.status >= 400
        return projectToolOutcome(name, data, failed)
      }
    } catch (error) {
      return toolErrorOutcome(
        name,
        error instanceof Error ? error.message : String(error),
      )
    }

    throw new KhalaMcpToolError('unknown_tool')
  },
  listResources: () => Promise.resolve([]),
  listTools: (_env, _request, principal) =>
    Promise.resolve(
      filterOpenAgentsMcpDescriptorsByGrantSet(KHALA_MCP_TOOLS, principal.grants)
        .map(toolListing),
    ),
  readResource: () => Promise.reject(new KhalaMcpToolError('unknown_resource')),
})

const isUnknownTool = (error: unknown): boolean =>
  error instanceof Error && error.message === 'unknown_tool'

const isUnknownResource = (error: unknown): boolean =>
  error instanceof Error && error.message === 'unknown_resource'

export const combineMcpCatalogs = <Bindings>(
  catalogs: ReadonlyArray<CrmMcpCatalog<Bindings>>,
): CrmMcpCatalog<Bindings> => ({
  callTool: async (env, request, principal, name, args) => {
    for (const catalog of catalogs) {
      try {
        return await catalog.callTool(env, request, principal, name, args)
      } catch (error) {
        if (!isUnknownTool(error)) throw error
      }
    }
    throw new KhalaMcpToolError('unknown_tool')
  },
  listResources: async (env, request, principal): Promise<ReadonlyArray<McpResourceListing>> =>
    (
      await Promise.all(
        catalogs.map(catalog => catalog.listResources(env, request, principal)),
      )
    ).flat(),
  listTools: async (env, request, principal): Promise<ReadonlyArray<McpToolListing>> =>
    (
      await Promise.all(
        catalogs.map(catalog => catalog.listTools(env, request, principal)),
      )
    ).flat(),
  readResource: async (
    env,
    request,
    principal,
    uri,
  ): Promise<McpResourceReadOutcome> => {
    for (const catalog of catalogs) {
      try {
        return await catalog.readResource(env, request, principal, uri)
      } catch (error) {
        if (!isUnknownResource(error)) throw error
      }
    }
    throw new KhalaMcpToolError('unknown_resource')
  },
})
