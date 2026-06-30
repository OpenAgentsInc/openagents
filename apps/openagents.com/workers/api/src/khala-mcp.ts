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
  khalaCodingRequestIdRef,
  type CodingDelegationInput,
} from './inference/coding-workflow-delegation'
import type {
  CodingWorkflowClass,
  CodingWorkflowClassification,
} from './inference/coding-workflow-classifier'
import type { ServedTokensRecorderInput } from './inference/served-tokens-recorder'
import {
  pylonCodingServiceCapacityProjection,
  type PylonApiAssignmentRecord,
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

const spawnTool = (): OpenAgentsMcpToolDescriptor => ({
  description:
    'Spawn a bounded parent Khala coding run with child assignments on caller-owned linked Pylon capacity.',
  inputSchemaRef: 'khala.spawn.input',
  name: 'khala.spawn',
  outputSchemaRef: 'khala.spawn.output',
  progressBehavior: 'streaming',
  publicSummary: 'Spawn Khala workers',
  receiptBehavior: 'mutation_receipt',
  requiredAuthorities: ['coding_session_control'],
  riskClass: 'low',
  sourceRefs: [SOURCE],
  title: 'Spawn Khala workers',
})

export const KHALA_MCP_TOOLS: ReadonlyArray<OpenAgentsMcpToolDescriptor> = [
  requestTool(),
  spawnTool(),
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
  readTool(
    'khala.spawnStatus',
    'Read Khala spawn status',
    'Read child assignment state for a parent Khala spawn ref without exposing private raw events.',
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
      enum: ['claude_agent_task', 'cloud_coding_session', 'codex_agent_task'],
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

const SPAWN_SCHEMA: Record<string, unknown> = {
  additionalProperties: false,
  properties: {
    branch: { description: 'Public repository branch.', type: 'string' },
    commit: { description: 'Pinned public repository commit SHA.', type: 'string' },
    count: {
      description: 'Requested worker count.',
      maximum: 20,
      minimum: 1,
      type: 'integer',
    },
    fixture: {
      description: 'Use the bounded public fixture assignment.',
      type: 'boolean',
    },
    maxParallel: {
      description: 'Maximum child assignments to dispatch in this parent run.',
      maximum: 20,
      minimum: 1,
      type: 'integer',
    },
    objective: { description: 'Spawn objective text alias for prompt.', type: 'string' },
    prompt: { description: 'Spawn objective text.', type: 'string' },
    pylonRef: { description: 'Caller-owned target Pylon ref.', type: 'string' },
    repo: { description: 'Public GitHub owner/repo.', type: 'string' },
    repository: { description: 'Public GitHub owner/repo.', type: 'string' },
    targetPylonRef: {
      description: 'Caller-owned target Pylon ref.',
      type: 'string',
    },
    verify: {
      description: 'Bounded public verification command argv.',
      type: 'string',
    },
    workflow: {
      description: 'Typed coding workflow class.',
      enum: ['claude_agent_task', 'cloud_coding_session', 'codex_agent_task'],
      type: 'string',
    },
  },
  required: ['count'],
  type: 'object',
}

const SPAWN_STATUS_SCHEMA: Record<string, unknown> = {
  additionalProperties: false,
  properties: {
    spawnRef: { description: 'Parent Khala spawn ref.', type: 'string' },
  },
  required: ['spawnRef'],
  type: 'object',
}

const KHALA_MCP_INPUT_SCHEMAS: Readonly<Record<string, Record<string, unknown>>> =
  {
    'khala.capacity': { additionalProperties: false, properties: {}, type: 'object' },
    'khala.request': REQUEST_SCHEMA,
    'khala.resume': DURABLE_READ_SCHEMA,
    'khala.spawn': SPAWN_SCHEMA,
    'khala.spawnStatus': SPAWN_STATUS_SCHEMA,
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
  if (
    value === 'claude_agent_task' ||
    value === 'cloud_coding_session' ||
    value === 'codex_agent_task'
  ) {
    return value
  }
  throw new KhalaMcpToolError(
    'workflow must be claude_agent_task, cloud_coding_session, or codex_agent_task',
  )
}

const MAX_SPAWN_COUNT = 20
const spawnRefPattern = /^spawn\.public\.khala_coding\.[A-Za-z0-9_.:-]{2,160}$/
const pylonRefPattern = /^[a-z0-9][a-z0-9_.:-]{2,119}$/
const githubFullNamePattern = /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/
const gitCommitShaPattern = /^[a-f0-9]{40}$/i
const placeholderCommitShaPattern = /^(0{40}|1{40})$/i
const verificationCommandArgPattern = /^[A-Za-z0-9_./:=@+-]{1,120}$/
const unsafeVerificationCommandArgPattern =
  /(^|[._/:=@+-])(access[_-]?token|bearer|cookie|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|preimage)|preimage|private[_-]?(key|repo)|provider[_-]?(credential|grant|payload|secret|token)|raw[_-]?(command|content|invoice|payment|payload|prompt|repo|runner|state)|secret|seed[_-]?phrase|ssh:|wallet[._-]?(key|material|mnemonic|preimage|secret|seed)|xprv)([._/:=@+-]|$)|\bsk-[A-Za-z0-9_-]{16,}\b|\bln(?:bc|tb|bcrt)[A-Za-z0-9]{20,}\b/i

const boundedInteger = (
  args: Record<string, unknown>,
  key: string,
  fallback: number | null,
): number => {
  const value = args[key]
  const parsed =
    typeof value === 'number' && Number.isFinite(value)
      ? value
      : typeof value === 'string' && value.trim() !== ''
        ? Number.parseInt(value, 10)
        : fallback
  if (parsed === null || !Number.isFinite(parsed)) {
    throw new KhalaMcpToolError(`${key} must be an integer`)
  }
  const integer = Math.trunc(parsed)
  if (integer < 1 || integer > MAX_SPAWN_COUNT) {
    throw new KhalaMcpToolError(`${key} must be between 1 and ${MAX_SPAWN_COUNT}`)
  }
  return integer
}

const spawnObjectiveFromArgs = (args: Record<string, unknown>): string => {
  const objective = optionalString(args, 'prompt') ?? optionalString(args, 'objective')
  if (objective === undefined) {
    throw new KhalaMcpToolError('khala.spawn requires prompt or objective')
  }
  if (objective.length < 3 || objective.length > 8000) {
    throw new KhalaMcpToolError('khala.spawn objective must be 3-8000 characters')
  }
  return objective
}

const targetPylonRefFromArgs = (
  args: Record<string, unknown>,
): string | undefined => {
  const pylonRef =
    optionalString(args, 'targetPylonRef') ?? optionalString(args, 'pylonRef')
  if (pylonRef === undefined) return undefined
  if (!pylonRefPattern.test(pylonRef)) {
    throw new KhalaMcpToolError('targetPylonRef must be a bounded public Pylon ref')
  }
  return pylonRef
}

const publicRefSegment = (value: string): string =>
  value.replaceAll(/[^A-Za-z0-9_.:-]/g, '_').slice(0, 160)

const spawnRefFromId = (id: string): string =>
  `spawn.public.khala_coding.${publicRefSegment(id)}`

const workerRef = (spawnRef: string, index: number): string => {
  const suffix = spawnRef.replace(/^spawn\.public\.khala_coding\./, '')
  return `worker.public.khala_coding.${publicRefSegment(suffix).slice(0, 120)}.${String(index + 1).padStart(2, '0')}`
}

const workerObjective = (
  objective: string,
  index: number,
  count: number,
): string =>
  [
    `Worker ${index + 1}/${count}.`,
    objective,
    'Work independently and return a concise public-safe closeout with evidence refs, blockers, and next step.',
  ].join(' ')

const cleanGithubFullName = (value: string): string => {
  const trimmed = value.trim()
  if (githubFullNamePattern.test(trimmed)) return trimmed
  const github =
    /^https:\/\/github\.com\/([^/\s]+)\/([^/\s#?]+)(?:[/?#].*)?$/.exec(trimmed)
  if (github !== null) return `${github[1]}/${github[2]!.replace(/\.git$/, '')}`
  throw new KhalaMcpToolError('repo must be owner/repo or a public GitHub URL')
}

const cleanCommitSha = (value: string): string => {
  const trimmed = value.trim()
  if (!gitCommitShaPattern.test(trimmed) || placeholderCommitShaPattern.test(trimmed)) {
    throw new KhalaMcpToolError(
      'commit must be a real pinned 40-character commit SHA',
    )
  }
  return trimmed.toLowerCase()
}

const cleanBranch = (value: string | undefined): string => {
  const branch = value?.trim() || 'main'
  if (branch.includes('..') || branch.startsWith('/') || branch.length > 120) {
    throw new KhalaMcpToolError('branch must be a bounded public branch name')
  }
  return branch
}

const verificationArgs = (value: string): string[] => {
  const args = value.trim().split(/\s+/).filter(Boolean)
  if (
    args.length === 0 ||
    args.length > 20 ||
    args.some(arg =>
      !verificationCommandArgPattern.test(arg) ||
      arg.includes('..') ||
      arg.startsWith('/'),
    )
  ) {
    throw new KhalaMcpToolError(
      'verify must be bounded argv tokens without absolute paths or traversal',
    )
  }
  if (args.some(arg => unsafeVerificationCommandArgPattern.test(arg))) {
    throw new KhalaMcpToolError(
      'verify contains private, payment, credential, wallet, or raw material',
    )
  }
  return args
}

const buildSpawnWorkspace = (
  args: Record<string, unknown>,
): Record<string, unknown> | undefined => {
  if (args.fixture === true) return undefined
  const repo = optionalString(args, 'repo') ?? optionalString(args, 'repository')
  const commit = optionalString(args, 'commit')
  const verify = optionalString(args, 'verify')
  if (repo === undefined && commit === undefined && verify === undefined) {
    return undefined
  }
  if (repo === undefined || commit === undefined || verify === undefined) {
    throw new KhalaMcpToolError(
      'repo, commit, and verify are required together for workspace-backed spawn requests',
    )
  }
  const argsTokens = verificationArgs(verify)
  const commandRefSegment = publicRefSegment(argsTokens.join('.')).slice(0, 96)
  return {
    kind: 'git_checkout',
    repository: {
      branch: cleanBranch(optionalString(args, 'branch')),
      commitSha: cleanCommitSha(commit),
      fullName: cleanGithubFullName(repo),
      provider: 'github',
      visibility: 'public',
    },
    verificationCommand: {
      args: argsTokens,
      commandRef: `command.public.khala_mcp.verify.${commandRefSegment}`,
    },
  }
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
  codingPatch: Record<string, unknown> = {},
): Record<string, unknown> => ({
  messages: [{ content: prompt, role: 'user' }],
  model: 'openagents/khala',
  openagents: {
    coding: {
      ...codingPatch,
      ...(targetPylonRef === undefined ? {} : { targetPylonRef }),
    },
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

const assignmentsForRegistrations = async (
  pylonStore: PylonApiStore,
  registrations: ReadonlyArray<PylonApiRegistrationRecord>,
): Promise<ReadonlyArray<PylonApiAssignmentRecord>> => {
  const pylonRefs = registrations.map(registration => registration.pylonRef)
  if (pylonRefs.length === 0) {
    return []
  }
  if (pylonStore.listAssignmentsForPylons !== undefined) {
    return pylonStore.listAssignmentsForPylons(pylonRefs, 200)
  }
  return (
    await Promise.all(
      pylonRefs.map(pylonRef =>
        pylonStore.listAssignmentsForPylon(pylonRef, 100),
      ),
    )
  ).flat()
}

const durableRequestIdFromAssignment = (
  assignment: PylonApiAssignmentRecord,
): string | null => {
  const prefix = 'request.public.khala_coding.'
  const ref = assignment.taskRefs.find(taskRef => taskRef.startsWith(prefix))
  return ref === undefined ? null : ref.slice(prefix.length)
}

const codexCapacityForRegistration = (
  registration: PylonApiRegistrationRecord,
): Readonly<{
  available: number
  busy: number
  queued: number
  ready: number
}> =>
  pylonCodingServiceCapacityProjection(registration).find(
    capacity => capacity.service === 'codex',
  ) ?? { available: 0, busy: 0, queued: 0, ready: 0 }

const spawnCapacityProjection = (
  registrations: ReadonlyArray<PylonApiRegistrationRecord>,
): Readonly<{
  advertisedAvailableCount: number
  readyCount: number
  pylons: ReadonlyArray<Record<string, unknown>>
}> => {
  const pylons = registrations.map(registration => {
    const capacity = codexCapacityForRegistration(registration)
    return {
      codingCapacity: {
        available: capacity.available,
        busy: capacity.busy,
        queued: capacity.queued,
        ready: capacity.ready,
        service: 'codex',
      },
      latestHeartbeatAt: registration.latestHeartbeatAt,
      latestHeartbeatStatus: registration.latestHeartbeatStatus,
      pylonRef: registration.pylonRef,
      status: registration.status,
    }
  })

  return {
    advertisedAvailableCount: pylons.reduce(
      (sum, pylon) =>
        sum +
        ((pylon.codingCapacity as { available: number }).available ?? 0),
      0,
    ),
    pylons,
    readyCount: pylons.reduce(
      (sum, pylon) =>
        sum + ((pylon.codingCapacity as { ready: number }).ready ?? 0),
      0,
    ),
  }
}

const spawnChildStatusProjection = (
  assignment: PylonApiAssignmentRecord,
): Record<string, unknown> => ({
  acceptedWorkRefCount: assignment.acceptedWorkRefs.length,
  artifactRefCount: assignment.artifactRefs.length,
  assignmentRef: assignment.assignmentRef,
  closeoutRefCount: assignment.closeoutRefs.length,
  durableRequestId: durableRequestIdFromAssignment(assignment),
  leaseExpiresAt: assignment.leaseExpiresAt,
  proofRefCount: assignment.proofRefs.length,
  pylonRef: assignment.pylonRef,
  rejectionRefCount: assignment.rejectionRefs.length,
  state: assignment.state,
  updatedAt: assignment.updatedAt,
})

const spawnStateFromChildren = (
  assignments: ReadonlyArray<PylonApiAssignmentRecord>,
): 'active' | 'accepted' | 'rejected' => {
  if (assignments.some(assignment => assignment.state === 'rejected')) {
    return 'rejected'
  }
  if (
    assignments.length > 0 &&
    assignments.every(assignment => assignment.state === 'accepted')
  ) {
    return 'accepted'
  }
  return 'active'
}

const spawnStatusAggregate = (
  assignments: ReadonlyArray<PylonApiAssignmentRecord>,
): Record<string, number> =>
  assignments.reduce(
    (aggregate, assignment) => ({
      acceptedCount:
        aggregate.acceptedCount + (assignment.state === 'accepted' ? 1 : 0),
      acceptedWorkRefCount:
        aggregate.acceptedWorkRefCount + assignment.acceptedWorkRefs.length,
      activeCount:
        aggregate.activeCount +
        (assignment.state !== 'accepted' && assignment.state !== 'rejected'
          ? 1
          : 0),
      artifactRefCount: aggregate.artifactRefCount + assignment.artifactRefs.length,
      closeoutRefCount: aggregate.closeoutRefCount + assignment.closeoutRefs.length,
      proofRefCount: aggregate.proofRefCount + assignment.proofRefs.length,
      rejectedCount:
        aggregate.rejectedCount + (assignment.state === 'rejected' ? 1 : 0),
      rejectionRefCount:
        aggregate.rejectionRefCount + assignment.rejectionRefs.length,
    }),
    {
      acceptedCount: 0,
      acceptedWorkRefCount: 0,
      activeCount: 0,
      artifactRefCount: 0,
      closeoutRefCount: 0,
      proofRefCount: 0,
      rejectedCount: 0,
      rejectionRefCount: 0,
    },
  )

const spawnStatusProjection = async (input: Readonly<{
  agentStore: AgentRegistrationStore
  principal: McpPrincipal
  pylonStore: PylonApiStore
  spawnRef: string
}>): Promise<Record<string, unknown>> => {
  if (!spawnRefPattern.test(input.spawnRef)) {
    throw new KhalaMcpToolError('spawnRef must be a bounded Khala spawn ref')
  }
  const linkedAgents = await linkedAgentsForPrincipal(
    input.agentStore,
    input.principal,
  )
  const registrations = await linkedRegistrations(input.pylonStore, linkedAgents)
  const ownerAgentUserIds = new Set(linkedAgents.map(agent => agent.agentUserId))
  const pylonRefs = new Set(
    registrations.map(registration => registration.pylonRef),
  )
  const children = (await assignmentsForRegistrations(
    input.pylonStore,
    registrations,
  ))
    .filter(
      assignment =>
        ownerAgentUserIds.has(assignment.ownerAgentUserId) &&
        pylonRefs.has(assignment.pylonRef) &&
        assignment.taskRefs.includes(input.spawnRef),
    )
    .sort((left, right) => left.createdAt.localeCompare(right.createdAt))

  if (children.length === 0) {
    throw new KhalaMcpToolError('spawn_not_found_or_not_authorized')
  }

  return {
    aggregate: spawnStatusAggregate(children),
    childCount: children.length,
    children: children.map(spawnChildStatusProjection),
    ok: true,
    schema: 'openagents.khala_mcp.spawn_status.v1',
    spawnRef: input.spawnRef,
    state: spawnStateFromChildren(children),
  }
}

export const khalaDurableRequestIsLinkedToPrincipal = async (
  input: Readonly<{
    agentStore: AgentRegistrationStore
    durableRequestId: string
    principal: McpPrincipal
    pylonStore: PylonApiStore
  }>,
): Promise<boolean> => {
  const linkedAgents = await linkedAgentsForPrincipal(
    input.agentStore,
    input.principal,
  )
  const registrations = await linkedRegistrations(input.pylonStore, linkedAgents)
  const ownerAgentUserIds = new Set(linkedAgents.map(agent => agent.agentUserId))
  const pylonRefs = new Set(
    registrations.map(registration => registration.pylonRef),
  )
  const requestRef = khalaCodingRequestIdRef(input.durableRequestId)
  const assignments = await assignmentsForRegistrations(
    input.pylonStore,
    registrations,
  )
  return assignments.some(
    assignment =>
      ownerAgentUserIds.has(assignment.ownerAgentUserId) &&
      pylonRefs.has(assignment.pylonRef) &&
      assignment.taskRefs.includes(requestRef),
  )
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
        // #6325: MCP delegation must NOT meter a handoff estimate at request
        // time. The local Pylon/Codex executor records the exact downstream SDK
        // turn usage through the registered-agent turns ingest route after Codex
        // actually runs, so `deps.recordTokensServed` is intentionally not
        // invoked here.
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

      if (name === 'khala.spawn') {
        const count = boundedInteger(args, 'count', null)
        const maxParallel =
          args.maxParallel === undefined || args.maxParallel === null
            ? count
            : boundedInteger(args, 'maxParallel', count)
        const objective = spawnObjectiveFromArgs(args)
        const workflowClass = workflowFromArgs(args)
        const targetPylonRef = targetPylonRefFromArgs(args)
        const workspace = buildSpawnWorkspace(args)
        const linkedAgents = await linkedAgentsForPrincipal(
          deps.agentStore(env),
          principal,
        )
        const pylonStore = deps.pylonStore(env)
        const registrations = await linkedRegistrations(pylonStore, linkedAgents)
        const targetRegistrations =
          targetPylonRef === undefined
            ? registrations
            : registrations.filter(
                registration => registration.pylonRef === targetPylonRef,
              )
        if (targetPylonRef !== undefined && targetRegistrations.length === 0) {
          return toolErrorOutcome(name, 'target_pylon_not_authorized', {
            reason:
              'The requested Pylon is not linked to this OpenAuth account and cannot be used for caller-owned Khala spawn capacity.',
            requestedPylonRef: targetPylonRef,
            statusCode: 403,
          })
        }

        const capacity = spawnCapacityProjection(targetRegistrations)
        const makeId = deps.makeId ?? (() => compactRandomId('chatcmpl'))
        const spawnRef = spawnRefFromId(makeId())
        const dispatchCount = Math.min(
          count,
          maxParallel,
          capacity.advertisedAvailableCount,
        )
        const earlyBlockerRefs = [
          ...(linkedAgents.length === 0
            ? ['blocker.khala_mcp.spawn.no_linked_agents']
            : []),
          ...(capacity.advertisedAvailableCount === 0
            ? ['blocker.khala_mcp.spawn.no_advertised_codex_availability']
            : []),
          ...(maxParallel < count
            ? ['blocker.khala_mcp.spawn.max_parallel_limited']
            : []),
          ...(dispatchCount < count
            ? ['blocker.khala_mcp.spawn.capacity_shortfall']
            : []),
        ]

        if (dispatchCount === 0) {
          return toolErrorOutcome(
            name,
            targetPylonRef === undefined
              ? 'linked_pylon_capacity_unavailable'
              : 'target_pylon_unavailable',
            {
              blockerRefs: earlyBlockerRefs,
              capacity,
              requestedCount: count,
              requestedPylonRef: targetPylonRef,
              schema: 'openagents.khala_mcp.spawn.v1',
              spawnRef,
              statusCode: targetPylonRef === undefined ? 503 : 409,
            },
          )
        }

        const childResults: Array<Record<string, unknown>> = []
        const blockerRefs = [...earlyBlockerRefs]
        const nowIso = (deps.nowIso ?? currentIsoTimestamp)()
        for (let index = 0; index < dispatchCount; index += 1) {
          const durableRequestId = makeId()
          const childObjective = workerObjective(objective, index, count)
          const childWorkerRef = workerRef(spawnRef, index)
          const rawBody = rawBodyForRequest(
            childObjective,
            workflowClass,
            targetPylonRef,
            {
              objectiveSummary: childObjective,
              spawnRunRef: spawnRef,
              spawnWorkerRef: childWorkerRef,
              ...(workspace === undefined ? {} : { workspace }),
            },
          )
          const delegation = await delegateCodingWorkflow({
            classification: classificationForWorkflow(workflowClass),
            linkedAgents,
            makeId,
            nowIso,
            pylonStore,
            rawBody,
            requestId: durableRequestId,
          })

          if (delegation?.kind === 'rejected') {
            blockerRefs.push(
              `blocker.khala_mcp.spawn.worker_${index + 1}_rejected`,
              `blocker.khala_mcp.spawn.${delegation.error}`,
            )
            childResults.push({
              durableRequestId,
              error: delegation.error,
              evidenceRefs: delegation.evidenceRefs,
              ok: false,
              reason: delegation.reason,
              requestedPylonRef: delegation.requestedPylonRef,
              slotIndex: index,
              statusCode: delegation.statusCode,
              workerRef: childWorkerRef,
            })
            continue
          }

          if (delegation === null) {
            blockerRefs.push(
              `blocker.khala_mcp.spawn.worker_${index + 1}_capacity_unavailable`,
            )
            childResults.push({
              durableRequestId,
              error: 'linked_pylon_capacity_unavailable',
              ok: false,
              slotIndex: index,
              workerRef: childWorkerRef,
            })
            continue
          }

          childResults.push({
            assignmentRef: delegation.assignment.assignmentRef,
            durableRequestId,
            durableStreamUrl: delegation.durableStreamUrl,
            ok: true,
            pylonRef: delegation.pylon.pylonRef,
            slotIndex: index,
            state: delegation.assignment.state,
            workerRef: childWorkerRef,
          })
        }

        const assignedCount = childResults.filter(
          child => child.ok === true,
        ).length
        return projectToolOutcome(
          name,
          {
            assignedCount,
            blockerRefs: [...new Set(blockerRefs)].sort(),
            capacity,
            children: childResults,
            maxParallel,
            ok: blockerRefs.length === 0 && assignedCount === count,
            requestedCount: count,
            schema: 'openagents.khala_mcp.spawn.v1',
            spawnRef,
            stream: true,
            workflow: workflowClass,
          },
          assignedCount === 0,
        )
      }

      if (name === 'khala.spawnStatus') {
        const spawnRef = requiredString(args, 'spawnRef')
        try {
          return projectToolOutcome(
            name,
            await spawnStatusProjection({
              agentStore: deps.agentStore(env),
              principal,
              pylonStore: deps.pylonStore(env),
              spawnRef,
            }),
          )
        } catch (error) {
          if (
            error instanceof Error &&
            error.message === 'spawn_not_found_or_not_authorized'
          ) {
            return toolErrorOutcome(name, 'spawn_not_found_or_not_authorized', {
              reason:
                'The Khala spawn ref is not attached to caller-owned linked Pylon assignments.',
              spawnRef,
              statusCode: 403,
            })
          }
          throw error
        }
      }

      if (name === 'khala.resume' || name === 'khala.status') {
        const durableRequestId = requiredString(args, 'durableRequestId')
        const authorized = await khalaDurableRequestIsLinkedToPrincipal({
          agentStore: deps.agentStore(env),
          durableRequestId,
          principal,
          pylonStore: deps.pylonStore(env),
        })
        if (!authorized) {
          return toolErrorOutcome(name, 'durable_request_not_authorized', {
            durableRequestId,
            reason:
              'The durable Khala stream is not attached to a caller-owned linked Pylon assignment.',
            statusCode: 403,
          })
        }
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
