import {
  decodeAgentDefinition,
  type AgentDefinition,
} from '@openagentsinc/agent-runtime-schema'

// Behavior contract oracle: background_agents.dispatch.budget_caps_enforced.v1
import {
  MemoryStreamStore,
  type StreamStore,
  handleRequest,
  streamIdFromUrl,
} from '@openagentsinc/durable-stream'
import {
  decodeForgeCoordinationIssueRow,
  type ForgeCoordinationIssueRow,
  type ForgeGitAccessTokenRow,
  type ForgeGitAccessTokenScopeRow,
  type ForgeTenantRow,
} from '@openagentsinc/forge-protocol'
import { describe, expect, test } from 'vitest'

import { type AgentDefinitionStore } from './agent-definition-routes'
import {
  type AgentDefinitionRunDailyBudgetUsage,
  type AgentDefinitionRunRecord,
  type AgentDefinitionRunStore,
  handleAgentDefinitionRunRequest,
  revokeAgentDefinitionRunForgeGitTokensForAssignment,
} from './agent-definition-run-routes'
import {
  type AgentCredentialLookup,
  type AgentCredentialRecord,
  type AgentRegistrationRecord,
  type AgentRegistrationStore,
  type AgentReissueSelector,
  type AgentReissueTarget,
  sha256Hex,
} from './agent-registration'
import { type ForgeCoordinationStore } from './forge-coordination-store'
import { type ForgeTenantGitAuthStore } from './forge-tenant-git-auth-store'
import { type DurableStreamNamespace } from './inference/durable-inference-do-transport'
import { routeDurableInferenceReadRequestDO } from './inference/durable-inference-read-routes'
import {
  type PylonApiAssignmentRecord,
  type PylonApiQuarantineRecord,
  type PylonApiRegistrationRecord,
  type PylonApiStore,
} from './pylon-api'

const nowIso = '2026-07-03T00:00:00.000Z'

class MemoryAgentRegistrationStore implements AgentRegistrationStore {
  readonly touchedCredentialIds: Array<string> = []

  constructor(
    private readonly lookupsByTokenHash: ReadonlyMap<
      string,
      AgentCredentialLookup
    >,
  ) {}

  createAgentRegistration(_record: AgentRegistrationRecord): Promise<void> {
    return Promise.resolve()
  }

  findAgentByTokenHash(
    tokenHash: string,
    _now: string,
  ): Promise<AgentCredentialLookup | undefined> {
    return Promise.resolve(this.lookupsByTokenHash.get(tokenHash))
  }

  touchAgentCredential(
    credentialId: string,
    _lastUsedAt: string,
  ): Promise<void> {
    this.touchedCredentialIds.push(credentialId)

    return Promise.resolve()
  }

  updateAgentDisplayName(): Promise<number> {
    return Promise.resolve(0)
  }

  findAgentForReissue(
    _selector: AgentReissueSelector,
  ): Promise<AgentReissueTarget | undefined> {
    return Promise.resolve(undefined)
  }

  addAgentCredential(_record: AgentCredentialRecord): Promise<void> {
    return Promise.resolve()
  }
}

class MemoryAgentDefinitionStore implements Pick<
  AgentDefinitionStore,
  'readDefinition'
> {
  readonly rows: Array<{
    readonly ownerAgentUserId: string
    readonly definition: AgentDefinition
  }> = []

  addDefinition(ownerAgentUserId: string, definition: AgentDefinition): void {
    this.rows.push({ ownerAgentUserId, definition })
  }

  readDefinition(
    ownerAgentUserId: string,
    definitionId: string,
  ): Promise<AgentDefinition | undefined> {
    return Promise.resolve(
      this.rows.find(
        row =>
          row.ownerAgentUserId === ownerAgentUserId &&
          row.definition.id === definitionId,
      )?.definition,
    )
  }
}

class MemoryAgentDefinitionRunStore implements AgentDefinitionRunStore {
  readonly rows: Array<AgentDefinitionRunRecord> = []
  private dailyBudgetUsageOverride:
    | AgentDefinitionRunDailyBudgetUsage
    | undefined

  listRunsForDefinition(
    ownerAgentUserId: string,
    definitionId: string,
    limit: number,
  ): Promise<ReadonlyArray<AgentDefinitionRunRecord>> {
    return Promise.resolve(
      this.rows
        .filter(
          row =>
            row.ownerAgentUserId === ownerAgentUserId &&
            row.definitionId === definitionId,
        )
        .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))
        .slice(0, limit),
    )
  }

  upsertRun(
    record: AgentDefinitionRunRecord,
  ): Promise<AgentDefinitionRunRecord> {
    const index = this.rows.findIndex(row => row.runId === record.runId)

    if (index === -1) {
      this.rows.push(record)
    } else {
      this.rows[index] = record
    }

    return Promise.resolve(record)
  }

  readRun(
    ownerAgentUserId: string,
    runId: string,
  ): Promise<AgentDefinitionRunRecord | undefined> {
    return Promise.resolve(
      this.rows.find(
        row => row.ownerAgentUserId === ownerAgentUserId && row.runId === runId,
      ),
    )
  }

  readRunByAssignmentRef(
    assignmentRef: string,
  ): Promise<AgentDefinitionRunRecord | undefined> {
    return Promise.resolve(
      this.rows.find(row => row.assignmentRef === assignmentRef),
    )
  }

  readDailyBudgetUsage(
    ownerAgentUserId: string,
    definitionId: string,
    dayStartIso: string,
    dayEndIso: string,
  ): Promise<AgentDefinitionRunDailyBudgetUsage> {
    if (this.dailyBudgetUsageOverride !== undefined) {
      return Promise.resolve(this.dailyBudgetUsageOverride)
    }

    const rows = this.rows.filter(
      row =>
        row.ownerAgentUserId === ownerAgentUserId &&
        row.definitionId === definitionId &&
        row.createdAt >= dayStartIso &&
        row.createdAt < dayEndIso,
    )

    return Promise.resolve({
      creditsReserved: rows.reduce(
        (total, row) => total + row.budgetCreditsReserved,
        0,
      ),
      runCount: rows.length,
    })
  }

  setDailyBudgetUsage(usage: AgentDefinitionRunDailyBudgetUsage): void {
    this.dailyBudgetUsageOverride = usage
  }
}

const lookup = (input: {
  readonly credentialId: string
  readonly tokenPrefix: string
  readonly userId: string
}): AgentCredentialLookup => ({
  user: {
    id: input.userId,
    kind: 'agent',
    displayName: `Agent ${input.userId}`,
    primaryEmail: null,
    avatarUrl: null,
    status: 'active',
    createdAt: nowIso,
    updatedAt: nowIso,
  },
  credentialId: input.credentialId,
  openauthUserId: null,
  profileMetadataJson: '{}',
  tokenPrefix: input.tokenPrefix,
})

const makeAgentStore = async () => {
  const ownerToken = 'oa_agent_owner_route_test'
  const otherToken = 'oa_agent_other_route_test'
  const store = new MemoryAgentRegistrationStore(
    new Map([
      [
        await sha256Hex(ownerToken),
        lookup({
          credentialId: 'credential_owner',
          tokenPrefix: 'oa_agent_owner',
          userId: 'agent_user_owner',
        }),
      ],
      [
        await sha256Hex(otherToken),
        lookup({
          credentialId: 'credential_other',
          tokenPrefix: 'oa_agent_other',
          userId: 'agent_user_other',
        }),
      ],
    ]),
  )

  return { otherToken, ownerToken, store }
}

const definition = (
  overrides: Partial<AgentDefinition> = {},
): AgentDefinition =>
  decodeAgentDefinition({
    schema: 'openagents.agent_definition.v1',
    id: 'agent_definition.route_test.owner',
    ownerRef: 'agent:agent_user_owner',
    name: 'Dispatch Route Definition',
    slug: 'dispatch-route-definition',
    goal: 'Run a bounded background task through caller-owned Pylon capacity.',
    harness: {
      kind: 'codex',
      modelHint: 'openagents/pylon-codex',
    },
    toolset: {
      allow: [
        'tool.openagents.issue.read',
        'tool.openagents.forge.git.receive_pack',
      ],
      deny: ['tool.openagents.payment.*'],
      ask: ['tool.openagents.github.comment'],
      networkPolicy: 'owner_scoped',
      secretPolicy: 'owner_scoped_refs_only',
    },
    triggers: [
      {
        kind: 'manual',
        triggerRef: 'trigger.public.route_test.manual',
      },
    ],
    lane: 'own_pylon',
    budget: {
      maxRunSeconds: 900,
      maxRunsPerDay: 3,
      maxCreditsPerDay: 0,
    },
    escalation: {
      channel: 'operator',
      askPolicy: {
        policyRef: 'policy.public.agent_definition.operator_required.v1',
        mode: 'operator_required',
      },
    },
    sourceRefs: ['issue.public.github.OpenAgentsInc.openagents.8189'],
    createdAt: nowIso,
    updatedAt: nowIso,
    ...overrides,
  })

const jsonRequest = (
  input: Readonly<{
    body?: unknown
    method?: string
    path: string
    token?: string | undefined
  }>,
): Request =>
  new Request(`https://openagents.com${input.path}`, {
    ...(input.body === undefined ? {} : { body: JSON.stringify(input.body) }),
    headers: {
      ...(input.body === undefined
        ? {}
        : { 'content-type': 'application/json' }),
      ...(input.token === undefined
        ? {}
        : { authorization: `Bearer ${input.token}` }),
    },
    method: input.method ?? 'POST',
  })

const registration = (
  overrides: Partial<PylonApiRegistrationRecord> = {},
): PylonApiRegistrationRecord => ({
  capabilityRefs: ['capability.pylon.local_codex'],
  clientProtocolVersion: '0.3.0',
  clientVersion: '0.3.0',
  createdAt: nowIso,
  displayName: 'Linked Codex Pylon',
  id: 'pylon_api_registration_1',
  latestCapacityRefs: [
    'capacity.coding.codex.ready=1',
    'capacity.coding.codex.available=1',
  ],
  latestHeartbeatAt: nowIso,
  latestHeartbeatStatus: 'online',
  latestHealthRefs: ['health.public.pylon_cli.ok'],
  latestLoadRefs: ['load.coding.codex.busy=0', 'load.coding.codex.queued=0'],
  latestResourceMode: 'background_20',
  ownerAgentCredentialId: 'credential_owner',
  ownerAgentTokenPrefix: 'oa_agent_owner',
  ownerAgentUserId: 'agent_user_owner',
  providerMarketRelayRefs: [],
  providerNip90LaneRefs: [],
  providerNostrNpub: null,
  providerNostrPubkey: null,
  publicProjectionJson: '{}',
  pylonRef: 'pylon.owner.codex',
  resourceMode: 'background_20',
  status: 'active',
  updatedAt: nowIso,
  walletReady: true,
  walletRef: null,
  ...overrides,
})

const makePylonStore = (input: {
  readonly registrations: ReadonlyArray<PylonApiRegistrationRecord>
}) => {
  const assignments: Array<PylonApiAssignmentRecord> = []

  const store: PylonApiStore = {
    createAssignment: async record => {
      assignments.push(record)

      return { idempotent: false, record }
    },
    createEvent: async () => Promise.reject(new Error('not used')),
    listAssignmentsForPylon: async pylonRef =>
      assignments.filter(item => item.pylonRef === pylonRef),
    listEventsForAssignment: async () => [],
    listEventsForPylon: async () => [],
    listProviderJobLifecycleForPylons: async () => [],
    listRegistrations: async () => input.registrations,
    listRegistrationsForOwnerAgentUserIds: async ownerAgentUserIds =>
      input.registrations.filter(item =>
        ownerAgentUserIds.includes(item.ownerAgentUserId),
      ),
    readActiveQuarantineForPylon: async () => undefined,
    readAssignment: async assignmentRef =>
      assignments.find(item => item.assignmentRef === assignmentRef),
    readAssignmentByIdempotencyKeyHash: async idempotencyKeyHash =>
      assignments.find(item => item.idempotencyKeyHash === idempotencyKeyHash),
    readEventByIdempotencyKeyHash: async () => undefined,
    readRegistration: async pylonRef =>
      input.registrations.find(item => item.pylonRef === pylonRef),
    sweepStaleAssignmentLeases: async () => [],
    updateAssignment: async record => record,
    updateAssignmentIfState: async (record, expectedState) => {
      const index = assignments.findIndex(
        item => item.assignmentRef === record.assignmentRef,
      )

      if (index < 0 || assignments[index]?.state !== expectedState) {
        return undefined
      }

      assignments[index] = record

      return record
    },
    upsertProviderJobLifecycle: async record => record,
    upsertQuarantine: async (
      record: PylonApiQuarantineRecord,
    ): Promise<PylonApiQuarantineRecord> => record,
    upsertRegistration: async record => record,
  }

  return { assignments, store }
}

const makeForgeStore = () => {
  const issues: Array<ForgeCoordinationIssueRow> = []

  const store: ForgeCoordinationStore = {
    upsertIssue: async input => {
      const row = decodeForgeCoordinationIssueRow({
        tenant_ref: input.tenantRef,
        issue_ref: input.issueRef,
        github_issue_number: input.githubIssueNumber ?? null,
        title: input.title,
        state: input.state,
        priority_ref: input.priorityRef ?? null,
        source_refs_json: JSON.stringify(input.sourceRefs),
        git_token_refs_json: JSON.stringify(input.gitTokenRefs ?? []),
        created_at: input.nowIso,
        updated_at: input.nowIso,
      })
      const index = issues.findIndex(
        item =>
          item.tenant_ref === row.tenant_ref &&
          item.issue_ref === row.issue_ref,
      )

      if (index === -1) {
        issues.push(row)
      } else {
        issues[index] = row
      }

      return row
    },
    listIssues: async (tenantRef, limit) =>
      issues.filter(item => item.tenant_ref === tenantRef).slice(0, limit),
    upsertChange: async () => Promise.reject(new Error('not used')),
    listChanges: async () => [],
    recordStatus: async () => Promise.reject(new Error('not used')),
    listStatuses: async () => [],
    acquireDispatchLease: async () => Promise.reject(new Error('not used')),
    listDispatchLeases: async () => [],
    readActiveDispatchLease: async () => undefined,
    recordMergeQueueLedger: async () => Promise.reject(new Error('not used')),
    listMergeQueueLedgers: async () => [],
    readLatestMergeQueueLedger: async () => undefined,
    recordVerificationReceipt: async receipt => receipt,
    listVerificationReceipts: async () => [],
    recordPromotionDecisionReceipt: async receipt => receipt,
    listPromotionDecisionReceipts: async () => [],
  }

  return { issues, store }
}

class MemoryForgeTenantGitAuthStore implements ForgeTenantGitAuthStore {
  readonly scopes: Array<ForgeGitAccessTokenScopeRow> = []
  readonly tenants: Array<ForgeTenantRow> = []
  readonly tokens: Array<ForgeGitAccessTokenRow> = []

  upsertTenant(
    input: Parameters<ForgeTenantGitAuthStore['upsertTenant']>[0],
  ): Promise<ForgeTenantRow> {
    const existing = this.tenants.findIndex(
      row => row.tenant_ref === input.tenantRef,
    )
    const row: ForgeTenantRow = {
      tenant_ref: input.tenantRef,
      display_name: input.displayName,
      state: input.state ?? 'active',
      confidential_workspace_mode: input.confidentialWorkspaceMode ?? null,
      attestation_ref: input.attestationRef ?? null,
      encrypted_knowledge_pack_ref: input.encryptedKnowledgePackRef ?? null,
      refusal_reason: input.refusalReason ?? null,
      retention_policy_ref: input.retentionPolicyRef ?? null,
      created_at: this.tenants[existing]?.created_at ?? input.nowIso,
      updated_at: input.nowIso,
    }

    if (existing === -1) {
      this.tenants.push(row)
    } else {
      this.tenants[existing] = row
    }

    return Promise.resolve(row)
  }

  mintGitAccessToken(
    input: Parameters<ForgeTenantGitAuthStore['mintGitAccessToken']>[0],
    options?: Parameters<ForgeTenantGitAuthStore['mintGitAccessToken']>[1],
  ): Promise<
    Awaited<ReturnType<ForgeTenantGitAuthStore['mintGitAccessToken']>>
  > {
    const token =
      options?.makeToken?.() ??
      `oa_forge_git_${input.tokenRef.replace(/[^A-Za-z0-9]/g, '').padEnd(32, '0')}`
    const record: ForgeGitAccessTokenRow = {
      tenant_ref: input.tenantRef,
      token_ref: input.tokenRef,
      subject_ref: input.subjectRef,
      repository_ref: input.repositoryRef,
      token_hash: `sha256.${input.tokenRef}`,
      token_prefix: token.slice(0, 'oa_forge_git_'.length + 16),
      state: 'active',
      created_at: input.nowIso,
      expires_at: input.expiresAt,
      last_used_at: null,
      revoked_at: null,
      source_refs_json: JSON.stringify(input.sourceRefs),
      ref_restrictions_json: JSON.stringify(input.refRestrictions ?? []),
    }
    const scopeRows = input.scopes.map(
      (scope): ForgeGitAccessTokenScopeRow => ({
        tenant_ref: input.tenantRef,
        token_ref: input.tokenRef,
        scope,
        created_at: input.nowIso,
      }),
    )

    this.tokens.push(record)
    this.scopes.push(...scopeRows)

    return Promise.resolve({ record, scopes: scopeRows, token })
  }

  authenticateGitAccessToken(): Promise<undefined> {
    return Promise.resolve(undefined)
  }

  revokeGitAccessToken(
    tenantRef: string,
    tokenRef: string,
    revokedAt: string,
  ): Promise<ForgeGitAccessTokenRow | undefined> {
    const index = this.tokens.findIndex(
      row => row.tenant_ref === tenantRef && row.token_ref === tokenRef,
    )
    if (index === -1) {
      return Promise.resolve(undefined)
    }

    const row = {
      ...this.tokens[index]!,
      state: 'revoked' as const,
      revoked_at: revokedAt,
    }
    this.tokens[index] = row

    return Promise.resolve(row)
  }

  readGitAccessToken(
    tenantRef: string,
    tokenRef: string,
  ): Promise<ForgeGitAccessTokenRow | undefined> {
    return Promise.resolve(
      this.tokens.find(
        row => row.tenant_ref === tenantRef && row.token_ref === tokenRef,
      ),
    )
  }
}

class StreamRegistry {
  private readonly stores = new Map<string, StreamStore>()

  private storeFor(streamId: string): StreamStore {
    let store = this.stores.get(streamId)

    if (store === undefined) {
      store = new MemoryStreamStore()
      this.stores.set(streamId, store)
    }

    return store
  }

  fetch(request: Request): Promise<Response> {
    const streamId = streamIdFromUrl(request.url)

    if (streamId === null) {
      return Promise.resolve(new Response('not found', { status: 404 }))
    }

    return handleRequest(this.storeFor(streamId), request, { streamId })
  }
}

const durableNamespace = (
  registry: StreamRegistry = new StreamRegistry(),
): DurableStreamNamespace => ({
  getByName: () => ({ fetch: (request: Request) => registry.fetch(request) }),
})

const makeDependencies = async (
  input: Readonly<{
    definition?: AgentDefinition
    definitionOwnerAgentUserId?: string
    ids?: ReadonlyArray<string>
    pylonRegistrations?: ReadonlyArray<PylonApiRegistrationRecord>
  }> = {},
) => {
  const agent = await makeAgentStore()
  const definitionStore = new MemoryAgentDefinitionStore()

  definitionStore.addDefinition(
    input.definitionOwnerAgentUserId ?? 'agent_user_owner',
    input.definition ?? definition(),
  )

  const runStore = new MemoryAgentDefinitionRunStore()
  const forge = makeForgeStore()
  const forgeGitAuthStore = new MemoryForgeTenantGitAuthStore()
  const pylon = makePylonStore({
    registrations: input.pylonRegistrations ?? [registration()],
  })
  const ids = [...(input.ids ?? ['run_001', 'assignment_001'])]
  const namespace = durableNamespace()

  return {
    agent,
    dependencies: {
      agentStore: agent.store,
      definitionStore,
      durableStreamNamespace: namespace,
      forgeGitAuthStore,
      forgeStore: forge.store,
      makeId: () => ids.shift() ?? 'extra_id',
      nowIso: () => nowIso,
      pylonStore: pylon.store,
      runStore,
    },
    forge,
    forgeGitAuthStore,
    namespace,
    pylon,
    runStore,
  }
}

type RunProjection = Readonly<{
  assignmentRef: string | null
  createdAt: string
  durableStreamUrl: string | null
  evidenceRefs: ReadonlyArray<string>
  exactAccounting: null | Readonly<{
    demandSource: string
    settlesOn: string
    taskRef: string
    usageTruth: string
  }>
  forge: Readonly<{
    gitTokenRefs: ReadonlyArray<string>
    repositoryRef: string | null
    tenantRef: string
    workRef: string
  }>
  lane: string
  pylonRef: string | null
  receiptRefs: ReadonlyArray<string>
  runId: string
  sessionEventStream: Readonly<{
    durableRequestId: string
    durableStreamUrl: string | null
    eventCount: number
    seeded: boolean
  }>
  status: string
  triggerRef: string
  updatedAt: string
}>

const readRunResponse = async (
  response: Response,
): Promise<{ run: RunProjection }> =>
  (await response.json()) as { run: RunProjection }

describe('agent definition run routes', () => {
  test('requires a registered agent bearer token', async () => {
    const { dependencies } = await makeDependencies()
    const response = await handleAgentDefinitionRunRequest(
      jsonRequest({
        path: '/v1/agent-definitions/agent_definition.route_test.owner/runs',
      }),
      dependencies,
    )

    expect(response?.status).toBe(401)
  })

  test('dispatches an own_pylon definition with a scoped Forge git token', async () => {
    const {
      agent,
      dependencies,
      forge,
      forgeGitAuthStore,
      namespace,
      pylon,
      runStore,
    } = await makeDependencies()
    const response = await handleAgentDefinitionRunRequest(
      jsonRequest({
        body: {
          triggerRef: 'trigger.public.route_test.manual',
          triggerPayload: {
            source: 'manual',
          },
          objectiveSummary:
            'Run the bounded background agent route test through owner Pylon capacity.',
          targetPylonRef: 'pylon.owner.codex',
          workspace: {
            kind: 'git_checkout',
            repository: {
              branch: 'main',
              commitSha: '8e95094c465cafbb74748ae8bc87513997655e03',
              fullName: 'OpenAgentsInc/openagents',
              provider: 'github',
              visibility: 'public',
            },
          },
        },
        path: '/v1/agent-definitions/agent_definition.route_test.owner/runs',
        token: agent.ownerToken,
      }),
      dependencies,
    )
    const body = await readRunResponse(response!)

    expect(response?.status).toBe(201)
    expect(body.run).toMatchObject({
      assignmentRef: 'assignment.public.khala_coding.assignment_001',
      durableStreamUrl:
        '/v1/chat/completions/durable/agent_definition_run.run_001',
      lane: 'own_pylon',
      pylonRef: 'pylon.owner.codex',
      status: 'dispatched',
      triggerRef: 'trigger.public.route_test.manual',
    })
    expect(body.run.exactAccounting).toEqual({
      demandSource: 'khala_coding_delegation',
      settlesOn: 'pylon_worker_closeout',
      taskRef: 'assignment.public.khala_coding.assignment_001',
      usageTruth: 'exact',
    })
    expect(body.run.sessionEventStream).toMatchObject({
      eventCount: 2,
      seeded: true,
    })
    expect(body.run.forge).toMatchObject({
      gitTokenRefs: [
        'forge_git_token.background_agent.agent_definition_run.run_001.receive_pack',
      ],
      repositoryRef: 'repo.openagents.openagents',
    })
    expect(body.run.receiptRefs).toEqual(
      expect.arrayContaining([
        'assignment.public.khala_coding.assignment_001',
        'evidence.agent_definition_run.exact_accounting_assignment_ref',
        'evidence.agent_definition_run.forge_git_token_minted',
        'evidence.agent_definition_run.forge_work_record_registered',
        'forge_git_token.background_agent.agent_definition_run.run_001.receive_pack',
      ]),
    )
    expect(runStore.rows[0]?.run.assignmentId).toBe(
      'assignment.public.khala_coding.assignment_001',
    )
    expect(forge.issues[0]).toMatchObject({
      issue_ref: body.run.forge.workRef,
      git_token_refs_json:
        '["forge_git_token.background_agent.agent_definition_run.run_001.receive_pack"]',
      state: 'open',
      tenant_ref: body.run.forge.tenantRef,
    })
    expect(runStore.rows[0]?.forgeGitTokenRefs).toEqual([
      'forge_git_token.background_agent.agent_definition_run.run_001.receive_pack',
    ])
    expect(forgeGitAuthStore.tokens[0]).toMatchObject({
      repository_ref: 'repo.openagents.openagents',
      token_ref:
        'forge_git_token.background_agent.agent_definition_run.run_001.receive_pack',
      state: 'active',
    })
    expect(
      JSON.parse(forgeGitAuthStore.tokens[0]?.ref_restrictions_json ?? '[]'),
    ).toEqual(['refs/heads/background-agents/agent_definition_run.run_001'])
    expect(pylon.assignments[0]).toMatchObject({
      assignmentRef: 'assignment.public.khala_coding.assignment_001',
      idempotencyKeyHash: 'khala-coding:agent_definition_run.run_001',
      jobKind: 'codex_agent_task',
      ownerAgentUserId: 'agent_user_owner',
      pylonRef: 'pylon.owner.codex',
    })
    expect(pylon.assignments[0]?.taskRefs).toContain(
      'workflow.public.khala_coding.codex_agent_task',
    )
    expect(pylon.assignments[0]?.codingAssignment).toMatchObject({
      codex: {
        timeoutSeconds: 900,
      },
    })
    expect(runStore.rows[0]?.budgetCreditsReserved).toBe(0)

    const replay = await routeDurableInferenceReadRequestDO(
      new Request(
        `https://openagents.com${body.run.durableStreamUrl ?? ''}?offset=0`,
      ),
      { enabled: true, namespace },
    )
    const durableBody = await replay!.text()

    expect(replay?.status).toBe(200)
    expect(durableBody).toContain('"tag":"run.input_accepted"')
    expect(durableBody).toContain('"tag":"external_agent.started"')

    await expect(
      revokeAgentDefinitionRunForgeGitTokensForAssignment(
        {
          forgeGitAuthStore,
          runStore,
        },
        {
          assignmentRef: 'assignment.public.khala_coding.assignment_001',
          nowIso: '2026-07-03T00:20:00.000Z',
        },
      ),
    ).resolves.toEqual({
      assignmentRef: 'assignment.public.khala_coding.assignment_001',
      foundRun: true,
      revokedTokenRefs: [
        'forge_git_token.background_agent.agent_definition_run.run_001.receive_pack',
      ],
    })
    expect(forgeGitAuthStore.tokens[0]).toMatchObject({
      revoked_at: '2026-07-03T00:20:00.000Z',
      state: 'revoked',
    })
    expect(runStore.rows[0]?.evidenceRefs).toContain(
      'evidence.agent_definition_run.forge_git_tokens_revoked',
    )
  })

  test('lists owner-scoped definition run history with status, trigger, and receipt refs', async () => {
    const { agent, dependencies } = await makeDependencies()
    const dispatchResponse = await handleAgentDefinitionRunRequest(
      jsonRequest({
        body: {
          triggerRef: 'trigger.public.route_test.manual',
          targetPylonRef: 'pylon.owner.codex',
        },
        path: '/v1/agent-definitions/agent_definition.route_test.owner/runs',
        token: agent.ownerToken,
      }),
      dependencies,
    )
    expect(dispatchResponse?.status).toBe(201)

    const response = await handleAgentDefinitionRunRequest(
      jsonRequest({
        method: 'GET',
        path: '/v1/agent-definitions/agent_definition.route_test.owner/runs?limit=10',
        token: agent.ownerToken,
      }),
      dependencies,
    )
    const body = (await response!.json()) as {
      readonly count: number
      readonly definitionId: string
      readonly limit: number
      readonly runs: ReadonlyArray<RunProjection>
      readonly schema: string
    }

    expect(response?.status).toBe(200)
    expect(body).toMatchObject({
      count: 1,
      definitionId: 'agent_definition.route_test.owner',
      limit: 10,
      schema: 'openagents.agent_definition_run_list.v0.1',
    })
    expect(body.runs[0]).toMatchObject({
      status: 'dispatched',
      triggerRef: 'trigger.public.route_test.manual',
    })
    expect(body.runs[0]?.receiptRefs).toEqual(
      expect.arrayContaining([
        'assignment.public.khala_coding.assignment_001',
        'evidence.agent_definition_run.exact_accounting_assignment_ref',
        'evidence.agent_definition_run.forge_work_record_registered',
      ]),
    )
  })

  test('refuses dispatch before minting when the definition denies Forge git receive-pack', async () => {
    const { agent, dependencies, forge, forgeGitAuthStore, pylon, runStore } =
      await makeDependencies({
        definition: definition({
          toolset: {
            allow: ['tool.openagents.issue.read'],
            deny: ['tool.openagents.forge.git.receive_pack'],
            ask: [],
            networkPolicy: 'owner_scoped',
            secretPolicy: 'owner_scoped_refs_only',
          },
        }),
      })
    const response = await handleAgentDefinitionRunRequest(
      jsonRequest({
        body: {
          triggerRef: 'trigger.public.route_test.manual',
        },
        path: '/v1/agent-definitions/agent_definition.route_test.owner/runs',
        token: agent.ownerToken,
      }),
      dependencies,
    )
    const body = (await response!.json()) as {
      readonly error: string
      readonly evidenceRefs: ReadonlyArray<string>
      readonly run: RunProjection
    }

    expect(response?.status).toBe(403)
    expect(body.error).toBe('agent_definition_forge_git_scope_denied')
    expect(body.evidenceRefs).toContain(
      'evidence.agent_definition_run.forge_git_scope_policy',
    )
    expect(body.run.status).toBe('refused')
    expect(body.run.forge.gitTokenRefs).toEqual([])
    expect(runStore.rows[0]?.forgeGitTokenRefs).toEqual([])
    expect(forge.issues).toHaveLength(1)
    expect(JSON.parse(forge.issues[0]?.git_token_refs_json ?? '[]')).toEqual([])
    expect(forgeGitAuthStore.tokens).toHaveLength(0)
    expect(pylon.assignments).toHaveLength(0)
  })

  test('does not list definition run history outside the authenticated owner scope', async () => {
    const { agent, dependencies } = await makeDependencies()
    const response = await handleAgentDefinitionRunRequest(
      jsonRequest({
        method: 'GET',
        path: '/v1/agent-definitions/agent_definition.route_test.owner/runs',
        token: agent.otherToken,
      }),
      dependencies,
    )
    const body = (await response!.json()) as { readonly error: string }

    expect(response?.status).toBe(404)
    expect(body.error).toBe('agent_definition_not_found')
  })

  test('run-now dispatches through the definition manual trigger path', async () => {
    const { agent, dependencies, runStore } = await makeDependencies()
    const response = await handleAgentDefinitionRunRequest(
      jsonRequest({
        path: '/v1/agent-definitions/agent_definition.route_test.owner/run-now',
        token: agent.ownerToken,
      }),
      dependencies,
    )
    const body = await readRunResponse(response!)

    expect(response?.status).toBe(201)
    expect(body.run).toMatchObject({
      status: 'dispatched',
      triggerRef: 'trigger.public.route_test.manual',
    })
    expect(runStore.rows[0]?.triggerPayload).toMatchObject({
      manualRunNow: true,
    })
  })

  test('run-now refuses definitions without a manual trigger before dispatch', async () => {
    const { agent, dependencies, pylon, runStore } = await makeDependencies({
      definition: definition({
        triggers: [
          {
            kind: 'cron',
            triggerRef: 'trigger.public.route_test.daily',
            expr: '0 14 * * *',
            tz: 'UTC',
          },
        ],
      }),
    })

    const response = await handleAgentDefinitionRunRequest(
      jsonRequest({
        path: '/v1/agent-definitions/agent_definition.route_test.owner/run-now',
        token: agent.ownerToken,
      }),
      dependencies,
    )
    const body = (await response!.json()) as {
      readonly error: string
      readonly reason: string
    }

    expect(response?.status).toBe(400)
    expect(body.error).toBe('invalid_agent_definition_run')
    expect(body.reason).toBe(
      'definition must include a manual trigger for run-now.',
    )
    expect(pylon.assignments).toHaveLength(0)
    expect(runStore.rows).toHaveLength(0)
  })

  test('refuses dispatch after maxRunsPerDay is exhausted', async () => {
    const { agent, dependencies, forge, pylon, runStore } =
      await makeDependencies()
    runStore.setDailyBudgetUsage({ creditsReserved: 0, runCount: 3 })

    const response = await handleAgentDefinitionRunRequest(
      jsonRequest({
        body: {
          triggerRef: 'trigger.public.route_test.manual',
        },
        path: '/v1/agent-definitions/agent_definition.route_test.owner/runs',
        token: agent.ownerToken,
      }),
      dependencies,
    )
    const body = (await response!.json()) as {
      readonly error: string
      readonly evidenceRefs: ReadonlyArray<string>
      readonly run: RunProjection
    }

    expect(response?.status).toBe(429)
    expect(body.error).toBe('agent_definition_budget_runs_exhausted')
    expect(body.evidenceRefs).toContain(
      'evidence.agent_definition_run.budget.max_runs_per_day_exhausted',
    )
    expect(body.run.status).toBe('refused')
    expect(runStore.rows[0]?.status).toBe('refused')
    expect(runStore.rows[0]?.refusalError).toBe(
      'agent_definition_budget_runs_exhausted',
    )
    expect(forge.issues).toHaveLength(1)
    expect(pylon.assignments).toHaveLength(0)
  })

  test('refuses dispatch when reserved daily credits exceed maxCreditsPerDay', async () => {
    const { agent, dependencies, pylon, runStore } = await makeDependencies()
    runStore.setDailyBudgetUsage({ creditsReserved: 1, runCount: 0 })

    const response = await handleAgentDefinitionRunRequest(
      jsonRequest({
        body: {
          triggerRef: 'trigger.public.route_test.manual',
        },
        path: '/v1/agent-definitions/agent_definition.route_test.owner/runs',
        token: agent.ownerToken,
      }),
      dependencies,
    )
    const body = (await response!.json()) as {
      readonly error: string
      readonly evidenceRefs: ReadonlyArray<string>
      readonly run: RunProjection
    }

    expect(response?.status).toBe(429)
    expect(body.error).toBe('agent_definition_budget_credits_exhausted')
    expect(body.evidenceRefs).toContain(
      'evidence.agent_definition_run.budget.max_credits_per_day_exhausted',
    )
    expect(body.run.status).toBe('refused')
    expect(pylon.assignments).toHaveLength(0)
  })

  test('refuses invalid dispatch budgets before Pylon assignment', async () => {
    const { agent, dependencies, pylon, runStore } = await makeDependencies({
      definition: definition({
        budget: {
          maxCreditsPerDay: 0,
          maxRunSeconds: 0,
          maxRunsPerDay: 3,
        },
      }),
    })

    const response = await handleAgentDefinitionRunRequest(
      jsonRequest({
        body: {
          triggerRef: 'trigger.public.route_test.manual',
        },
        path: '/v1/agent-definitions/agent_definition.route_test.owner/runs',
        token: agent.ownerToken,
      }),
      dependencies,
    )
    const body = (await response!.json()) as {
      readonly error: string
      readonly run: RunProjection
    }

    expect(response?.status).toBe(400)
    expect(body.error).toBe('agent_definition_budget_invalid')
    expect(body.run.status).toBe('refused')
    expect(runStore.rows[0]?.refusalReason).toContain('maxRunSeconds')
    expect(pylon.assignments).toHaveLength(0)
  })

  test('persists a typed refusal for unsupported lanes', async () => {
    const { agent, dependencies, forge, runStore } = await makeDependencies({
      definition: definition({ lane: 'worker_only' }),
      pylonRegistrations: [],
    })
    const response = await handleAgentDefinitionRunRequest(
      jsonRequest({
        body: {
          triggerRef: 'trigger.public.route_test.manual',
          triggerPayload: {
            source: 'manual',
          },
        },
        path: '/v1/agent-definitions/agent_definition.route_test.owner/runs',
        token: agent.ownerToken,
      }),
      dependencies,
    )
    const body = (await response!.json()) as {
      readonly error: string
      readonly evidenceRefs: ReadonlyArray<string>
      readonly run: RunProjection
    }

    expect(response?.status).toBe(409)
    expect(body.error).toBe('target_pylon_unavailable')
    expect(body.evidenceRefs).toContain(
      'evidence.agent_definition_run.lane.unsupported',
    )
    expect(body.run.status).toBe('refused')
    expect(runStore.rows[0]?.status).toBe('refused')
    expect(runStore.rows[0]?.refusalError).toBe('target_pylon_unavailable')
    expect(forge.issues).toHaveLength(1)
  })

  test('persists a typed refusal when linked Pylon capacity is unavailable', async () => {
    const { agent, dependencies, pylon, runStore } = await makeDependencies({
      pylonRegistrations: [],
    })
    const response = await handleAgentDefinitionRunRequest(
      jsonRequest({
        body: {
          triggerRef: 'trigger.public.route_test.manual',
        },
        path: '/v1/agent-definitions/agent_definition.route_test.owner/runs',
        token: agent.ownerToken,
      }),
      dependencies,
    )
    const body = (await response!.json()) as {
      readonly error: string
      readonly evidenceRefs: ReadonlyArray<string>
      readonly run: RunProjection
    }

    expect(response?.status).toBe(503)
    expect(body.error).toBe('target_pylon_unavailable')
    expect(body.evidenceRefs).toContain(
      'evidence.agent_definition_run.no_linked_pylon_capacity',
    )
    expect(body.run.status).toBe('refused')
    expect(runStore.rows[0]?.run.state).toBe('failed')
    expect(pylon.assignments).toHaveLength(0)
  })

  test('does not read definitions outside the authenticated owner scope', async () => {
    const { agent, dependencies, forge, pylon, runStore } =
      await makeDependencies()
    const response = await handleAgentDefinitionRunRequest(
      jsonRequest({
        body: {
          triggerRef: 'trigger.public.route_test.manual',
        },
        path: '/v1/agent-definitions/agent_definition.route_test.owner/runs',
        token: agent.otherToken,
      }),
      dependencies,
    )
    const body = (await response!.json()) as { readonly error: string }

    expect(response?.status).toBe(404)
    expect(body.error).toBe('agent_definition_not_found')
    expect(forge.issues).toHaveLength(0)
    expect(pylon.assignments).toHaveLength(0)
    expect(runStore.rows).toHaveLength(0)
  })
})
