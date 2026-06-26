import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type AgentCredentialLookup,
  type AgentRegistrationRecord,
  type AgentRegistrationStore,
  sha256Hex,
} from './agent-registration'
import {
  PYLON_CODEX_TURN_INGEST_PATH,
  codexTurnUsageTokenCounts,
  makePylonCodexTurnIngestRoutes,
} from './pylon-codex-turn-ingest-routes'
import type { TraceVisibility } from './atif-trace-schema'
import {
  type PylonApiAssignmentRecord,
  type PylonApiStore,
} from './pylon-api'
import {
  type TokenUsageIngestResult,
  type TokenUsageLedgerShape,
} from './token-usage-ledger'
import {
  type CreateTraceInput,
  type CreateTraceResult,
  type TraceDemandKind,
  type TraceRecord,
  type TraceStore,
} from './trace-store-d1'

const nowIso = '2026-06-26T12:00:00.000Z'
const agentToken = 'oa_agent_pylon_codex_turn_ingest_test_token'
const agentUserId = 'agent-user-pylon-codex-1'
const linkedOpenAuthUserId = 'user-openauth-linked-1'

class MemoryAgentStore implements AgentRegistrationStore {
  private readonly tokenHash: string

  constructor(tokenHash: string) {
    this.tokenHash = tokenHash
  }

  createAgentRegistration(_record: AgentRegistrationRecord): Promise<void> {
    return Promise.resolve()
  }

  findAgentByTokenHash(
    tokenHash: string,
    _now: string,
  ): Promise<AgentCredentialLookup | undefined> {
    if (tokenHash !== this.tokenHash) {
      return Promise.resolve(undefined)
    }

    return Promise.resolve({
      credentialId: 'credential-pylon-codex-1',
      openauthUserId: linkedOpenAuthUserId,
      profileMetadataJson: '{}',
      tokenPrefix: 'oa_agent_pyl',
      user: {
        avatarUrl: null,
        createdAt: nowIso,
        displayName: 'Linked Pylon Codex',
        id: agentUserId,
        kind: 'agent',
        primaryEmail: null,
        status: 'active',
        updatedAt: nowIso,
      },
    })
  }

  touchAgentCredential(
    _credentialId: string,
    _lastUsedAt: string,
  ): Promise<void> {
    return Promise.resolve()
  }

  updateAgentDisplayName(): Promise<number> {
    return Promise.resolve(0)
  }
}

const assignmentRecord = (
  overrides: Partial<PylonApiAssignmentRecord> = {},
): PylonApiAssignmentRecord => ({
  acceptanceCriteriaRefs: [],
  acceptedWorkRefs: [],
  artifactRefs: [],
  assignmentRef: 'assignment-pylon-codex-1',
  closeoutRefs: [],
  codingAssignment: null,
  createdAt: nowIso,
  id: 'assignment-id-1',
  idempotencyKeyHash: 'assignment-idempotency-1',
  jobKind: 'codex_agent_task',
  leaseExpiresAt: '2026-06-26T12:10:00.000Z',
  ownerAgentUserId: agentUserId,
  proofRefs: [],
  publicProjectionJson: '{}',
  pylonRef: 'pylon-local-codex-1',
  rejectionRefs: [],
  resultExpectationRefs: [],
  state: 'running',
  taskRefs: [],
  updatedAt: nowIso,
  ...overrides,
})

class MemoryPylonStore {
  constructor(private readonly assignment: PylonApiAssignmentRecord) {}

  readAssignment = async (assignmentRef: string) =>
    assignmentRef === this.assignment.assignmentRef ? this.assignment : undefined
}

class MemoryTokenUsageLedger {
  readonly events: Array<Record<string, unknown>> = []
  private readonly idempotencyKeys = new Set<string>()

  readonly ingestEvent = (
    body: unknown,
  ): Effect.Effect<TokenUsageIngestResult> => {
    const event = body as Record<string, unknown>
    const idempotencyKey = String(event.idempotencyKey ?? '')
    const inserted = !this.idempotencyKeys.has(idempotencyKey)
    if (inserted) {
      this.idempotencyKeys.add(idempotencyKey)
      this.events.push(event)
    }

    return Effect.succeed({
      event: body as TokenUsageIngestResult['event'],
      inserted,
    })
  }

  readonly shape = (): TokenUsageLedgerShape =>
    ({
      ingestEvent: this.ingestEvent,
    }) as unknown as TokenUsageLedgerShape
}

class MemoryTraceStore implements TraceStore {
  readonly inputs: Array<CreateTraceInput> = []
  readonly records: Array<TraceRecord> = []

  failCreate = false

  createTrace(input: CreateTraceInput): Promise<CreateTraceResult> {
    this.inputs.push(input)
    if (this.failCreate) {
      return Promise.reject(new Error('synthetic trace store failure'))
    }

    const existing = this.records.find(
      record =>
        record.ownerUserId === input.ownerUserId &&
        input.idempotencyKey !== null &&
        record.idempotencyKey === input.idempotencyKey,
    )
    if (existing !== undefined) {
      return Promise.resolve({ created: false, record: existing })
    }

    const record: TraceRecord = {
      agentRef: input.agentRef,
      blobRefs: input.blobRefs,
      contentDigest: input.contentDigest,
      createdAt: input.nowIso,
      demandKind: input.demandKind,
      demandSource: input.demandSource,
      idempotencyKey: input.idempotencyKey,
      license: input.license,
      ownerUserId: input.ownerUserId,
      rewardAmountSats: input.rewardAmountSats,
      rewardEligible: input.rewardEligible,
      schemaVersion: input.schemaVersion,
      sessionId: input.sessionId,
      stepCount: input.stepCount,
      traceUuid: input.traceUuid,
      trainingConsent: input.trainingConsent,
      trajectory: input.trajectory,
      trajectoryId: input.trajectoryId,
      trajectoryR2Key: input.trajectoryR2Key,
      updatedAt: input.nowIso,
      uploadSource: input.uploadSource,
      visibility: input.visibility,
    }
    this.records.push(record)

    return Promise.resolve({ created: true, record })
  }

  readTraceByUuid(traceUuid: string): Promise<TraceRecord | undefined> {
    return Promise.resolve(
      this.records.find(record => record.traceUuid === traceUuid),
    )
  }

  listTracesForOwner(
    ownerUserId: string,
    limit: number,
  ): Promise<ReadonlyArray<TraceRecord>> {
    return Promise.resolve(
      this.records
        .filter(record => record.ownerUserId === ownerUserId)
        .slice(0, limit),
    )
  }

  findTraceByOwnerDigest(
    ownerUserId: string,
    contentDigest: string,
  ): Promise<TraceRecord | undefined> {
    return Promise.resolve(
      this.records.find(
        record =>
          record.ownerUserId === ownerUserId &&
          record.contentDigest === contentDigest,
      ),
    )
  }

  countTracesForOwnerSince(
    ownerUserId: string,
    _sinceIso: string,
  ): Promise<number> {
    return Promise.resolve(
      this.records.filter(record => record.ownerUserId === ownerUserId).length,
    )
  }

  listTracesForOwnerByDemand(
    ownerUserId: string,
    limit: number,
    demandKinds: ReadonlyArray<TraceDemandKind> | undefined,
  ): Promise<ReadonlyArray<TraceRecord>> {
    return Promise.resolve(
      this.records
        .filter(
          record =>
            record.ownerUserId === ownerUserId &&
            (demandKinds === undefined ||
              demandKinds.includes(record.demandKind ?? 'unlabeled')),
        )
        .slice(0, limit),
    )
  }

  countTracesForOwnerByDemand(
    ownerUserId: string,
  ): Promise<Record<TraceDemandKind, number>> {
    const counts: Record<TraceDemandKind, number> = {
      external: 0,
      internal: 0,
      own_capacity: 0,
      unlabeled: 0,
    }
    for (const record of this.records) {
      if (record.ownerUserId === ownerUserId) {
        counts[record.demandKind ?? 'unlabeled'] += 1
      }
    }

    return Promise.resolve(counts)
  }

  updateTraceVisibility(
    traceUuid: string,
    ownerUserId: string,
    visibility: TraceVisibility,
    nowIsoValue: string,
  ): Promise<TraceRecord | undefined> {
    const existing = this.records.find(
      record =>
        record.traceUuid === traceUuid && record.ownerUserId === ownerUserId,
    )
    if (existing === undefined) {
      return Promise.resolve(undefined)
    }
    const updated = { ...existing, updatedAt: nowIsoValue, visibility }
    const index = this.records.indexOf(existing)
    this.records[index] = updated

    return Promise.resolve(updated)
  }
}

const requestBody = () => ({
  schemaVersion: 'openagents.pylon.codex_turn.v1',
  assignmentRef: 'assignment-pylon-codex-1',
  leaseRef: 'lease-pylon-codex-1',
  pylonRef: 'pylon-local-codex-1',
  runRef: 'run-pylon-codex-1',
  sessionRef: 'session-pylon-codex-1',
  workspaceRef: 'workspace.public.pylon-codex-1',
  turnIndex: 1,
  observedAt: nowIso,
  usage: {
    cachedInputTokens: 9,
    inputTokens: 100,
    outputTokens: 25,
    reasoningOutputTokens: 7,
  },
  items: [
    {
      itemType: 'agent_message',
      message:
        'Removed alice@example.com, sk-proj-abcdefghijklmnopqrstuvwxyz, and /Users/chris/.codex/auth.json.',
      ordinal: 1,
      status: 'completed',
    },
    {
      commandLabel: 'shell_command',
      exitCode: 0,
      itemType: 'command_execution',
      ordinal: 2,
      outputBytes: 123,
      status: 'completed',
    },
    {
      changeCount: 2,
      itemType: 'file_change',
      ordinal: 3,
      status: 'completed',
    },
  ],
})

const postTurn = (body: unknown): Request =>
  new Request(`https://openagents.com${PYLON_CODEX_TURN_INGEST_PATH}`, {
    body: JSON.stringify(body),
    headers: {
      authorization: `Bearer ${agentToken}`,
      'content-type': 'application/json',
    },
    method: 'POST',
  })

const makeHarness = async (
  overrides: Readonly<{
    assignment?: PylonApiAssignmentRecord
  }> = {},
) => {
  const agentStore = new MemoryAgentStore(await sha256Hex(agentToken))
  const ledger = new MemoryTokenUsageLedger()
  const traceStore = new MemoryTraceStore()
  const pylonStore = new MemoryPylonStore(
    overrides.assignment ?? assignmentRecord(),
  )
  const deltas: Array<{
    eventRef: string
    observedAt: string
    tokensServedDelta: number
  }> = []
  let idCounter = 0
  const routes = makePylonCodexTurnIngestRoutes({
    agentStore: () => agentStore,
    ledger: () => ledger.shape(),
    makeId: () => {
      idCounter += 1
      return `trace-pylon-codex-${idCounter}`
    },
    nowIso: () => nowIso,
    publishDelta: (_env, delta) => {
      deltas.push(delta)
      return Effect.void
    },
    pylonStore: () =>
      pylonStore as unknown as Pick<PylonApiStore, 'readAssignment'>,
    traceStore: () => traceStore,
  })

  return { deltas, ledger, routes, traceStore }
}

describe('POST /api/pylon/codex/turns', () => {
  test('stores exact downstream Codex tokens and an owner-only redacted trace', async () => {
    const { deltas, ledger, routes, traceStore } = await makeHarness()
    const response = await Effect.runPromise(
      routes.handlePylonCodexTurnIngestApi(postTurn(requestBody()), {}),
    )
    const body = (await response.json()) as {
      insertedTokenUsage: boolean
      tokensServedDelta: number
      trace: { created?: boolean; visibility?: string }
    }

    expect(response.status).toBe(200)
    expect(body.insertedTokenUsage).toBe(true)
    expect(body.tokensServedDelta).toBe(132)
    expect(body.trace).toMatchObject({
      created: true,
      visibility: 'owner_only',
    })

    const event = ledger.events[0]
    expect(event).toMatchObject({
      backendProfile: 'pylon-codex-own-capacity',
      demand: {
        demandKind: 'own_capacity',
        demandSource: 'khala_coding_delegation',
      },
      model: 'openagents/pylon-codex',
      provider: 'pylon-codex-own-capacity',
      usageTruth: 'exact',
    })
    expect(event?.actor).toMatchObject({
      accountRef: `agent:${agentUserId}`,
      userId: linkedOpenAuthUserId,
    })
    expect(event?.sourceRefs).toMatchObject({
      repositoryRef: 'workspace.public.pylon-codex-1',
      runRef: 'run-pylon-codex-1',
      sessionRef: 'session-pylon-codex-1',
      taskRef: 'assignment-pylon-codex-1',
    })
    expect(event?.tokenCounts).toMatchObject({
      cacheReadTokens: 9,
      inputTokens: 100,
      outputTokens: 32,
      reasoningTokens: 7,
      totalTokens: 132,
    })
    expect(event?.safeMetadata).toMatchObject({
      assignmentRef: 'assignment-pylon-codex-1',
      codexUsageSplit: {
        cachedInputTokens: 9,
        inputTokens: 100,
        outputTokens: 25,
        reasoningOutputTokens: 7,
      },
      costCaveat: 'owner_capacity_provider_cost_unknown',
      leaseRef: 'lease-pylon-codex-1',
      pylonRef: 'pylon-local-codex-1',
      usageBasis: 'codex_sdk_turn_completed',
    })
    expect(JSON.stringify(event)).not.toMatch(/raw|prompt|secret/i)
    expect(deltas).toEqual([
      {
        eventRef: String(event?.eventId),
        observedAt: nowIso,
        tokensServedDelta: 132,
      },
    ])

    const traceInput = traceStore.inputs[0]
    expect(traceInput).toMatchObject({
      agentRef: `agent:${agentUserId}`,
      demandKind: 'own_capacity',
      demandSource: 'khala_coding_delegation',
      ownerUserId: linkedOpenAuthUserId,
      trainingConsent: false,
      uploadSource: 'agent',
      visibility: 'owner_only',
    })
    const traceJson = JSON.stringify(traceInput?.trajectory)
    expect(traceJson).not.toContain('alice@example.com')
    expect(traceJson).not.toContain('sk-proj-abcdefghijklmnopqrstuvwxyz')
    expect(traceJson).not.toContain('/Users/chris')
    expect(traceJson).toContain('[REDACTED:')
  })

  test('publishes no second delta and creates no second row for an idempotent replay', async () => {
    const { deltas, ledger, routes, traceStore } = await makeHarness()
    const first = await Effect.runPromise(
      routes.handlePylonCodexTurnIngestApi(postTurn(requestBody()), {}),
    )
    const second = await Effect.runPromise(
      routes.handlePylonCodexTurnIngestApi(postTurn(requestBody()), {}),
    )
    const secondBody = (await second.json()) as {
      insertedTokenUsage: boolean
      tokensServedDelta: number
      trace: { created?: boolean; uuid?: string }
    }

    expect(first.status).toBe(200)
    expect(second.status).toBe(200)
    expect(secondBody.insertedTokenUsage).toBe(false)
    expect(secondBody.tokensServedDelta).toBe(0)
    expect(secondBody.trace.created).toBe(false)
    expect(ledger.events).toHaveLength(1)
    expect(traceStore.records).toHaveLength(1)
    expect(deltas).toHaveLength(1)
  })

  test('rejects attempts to ingest turns for another agent owner', async () => {
    const { ledger, routes, traceStore } = await makeHarness({
      assignment: assignmentRecord({ ownerAgentUserId: 'agent-user-other' }),
    })
    const response = await Effect.runPromise(
      routes.handlePylonCodexTurnIngestApi(postTurn(requestBody()), {}),
    )
    const body = (await response.json()) as { error?: string }

    expect(response.status).toBe(403)
    expect(body.error).toBe('pylon_codex_forbidden')
    expect(ledger.events).toHaveLength(0)
    expect(traceStore.records).toHaveLength(0)
  })

  test('keeps exact token ingest fail-soft when trace storage is unavailable', async () => {
    const { deltas, ledger, routes, traceStore } = await makeHarness()
    traceStore.failCreate = true

    const response = await Effect.runPromise(
      routes.handlePylonCodexTurnIngestApi(postTurn(requestBody()), {}),
    )
    const body = (await response.json()) as {
      insertedTokenUsage: boolean
      tokensServedDelta: number
      trace: {
        diagnostic?: { operation?: string; reason?: string }
        dropped?: boolean
        visibility?: string
      }
    }

    expect(response.status).toBe(200)
    expect(body.insertedTokenUsage).toBe(true)
    expect(body.tokensServedDelta).toBe(132)
    expect(body.trace).toMatchObject({
      diagnostic: {
        operation: 'trace_store_create',
        reason: 'trace_store_unavailable',
      },
      dropped: true,
      visibility: 'owner_only',
    })
    expect(ledger.events).toHaveLength(1)
    expect(traceStore.records).toHaveLength(0)
    expect(deltas).toHaveLength(1)
  })

  test('counts Codex reasoning tokens into the public served-token total', () => {
    expect(
      codexTurnUsageTokenCounts({
        cachedInputTokens: 20,
        inputTokens: 100,
        outputTokens: 30,
        reasoningOutputTokens: 11,
      }),
    ).toEqual({
      cacheReadTokens: 20,
      inputTokens: 100,
      outputTokens: 41,
      reasoningTokens: 11,
      totalTokens: 141,
    })
  })
})
