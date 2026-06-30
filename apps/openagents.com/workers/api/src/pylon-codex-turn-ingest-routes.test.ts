import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type AgentCredentialLookup,
  type AgentRegistrationRecord,
  type AgentRegistrationStore,
  sha256Hex,
} from './agent-registration'
import {
  PYLON_CLAUDE_TURN_INGEST_PATH,
  PYLON_CODEX_ASSIGNMENT_PROOF_PATH,
  PYLON_CODEX_ASSIGNMENT_TRACE_STATUS_PATH,
  PYLON_CODEX_EVENT_CHUNK_INGEST_PATH,
  PYLON_CODEX_TURN_INGEST_PATH,
  codexTurnUsageTokenCounts,
  makeD1PylonCodexAssignmentProofStore,
  makeD1R2PylonCodexRawEventChunkStore,
  makeD1R2PylonCodexRawEventStore,
  makePylonCodexTurnIngestRoutes,
  pylonCodexRawEventChunkRef,
  pylonCodexRawEventRef,
  type PylonCodexAssignmentProof,
  type PylonCodexAssignmentProofStore,
  type PylonCodexAssignmentTraceStatus,
  type PylonCodexAssignmentTraceStatusStore,
  type PylonCodexRawEventChunkStore,
  type PylonCodexRawEventChunkStoreInput,
  type PylonCodexRawEventChunkStoreResult,
  type PylonCodexRawEventStore,
  type PylonCodexRawEventStoreInput,
  type PylonCodexRawEventStoreResult,
} from './pylon-codex-turn-ingest-routes'
import type { TraceVisibility } from './atif-trace-schema'
import {
  type PylonApiAssignmentRecord,
  type PylonApiEventRecord,
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
const noSpendCloseoutPolicy = {
  paymentMode: 'no-spend' as const,
  payoutClaimAllowed: false,
  settlementState: 'not_applicable' as const,
  source: 'worker_closeout_event' as const,
}

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

  listEventsForAssignment = async (
    assignmentRef: string,
  ): Promise<ReadonlyArray<PylonApiEventRecord>> =>
    assignmentRef === this.assignment.assignmentRef
      ? [
          {
            assignmentRef,
            createdAt: nowIso,
            eventBody: {
              closeoutRefs: ['assignment.closeout.summary.fixture'],
            },
            eventKind: 'worker_closeout',
            eventRef: 'event.pylon_codex.worker_closeout',
            id: 'event-id-pylon-codex-worker-closeout',
            idempotencyKeyHash: 'event-idempotency-pylon-codex-worker-closeout',
            ownerAgentUserId: this.assignment.ownerAgentUserId,
            publicProjectionJson: '{}',
            pylonRef: this.assignment.pylonRef,
            status: 'accepted',
          },
        ]
      : []

  readAssignment = async (assignmentRef: string) =>
    assignmentRef === this.assignment.assignmentRef ? this.assignment : undefined
}

class MemoryProofStore
  implements PylonCodexAssignmentProofStore, PylonCodexAssignmentTraceStatusStore
{
  readonly inputs: Array<{
    assignmentRef: string
    nowIso: string
    ownerAgentUserId: string
    ownerUserId: string
    pylonRef: string
  }> = []

  readAssignmentProof(
    input: Parameters<PylonCodexAssignmentProofStore['readAssignmentProof']>[0],
  ): Promise<PylonCodexAssignmentProof> {
    this.inputs.push({ ...input })
    return Promise.resolve({
      schemaVersion: 'openagents.pylon.codex_assignment_proof.v1',
      assignmentRef: input.assignmentRef,
      pylonRef: input.pylonRef,
      owner: {
        agentUserRef: `agent:${input.ownerAgentUserId}`,
        openauthUserRef: input.ownerUserId,
      },
      tokenUsage: {
        rowCount: 2,
        refs: [
          'event.inference.served-tokens.pylon-codex.001',
          'event.inference.served-tokens.pylon-codex.002',
        ],
        provider: 'pylon-codex-own-capacity',
        model: 'openagents/pylon-codex',
        usageTruth: 'exact',
        demandKind: 'own_capacity',
        demandSource: 'khala_coding_delegation',
        inputTokens: 200,
        outputTokens: 64,
        reasoningTokens: 14,
        cacheReadTokens: 18,
        totalTokens: 264,
      },
      traces: {
        count: 2,
        visibility: 'owner_only',
        schemaVersion: 'ATIF-v1.7',
        refs: ['trace-pylon-codex-1', 'trace-pylon-codex-2'],
      },
      rawEvents: {
        count: 2,
        eventCount: 6,
        byteLength: 2048,
        visibility: 'owner_only',
        refs: ['raw.pylon_codex.abc123', 'raw.pylon_codex.def456'],
      },
      closeoutPolicy: input.closeoutPolicy,
      generatedAt: input.nowIso,
    })
  }

  readAssignmentTraceStatus(
    input: Parameters<
      PylonCodexAssignmentTraceStatusStore['readAssignmentTraceStatus']
    >[0],
  ): Promise<PylonCodexAssignmentTraceStatus> {
    return Promise.resolve({
      schemaVersion: 'openagents.pylon.codex_assignment_trace_status.v1',
      assignmentRef: input.assignment.assignmentRef,
      pylonRef: input.assignment.pylonRef,
      owner: {
        agentUserRef: `agent:${input.ownerAgentUserId}`,
        openauthUserRef: input.ownerUserId,
      },
      lifecycle: {
        acceptedWorkRefs: input.assignment.acceptedWorkRefs,
        artifactRefs: input.assignment.artifactRefs,
        closeoutRefs: input.assignment.closeoutRefs,
        createdAt: input.assignment.createdAt,
        proofRefs: input.assignment.proofRefs,
        rejectionRefs: input.assignment.rejectionRefs,
        state: input.assignment.state,
        updatedAt: input.assignment.updatedAt,
      },
      events: {
        count: 2,
        latestProgressObservedAt: nowIso,
        latestProgressStatus: 'proof-ready',
        progressCount: 1,
        latestEventKind: 'assignment_progress',
        latestStatus: 'proof-ready',
        latestObservedAt: nowIso,
      },
      tokenUsage: {
        rowCount: 0,
        refs: [],
        provider: 'pylon-codex-own-capacity',
        model: 'openagents/pylon-codex',
        usageTruth: 'exact',
        demandKind: 'own_capacity',
        demandSource: 'khala_coding_delegation',
        inputTokens: 0,
        outputTokens: 0,
        reasoningTokens: 0,
        cacheReadTokens: 0,
        totalTokens: 0,
        status: 'pending',
      },
      traces: {
        count: 1,
        visibility: 'owner_only',
        schemaVersion: 'ATIF-v1.7',
        latestTraceUuid: 'trace-pylon-codex-latest',
        finalTraceUuid: null,
        refs: ['trace-pylon-codex-latest'],
      },
      rawEventChunks: {
        count: 3,
        eventCount: 9,
        byteLength: 1234,
        latestChunkRef: 'raw_chunk.pylon_codex.latest',
        latestObservedAt: nowIso,
        visibility: 'owner_only',
      },
      rawEvents: {
        count: 0,
        eventCount: 0,
        byteLength: 0,
        latestRawEventRef: null,
        latestObservedAt: null,
        visibility: 'owner_only',
        refs: [],
      },
      closeoutPolicy: input.closeoutPolicy,
      progress: {
        closeoutReady: false,
        hasFinalTrace: false,
        hasLiveChunks: true,
        hasTokenUsage: false,
        missingReadinessRefs: [
          'status.pylon_codex.final_trace.pending',
          'status.pylon_codex.token_usage.pending',
          'status.pylon_codex.closeout.pending',
        ],
        state: 'streaming_chunks',
      },
      generatedAt: input.nowIso,
    })
  }
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
      internal_stress: 0,
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

class MemoryRawEventStore implements PylonCodexRawEventStore {
  readonly inputs: Array<PylonCodexRawEventStoreInput> = []
  readonly records = new Map<string, PylonCodexRawEventStoreResult>()

  failPut = false

  putTurnEvents(
    input: PylonCodexRawEventStoreInput,
  ): Promise<PylonCodexRawEventStoreResult> {
    this.inputs.push(input)
    if (this.failPut) {
      return Promise.reject(new Error('synthetic raw event store failure'))
    }
    const ref = pylonCodexRawEventRef(input.digest)
    const r2Key = `private/pylon-codex-raw-events/${input.assignmentRef}/turn-${input.turnIndex}/${input.digest}.json`
    const existing = this.records.get(ref)
    if (existing !== undefined) {
      return Promise.resolve({ ...existing, created: false })
    }
    const record = {
      byteLength: new TextEncoder().encode(input.eventsJson).byteLength,
      created: true,
      ref,
      r2Key,
    }
    this.records.set(ref, record)
    return Promise.resolve(record)
  }
}

class MemoryRawEventChunkStore implements PylonCodexRawEventChunkStore {
  readonly inputs: Array<PylonCodexRawEventChunkStoreInput> = []
  readonly records = new Map<string, PylonCodexRawEventChunkStoreResult>()

  failPut = false

  putEventChunk(
    input: PylonCodexRawEventChunkStoreInput,
  ): Promise<PylonCodexRawEventChunkStoreResult> {
    this.inputs.push(input)
    if (this.failPut) {
      return Promise.reject(new Error('synthetic raw event chunk failure'))
    }
    const ref = pylonCodexRawEventChunkRef(input.digest)
    const r2Key = `private/pylon-codex-raw-event-chunks/${input.assignmentRef}/turn-${input.turnIndex}/chunk-${input.chunkIndex}/${input.digest}.json`
    const existing = this.records.get(ref)
    if (existing !== undefined) {
      return Promise.resolve({ ...existing, created: false })
    }
    const record = {
      byteLength: new TextEncoder().encode(input.eventsJson).byteLength,
      created: true,
      ref,
      r2Key,
    }
    this.records.set(ref, record)
    return Promise.resolve(record)
  }
}

type RawEventMetadataRow = Readonly<{
  assignment_ref: string
  byte_length: number
  content_digest: string
  created_at: string
  demand_kind: string
  demand_source: string
  event_count: number
  lease_ref: string
  observed_at: string
  owner_user_id: string
  pylon_ref: string
  r2_key: string
  raw_event_ref: string
  run_ref: string | null
  session_ref: string | null
  turn_index: number
  updated_at: string
  workspace_ref: string | null
}>

type RawEventChunkMetadataRow = Readonly<{
  assignment_ref: string
  byte_length: number
  chunk_index: number
  chunk_ref: string
  content_digest: string
  created_at: string
  demand_kind: string
  demand_source: string
  event_count: number
  lease_ref: string
  observed_at: string
  owner_user_id: string
  pylon_ref: string
  r2_key: string
  run_ref: string | null
  session_ref: string | null
  turn_index: number
  updated_at: string
  workspace_ref: string | null
}>

type ProofTokenUsageRow = Readonly<{
  account_ref: string
  actor_user_id: string
  cache_read_tokens: number
  demand_kind: string
  demand_source: string
  id: string
  input_tokens: number
  model: string
  observed_at: string
  output_tokens: number
  provider: string
  reasoning_tokens: number
  task_ref: string
  total_tokens: number
  usage_truth: string
}>

type ProofTraceRow = Readonly<{
  agent_ref: string
  created_at: string
  demand_kind: string
  demand_source: string
  owner_user_id: string
  schema_version: string
  trace_uuid: string
  trajectory_id: string
  visibility: string
}>

type ProofRawEventRow = Readonly<{
  assignment_ref: string
  byte_length: number
  demand_kind: string
  demand_source: string
  event_count: number
  observed_at: string
  owner_user_id: string
  pylon_ref: string
  raw_event_ref: string
  turn_index: number
}>

type ProofRawEventChunkRow = Readonly<{
  assignment_ref: string
  byte_length: number
  chunk_index: number
  chunk_ref: string
  demand_kind: string
  demand_source: string
  event_count: number
  observed_at: string
  owner_user_id: string
  pylon_ref: string
  turn_index: number
}>

type ProofPylonEventRow = Readonly<{
  archived_at: string | null
  assignment_ref: string
  created_at: string
  event_kind: string
  owner_agent_user_id: string
  status: string
}>

const makeFakeRawEventD1 = (): D1Database & {
  rows: Array<RawEventMetadataRow>
} => {
  const rows: Array<RawEventMetadataRow> = []

  const statement = (_query: string): D1PreparedStatement => {
    let bound: ReadonlyArray<unknown> = []
    const stmt: D1PreparedStatement = {
      all: async <T,>() => ({
        meta: {} as D1Meta & Record<string, unknown>,
        results: rows as unknown as T[],
        success: true as const,
      }),
      bind: (...values: ReadonlyArray<unknown>) => {
        bound = values
        return stmt
      },
      first: async <T,>() => {
        const ref = String(bound[0])
        return (rows.find(row => row.raw_event_ref === ref) ??
          null) as T | null
      },
      raw: async () => {
        throw new Error('raw unused')
      },
      run: async <T,>() => {
        const [
          raw_event_ref,
          assignment_ref,
          lease_ref,
          pylon_ref,
          owner_user_id,
          run_ref,
          session_ref,
          workspace_ref,
          turn_index,
          event_count,
          byte_length,
          content_digest,
          r2_key,
          observed_at,
          created_at,
          updated_at,
          demand_kind,
          demand_source,
        ] = bound as [
          string,
          string,
          string,
          string,
          string,
          string | null,
          string | null,
          string | null,
          number,
          number,
          number,
          string,
          string,
          string,
          string,
          string,
          string,
          string,
        ]
        const existed = rows.some(row => row.raw_event_ref === raw_event_ref)
        if (!existed) {
          rows.push({
            assignment_ref,
            byte_length,
            content_digest,
            created_at,
            demand_kind,
            demand_source,
            event_count,
            lease_ref,
            observed_at,
            owner_user_id,
            pylon_ref,
            r2_key,
            raw_event_ref,
            run_ref,
            session_ref,
            turn_index,
            updated_at,
            workspace_ref,
          })
        }
        return {
          meta: { changes: existed ? 0 : 1 } as D1Meta &
            Record<string, unknown>,
          results: [] as unknown as T[],
          success: true as const,
        }
      },
    }
    return stmt
  }

  return {
    batch: () => Promise.reject(new Error('batch unused')),
    dump: () => Promise.reject(new Error('dump unused')),
    exec: () => Promise.reject(new Error('exec unused')),
    prepare: (query: string) => statement(query),
    rows,
    withSession: () => {
      throw new Error('session unused')
    },
  } as unknown as D1Database & { rows: Array<RawEventMetadataRow> }
}

const makeFakeRawEventChunkD1 = (): D1Database & {
  rows: Array<RawEventChunkMetadataRow>
} => {
  const rows: Array<RawEventChunkMetadataRow> = []

  const statement = (_query: string): D1PreparedStatement => {
    let bound: ReadonlyArray<unknown> = []
    const stmt: D1PreparedStatement = {
      all: async <T,>() => ({
        meta: {} as D1Meta & Record<string, unknown>,
        results: rows as unknown as T[],
        success: true as const,
      }),
      bind: (...values: ReadonlyArray<unknown>) => {
        bound = values
        return stmt
      },
      first: async <T,>() => {
        const ref = String(bound[0])
        return (rows.find(row => row.chunk_ref === ref) ?? null) as T | null
      },
      raw: async () => {
        throw new Error('raw unused')
      },
      run: async <T,>() => {
        const [
          chunk_ref,
          assignment_ref,
          lease_ref,
          pylon_ref,
          owner_user_id,
          run_ref,
          session_ref,
          workspace_ref,
          turn_index,
          chunk_index,
          event_count,
          byte_length,
          content_digest,
          r2_key,
          observed_at,
          created_at,
          updated_at,
          demand_kind,
          demand_source,
        ] = bound as [
          string,
          string,
          string,
          string,
          string,
          string | null,
          string | null,
          string | null,
          number,
          number,
          number,
          number,
          string,
          string,
          string,
          string,
          string,
          string,
          string,
        ]
        const existed = rows.some(row => row.chunk_ref === chunk_ref)
        if (!existed) {
          rows.push({
            assignment_ref,
            byte_length,
            chunk_index,
            chunk_ref,
            content_digest,
            created_at,
            demand_kind,
            demand_source,
            event_count,
            lease_ref,
            observed_at,
            owner_user_id,
            pylon_ref,
            r2_key,
            run_ref,
            session_ref,
            turn_index,
            updated_at,
            workspace_ref,
          })
        }
        return {
          meta: { changes: existed ? 0 : 1 } as D1Meta &
            Record<string, unknown>,
          results: [] as unknown as T[],
          success: true as const,
        }
      },
    }
    return stmt
  }

  return {
    batch: () => Promise.reject(new Error('batch unused')),
    dump: () => Promise.reject(new Error('dump unused')),
    exec: () => Promise.reject(new Error('exec unused')),
    prepare: (query: string) => statement(query),
    rows,
    withSession: () => {
      throw new Error('session unused')
    },
  } as unknown as D1Database & { rows: Array<RawEventChunkMetadataRow> }
}

const makeFakeProofD1 = (): D1Database & {
  eventRows: Array<ProofPylonEventRow>
  rawChunkRows: Array<ProofRawEventChunkRow>
  rawRows: Array<ProofRawEventRow>
  tokenRows: Array<ProofTokenUsageRow>
  traceRows: Array<ProofTraceRow>
} => {
  const tokenRows: Array<ProofTokenUsageRow> = []
  const traceRows: Array<ProofTraceRow> = []
  const rawRows: Array<ProofRawEventRow> = []
  const rawChunkRows: Array<ProofRawEventChunkRow> = []
  const eventRows: Array<ProofPylonEventRow> = []

  const matchTrace = (
    row: ProofTraceRow,
    values: ReadonlyArray<unknown>,
  ): boolean => {
    const ownerUserId = String(values[0] ?? '')
    const agentRef = String(values[1] ?? '')
    const schemaVersion = String(values[2] ?? '')
    const demandKind = String(values[3] ?? '')
    const demandSource = String(values[4] ?? '')
    const trajectoryPrefixLength = Number(values[5] ?? 0)
    const trajectoryPrefix = String(values[6] ?? '')
    return (
      row.owner_user_id === ownerUserId &&
      row.agent_ref === agentRef &&
      row.visibility === 'owner_only' &&
      row.schema_version === schemaVersion &&
      row.demand_kind === demandKind &&
      row.demand_source === demandSource &&
      row.trajectory_id.slice(0, trajectoryPrefixLength) === trajectoryPrefix
    )
  }

  const matchRaw = (
    row: ProofRawEventRow | ProofRawEventChunkRow,
    values: ReadonlyArray<unknown>,
  ): boolean => {
    const [ownerUserId, assignmentRef, pylonRef, demandKind, demandSource] =
      values.map(String)
    return (
      row.owner_user_id === ownerUserId &&
      row.assignment_ref === assignmentRef &&
      row.pylon_ref === pylonRef &&
      row.demand_kind === demandKind &&
      row.demand_source === demandSource
    )
  }

  const matchToken = (
    row: ProofTokenUsageRow,
    values: ReadonlyArray<unknown>,
  ): boolean => {
    const [
      provider,
      model,
      demandKind,
      demandSource,
      taskRef,
      accountRef,
      actorUserId,
    ] = values.map(String)
    return (
      row.provider === provider &&
      row.model === model &&
      row.usage_truth === 'exact' &&
      row.demand_kind === demandKind &&
      row.demand_source === demandSource &&
      row.task_ref === taskRef &&
      row.account_ref === accountRef &&
      row.actor_user_id === actorUserId
    )
  }

  const statement = (query: string): D1PreparedStatement => {
    let bound: ReadonlyArray<unknown> = []
    const stmt: D1PreparedStatement = {
      all: async <T,>() => {
        if (
          query.includes('SELECT id') &&
          query.includes('FROM token_usage_events')
        ) {
          return {
            meta: {} as D1Meta & Record<string, unknown>,
            results: tokenRows
              .filter(row => matchToken(row, bound))
              .sort(
                (left, right) =>
                  left.observed_at.localeCompare(right.observed_at) ||
                  left.id.localeCompare(right.id),
              )
              .slice(0, 100)
              .map(row => ({ id: row.id })) as unknown as T[],
            success: true as const,
          }
        }
        if (query.includes('SELECT trace_uuid')) {
          return {
            meta: {} as D1Meta & Record<string, unknown>,
            results: traceRows
              .filter(row => matchTrace(row, bound))
              .sort((left, right) => right.created_at.localeCompare(left.created_at))
              .slice(0, 100)
              .map(row => ({
                trace_uuid: row.trace_uuid,
                trajectory_id: row.trajectory_id,
              })) as unknown as T[],
            success: true as const,
          }
        }
        if (query.includes('SELECT chunk_ref')) {
          return {
            meta: {} as D1Meta & Record<string, unknown>,
            results: rawChunkRows
              .filter(row => matchRaw(row, bound))
              .sort(
                (left, right) =>
                  right.turn_index - left.turn_index ||
                  right.chunk_index - left.chunk_index,
              )
              .slice(0, 100)
              .map(row => ({
                chunk_ref: row.chunk_ref,
                observed_at: row.observed_at,
              })) as unknown as T[],
            success: true as const,
          }
        }
        if (query.includes('SELECT raw_event_ref')) {
          return {
            meta: {} as D1Meta & Record<string, unknown>,
            results: rawRows
              .filter(row => matchRaw(row, bound))
              .sort((left, right) => left.turn_index - right.turn_index)
              .slice(0, 100)
              .map(row => ({
                observed_at: row.observed_at,
                raw_event_ref: row.raw_event_ref,
              })) as unknown as T[],
            success: true as const,
          }
        }
        throw new Error(`unexpected proof all query: ${query}`)
      },
      bind: (...values: ReadonlyArray<unknown>) => {
        bound = values
        return stmt
      },
      first: async <T,>() => {
        if (query.includes('FROM token_usage_events')) {
          const matches = tokenRows.filter(row => matchToken(row, bound))
          return {
            cache_read_tokens: matches.reduce(
              (total, row) => total + row.cache_read_tokens,
              0,
            ),
            input_tokens: matches.reduce(
              (total, row) => total + row.input_tokens,
              0,
            ),
            output_tokens: matches.reduce(
              (total, row) => total + row.output_tokens,
              0,
            ),
            reasoning_tokens: matches.reduce(
              (total, row) => total + row.reasoning_tokens,
              0,
            ),
            row_count: matches.length,
            total_tokens: matches.reduce(
              (total, row) => total + row.total_tokens,
              0,
            ),
          } as T
        }
        if (query.includes('FROM pylon_api_events')) {
          const [
            latestEventAssignmentRef,
            latestEventOwnerId,
            latestStatusAssignmentRef,
            latestStatusOwnerId,
            latestProgressStatusAssignmentRef,
            latestProgressStatusOwnerId,
            latestProgressObservedAtAssignmentRef,
            latestProgressObservedAtOwnerId,
            assignmentRef,
            ownerAgentUserId,
          ] = bound.map(String)
          const activeRows = eventRows.filter(row => row.archived_at === null)
          const matches = activeRows
            .filter(
              row =>
                row.assignment_ref === assignmentRef &&
                row.owner_agent_user_id === ownerAgentUserId,
            )
            .sort((left, right) => right.created_at.localeCompare(left.created_at))
          const latestEvent = activeRows
            .filter(
              row =>
                row.assignment_ref === latestEventAssignmentRef &&
                row.owner_agent_user_id === latestEventOwnerId,
            )
            .sort((left, right) => right.created_at.localeCompare(left.created_at))[0]
          const latestStatus = activeRows
            .filter(
              row =>
                row.assignment_ref === latestStatusAssignmentRef &&
                row.owner_agent_user_id === latestStatusOwnerId,
            )
            .sort((left, right) => right.created_at.localeCompare(left.created_at))[0]
          const latestProgressStatus = activeRows
            .filter(
              row =>
                row.assignment_ref === latestProgressStatusAssignmentRef &&
                row.owner_agent_user_id === latestProgressStatusOwnerId &&
                row.event_kind === 'assignment_progress',
            )
            .sort((left, right) => right.created_at.localeCompare(left.created_at))[0]
          const latestProgressObservedAt = activeRows
            .filter(
              row =>
                row.assignment_ref === latestProgressObservedAtAssignmentRef &&
                row.owner_agent_user_id === latestProgressObservedAtOwnerId &&
                row.event_kind === 'assignment_progress',
            )
            .sort((left, right) => right.created_at.localeCompare(left.created_at))[0]
          return {
            event_count: matches.length,
            latest_event_kind: latestEvent?.event_kind ?? null,
            latest_observed_at: matches[0]?.created_at ?? null,
            latest_progress_observed_at:
              latestProgressObservedAt?.created_at ?? null,
            latest_progress_status: latestProgressStatus?.status ?? null,
            latest_status: latestStatus?.status ?? null,
            progress_count: matches.filter(
              row => row.event_kind === 'assignment_progress',
            ).length,
          } as T
        }
        if (
          query.includes('FROM agent_traces') &&
          query.includes('COUNT(*) AS row_count')
        ) {
          return {
            row_count: traceRows.filter(row => matchTrace(row, bound)).length,
          } as T
        }
        if (
          query.includes('FROM pylon_codex_raw_events') &&
          query.includes('SUM(event_count)')
        ) {
          const matches = rawRows.filter(row => matchRaw(row, bound))
          return {
            byte_length: matches.reduce(
              (total, row) => total + row.byte_length,
              0,
            ),
            event_count: matches.reduce(
              (total, row) => total + row.event_count,
              0,
            ),
            row_count: matches.length,
          } as T
        }
        if (
          query.includes('FROM pylon_codex_raw_event_chunks') &&
          query.includes('SUM(event_count)')
        ) {
          const matches = rawChunkRows.filter(row => matchRaw(row, bound))
          return {
            byte_length: matches.reduce(
              (total, row) => total + row.byte_length,
              0,
            ),
            event_count: matches.reduce(
              (total, row) => total + row.event_count,
              0,
            ),
            row_count: matches.length,
          } as T
        }
        if (query.includes('FROM pylon_codex_raw_event_chunks')) {
          return (
            rawChunkRows
              .filter(row => matchRaw(row, bound))
              .sort(
                (left, right) =>
                  right.turn_index - left.turn_index ||
                  right.chunk_index - left.chunk_index,
              )
              .map(row => ({
                chunk_ref: row.chunk_ref,
                observed_at: row.observed_at,
              }))[0] ?? null
          ) as T | null
        }
        throw new Error(`unexpected proof first query: ${query}`)
      },
      raw: async () => {
        throw new Error('raw unused')
      },
      run: async () => {
        throw new Error('run unused')
      },
    }
    return stmt
  }

  return {
    batch: () => Promise.reject(new Error('batch unused')),
    dump: () => Promise.reject(new Error('dump unused')),
    exec: () => Promise.reject(new Error('exec unused')),
    prepare: (query: string) => statement(query),
    eventRows,
    rawChunkRows,
    rawRows,
    tokenRows,
    traceRows,
    withSession: () => {
      throw new Error('session unused')
    },
  } as unknown as D1Database & {
    eventRows: Array<ProofPylonEventRow>
    rawChunkRows: Array<ProofRawEventChunkRow>
    rawRows: Array<ProofRawEventRow>
    tokenRows: Array<ProofTokenUsageRow>
    traceRows: Array<ProofTraceRow>
  }
}

class MemoryRawEventsR2Bucket {
  readonly objects = new Map<
    string,
    Readonly<{
      body: string
      customMetadata: Record<string, string>
      contentType: string
    }>
  >()

  head(key: string): Promise<R2Object | null> {
    return Promise.resolve(
      this.objects.has(key) ? ({ key } as R2Object) : null,
    )
  }

  put(
    key: string,
    value: string | ArrayBuffer | ArrayBufferView | ReadableStream | Blob,
    options?: R2PutOptions,
  ): Promise<R2Object> {
    this.objects.set(key, {
      body: typeof value === 'string' ? value : '',
      contentType:
        options?.httpMetadata instanceof Headers
          ? (options.httpMetadata.get('content-type') ?? '')
          : (options?.httpMetadata?.contentType ?? ''),
      customMetadata: options?.customMetadata ?? {},
    })
    return Promise.resolve({ key } as R2Object)
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
  rawEvents: [
    {
      local_path: '/Users/chris/.codex/auth.json',
      thread_id: 'thread-private-raw-1',
      type: 'thread.started',
    },
    {
      item: {
        aggregated_output: 'raw shell output with sk-proj-secret',
        command: 'cat /Users/chris/.codex/auth.json',
        exit_code: 0,
        type: 'command_execution',
      },
      type: 'item.completed',
    },
    {
      type: 'turn.completed',
      usage: {
        cached_input_tokens: 9,
        input_tokens: 100,
        output_tokens: 25,
        reasoning_output_tokens: 7,
      },
    },
  ],
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

const postEventChunk = (body: unknown): Request =>
  new Request(
    `https://openagents.com${PYLON_CODEX_EVENT_CHUNK_INGEST_PATH}`,
    {
      body: JSON.stringify(body),
      headers: {
        authorization: `Bearer ${agentToken}`,
        'content-type': 'application/json',
      },
      method: 'POST',
    },
  )

const getProof = (assignmentRef: string, token = agentToken): Request =>
  new Request(
    `https://openagents.com${PYLON_CODEX_ASSIGNMENT_PROOF_PATH}?assignmentRef=${encodeURIComponent(assignmentRef)}`,
    {
      headers: { authorization: `Bearer ${token}` },
      method: 'GET',
    },
  )

const getTraceStatus = (assignmentRef: string, token = agentToken): Request =>
  new Request(
    `https://openagents.com${PYLON_CODEX_ASSIGNMENT_TRACE_STATUS_PATH}?assignmentRef=${encodeURIComponent(assignmentRef)}`,
    {
      headers: { authorization: `Bearer ${token}` },
      method: 'GET',
    },
  )

const makeHarness = async (
  overrides: Readonly<{
    assignment?: PylonApiAssignmentRecord
  }> = {},
) => {
  const agentStore = new MemoryAgentStore(await sha256Hex(agentToken))
  const ledger = new MemoryTokenUsageLedger()
  const proofStore = new MemoryProofStore()
  const rawEventChunkStore = new MemoryRawEventChunkStore()
  const rawEventStore = new MemoryRawEventStore()
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
      pylonStore as unknown as Pick<
        PylonApiStore,
        'listEventsForAssignment' | 'readAssignment'
      >,
    proofStore: () => proofStore,
    traceStatusStore: () => proofStore,
    rawEventChunkStore: () => rawEventChunkStore,
    rawEventStore: () => rawEventStore,
    traceStore: () => traceStore,
  })

  return {
    deltas,
    ledger,
    proofStore,
    rawEventChunkStore,
    rawEventStore,
    routes,
    traceStore,
  }
}

describe('GET /api/pylon/codex/proof', () => {
  test('returns owner-scoped public-safe proof totals and refs', async () => {
    const { proofStore, routes } = await makeHarness()
    const response = await Effect.runPromise(
      routes.handlePylonCodexAssignmentProofApi(
        getProof('assignment-pylon-codex-1'),
        {},
      ),
    )
    const body = (await response.json()) as PylonCodexAssignmentProof

    expect(response.status).toBe(200)
    expect(body).toMatchObject({
      schemaVersion: 'openagents.pylon.codex_assignment_proof.v1',
      assignmentRef: 'assignment-pylon-codex-1',
      pylonRef: 'pylon-local-codex-1',
      owner: {
        agentUserRef: `agent:${agentUserId}`,
        openauthUserRef: linkedOpenAuthUserId,
      },
      tokenUsage: {
        rowCount: 2,
        refs: [
          'event.inference.served-tokens.pylon-codex.001',
          'event.inference.served-tokens.pylon-codex.002',
        ],
        provider: 'pylon-codex-own-capacity',
        model: 'openagents/pylon-codex',
        usageTruth: 'exact',
        demandKind: 'own_capacity',
        demandSource: 'khala_coding_delegation',
        totalTokens: 264,
      },
      traces: {
        count: 2,
        visibility: 'owner_only',
        schemaVersion: 'ATIF-v1.7',
      },
      rawEvents: {
        count: 2,
        eventCount: 6,
        byteLength: 2048,
        visibility: 'owner_only',
      },
      closeoutPolicy: noSpendCloseoutPolicy,
    })
    expect(body.traces.refs).toEqual([
      'trace-pylon-codex-1',
      'trace-pylon-codex-2',
    ])
    expect(body.rawEvents.refs).toEqual([
      'raw.pylon_codex.abc123',
      'raw.pylon_codex.def456',
    ])
    expect(JSON.stringify(body)).not.toMatch(
      /trajectory_json|safe_metadata_json|r2_key|prompt|command|shell|\/Users|secret|access[_-]?token|bearer/i,
    )
    expect(proofStore.inputs).toEqual([
      {
        assignmentRef: 'assignment-pylon-codex-1',
        nowIso,
        ownerAgentUserId: agentUserId,
        ownerUserId: linkedOpenAuthUserId,
        pylonRef: 'pylon-local-codex-1',
        closeoutPolicy: noSpendCloseoutPolicy,
      },
    ])
  })

  test('does not read proof rows for another agent owner', async () => {
    const { proofStore, routes } = await makeHarness({
      assignment: assignmentRecord({ ownerAgentUserId: 'agent-user-other' }),
    })
    const response = await Effect.runPromise(
      routes.handlePylonCodexAssignmentProofApi(
        getProof('assignment-pylon-codex-1'),
        {},
      ),
    )
    const body = (await response.json()) as { error?: string }

    expect(response.status).toBe(403)
    expect(body.error).toBe('pylon_codex_forbidden')
    expect(proofStore.inputs).toHaveLength(0)
  })

  test('D1 proof store aggregates exact rows and bounds returned refs', async () => {
    const db = makeFakeProofD1()
    const store = makeD1PylonCodexAssignmentProofStore(
      db as unknown as D1Database,
    )

    db.tokenRows.push(
      {
        account_ref: `agent:${agentUserId}`,
        actor_user_id: linkedOpenAuthUserId,
        cache_read_tokens: 3,
        demand_kind: 'own_capacity',
        demand_source: 'khala_coding_delegation',
        id: 'event.inference.served-tokens.pylon-codex.001',
        input_tokens: 100,
        model: 'openagents/pylon-codex',
        observed_at: '2026-06-26T12:00:01.000Z',
        output_tokens: 50,
        provider: 'pylon-codex-own-capacity',
        reasoning_tokens: 10,
        task_ref: 'assignment-pylon-codex-1',
        total_tokens: 150,
        usage_truth: 'exact',
      },
      {
        account_ref: `agent:${agentUserId}`,
        actor_user_id: linkedOpenAuthUserId,
        cache_read_tokens: 7,
        demand_kind: 'own_capacity',
        demand_source: 'khala_coding_delegation',
        id: 'event.inference.served-tokens.pylon-codex.002',
        input_tokens: 20,
        model: 'openagents/pylon-codex',
        observed_at: '2026-06-26T12:00:02.000Z',
        output_tokens: 15,
        provider: 'pylon-codex-own-capacity',
        reasoning_tokens: 4,
        task_ref: 'assignment-pylon-codex-1',
        total_tokens: 35,
        usage_truth: 'exact',
      },
      {
        account_ref: `agent:${agentUserId}`,
        actor_user_id: 'other-user',
        cache_read_tokens: 1000,
        demand_kind: 'own_capacity',
        demand_source: 'khala_coding_delegation',
        id: 'event.inference.served-tokens.pylon-codex.other',
        input_tokens: 1000,
        model: 'openagents/pylon-codex',
        observed_at: '2026-06-26T12:00:03.000Z',
        output_tokens: 1000,
        provider: 'pylon-codex-own-capacity',
        reasoning_tokens: 1000,
        task_ref: 'assignment-pylon-codex-1',
        total_tokens: 2000,
        usage_truth: 'exact',
      },
    )

    for (let index = 0; index < 101; index += 1) {
      const padded = String(index).padStart(3, '0')
      db.traceRows.push({
        agent_ref: `agent:${agentUserId}`,
        created_at: `2026-06-26T12:${padded.slice(0, 2)}:${padded.slice(1)}.000Z`,
        demand_kind: 'own_capacity',
        demand_source: 'khala_coding_delegation',
        owner_user_id: linkedOpenAuthUserId,
        schema_version: 'ATIF-v1.7',
        trace_uuid: `trace-${padded}`,
        trajectory_id: `pylon_codex:assignment-pylon-codex-1:turn:${index}`,
        visibility: 'owner_only',
      })
      db.rawRows.push({
        assignment_ref: 'assignment-pylon-codex-1',
        byte_length: 100 + index,
        demand_kind: 'own_capacity',
        demand_source: 'khala_coding_delegation',
        event_count: 1 + index,
        observed_at: `2026-06-26T12:${padded.slice(0, 2)}:${padded.slice(1)}.000Z`,
        owner_user_id: linkedOpenAuthUserId,
        pylon_ref: 'pylon-local-codex-1',
        raw_event_ref: `raw.pylon_codex.${padded}`,
        turn_index: index,
      })
    }
    db.rawRows.push({
      assignment_ref: 'assignment-pylon-codex-1',
      byte_length: 9999,
      demand_kind: 'own_capacity',
      demand_source: 'khala_coding_delegation',
      event_count: 9999,
      observed_at: '2026-06-26T12:99:99.000Z',
      owner_user_id: 'other-user',
      pylon_ref: 'pylon-local-codex-1',
      raw_event_ref: 'raw.pylon_codex.other',
      turn_index: 999,
    })

    const proof = await store.readAssignmentProof({
      assignmentRef: 'assignment-pylon-codex-1',
      closeoutPolicy: noSpendCloseoutPolicy,
      nowIso,
      ownerAgentUserId: agentUserId,
      ownerUserId: linkedOpenAuthUserId,
      pylonRef: 'pylon-local-codex-1',
    })

    expect(proof.tokenUsage).toMatchObject({
      cacheReadTokens: 10,
      inputTokens: 120,
      outputTokens: 65,
      reasoningTokens: 14,
      refs: [
        'event.inference.served-tokens.pylon-codex.001',
        'event.inference.served-tokens.pylon-codex.002',
      ],
      rowCount: 2,
      totalTokens: 185,
      usageTruth: 'exact',
    })
    expect(proof.traces.count).toBe(101)
    expect(proof.traces.refs).toHaveLength(100)
    expect(proof.rawEvents).toMatchObject({
      byteLength: 15150,
      count: 101,
      eventCount: 5151,
      visibility: 'owner_only',
    })
    expect(proof.rawEvents.refs).toHaveLength(100)
    expect(JSON.stringify(proof)).not.toMatch(
      /trajectory_json|safe_metadata_json|r2_key|prompt|command|shell|\/Users|secret|access[_-]?token|bearer/i,
    )
  })
})

describe('GET /api/pylon/codex/trace-status', () => {
  test('returns owner-scoped in-progress assignment status without raw payloads', async () => {
    const { routes } = await makeHarness()
    const response = await Effect.runPromise(
      routes.handlePylonCodexAssignmentTraceStatusApi(
        getTraceStatus('assignment-pylon-codex-1'),
        {},
      ),
    )
    const body = (await response.json()) as PylonCodexAssignmentTraceStatus

    expect(response.status).toBe(200)
    expect(body).toMatchObject({
      schemaVersion: 'openagents.pylon.codex_assignment_trace_status.v1',
      assignmentRef: 'assignment-pylon-codex-1',
      lifecycle: {
        state: 'running',
      },
      events: {
        count: 2,
        latestEventKind: 'assignment_progress',
        latestStatus: 'proof-ready',
        latestProgressStatus: 'proof-ready',
        progressCount: 1,
      },
      owner: {
        agentUserRef: `agent:${agentUserId}`,
        openauthUserRef: linkedOpenAuthUserId,
      },
      progress: {
        closeoutReady: false,
        hasFinalTrace: false,
        hasLiveChunks: true,
        hasTokenUsage: false,
        state: 'streaming_chunks',
      },
      rawEventChunks: {
        count: 3,
        eventCount: 9,
        latestChunkRef: 'raw_chunk.pylon_codex.latest',
        visibility: 'owner_only',
      },
      tokenUsage: {
        rowCount: 0,
        status: 'pending',
      },
    })
    expect(JSON.stringify(body)).not.toMatch(
      /trajectory_json|safe_metadata_json|r2_key|prompt|command|shell|\/Users|secret|access[_-]?token|bearer/i,
    )
  })

  test('does not read trace status for another agent owner', async () => {
    const { routes } = await makeHarness({
      assignment: assignmentRecord({ ownerAgentUserId: 'agent-user-other' }),
    })
    const response = await Effect.runPromise(
      routes.handlePylonCodexAssignmentTraceStatusApi(
        getTraceStatus('assignment-pylon-codex-1'),
        {},
      ),
    )
    const body = (await response.json()) as { error?: string }

    expect(response.status).toBe(403)
    expect(body.error).toBe('pylon_codex_forbidden')
  })

  test('D1 trace status distinguishes live chunks from final exact closeout rows', async () => {
    const db = makeFakeProofD1()
    const store = makeD1PylonCodexAssignmentProofStore(
      db as unknown as D1Database,
    )

    db.rawChunkRows.push(
      {
        assignment_ref: 'assignment-pylon-codex-1',
        byte_length: 50,
        chunk_index: 1,
        chunk_ref: 'raw_chunk.pylon_codex.001',
        demand_kind: 'own_capacity',
        demand_source: 'khala_coding_delegation',
        event_count: 2,
        observed_at: '2026-06-26T12:00:01.000Z',
        owner_user_id: linkedOpenAuthUserId,
        pylon_ref: 'pylon-local-codex-1',
        turn_index: 1,
      },
      {
        assignment_ref: 'assignment-pylon-codex-1',
        byte_length: 70,
        chunk_index: 2,
        chunk_ref: 'raw_chunk.pylon_codex.002',
        demand_kind: 'own_capacity',
        demand_source: 'khala_coding_delegation',
        event_count: 3,
        observed_at: '2026-06-26T12:00:02.000Z',
        owner_user_id: linkedOpenAuthUserId,
        pylon_ref: 'pylon-local-codex-1',
        turn_index: 1,
      },
    )
    db.eventRows.push(
      {
        archived_at: null,
        assignment_ref: 'assignment-pylon-codex-1',
        created_at: '2026-06-26T12:00:01.000Z',
        event_kind: 'assignment_accepted',
        owner_agent_user_id: agentUserId,
        status: 'accepted',
      },
      {
        archived_at: null,
        assignment_ref: 'assignment-pylon-codex-1',
        created_at: '2026-06-26T12:00:02.000Z',
        event_kind: 'assignment_progress',
        owner_agent_user_id: agentUserId,
        status: 'runtime_active',
      },
      {
        archived_at: '2026-06-26T12:00:03.000Z',
        assignment_ref: 'assignment-pylon-codex-1',
        created_at: '2026-06-26T12:00:03.000Z',
        event_kind: 'assignment_progress',
        owner_agent_user_id: agentUserId,
        status: 'archived',
      },
    )
    db.traceRows.push({
      agent_ref: `agent:${agentUserId}`,
      created_at: '2026-06-26T12:00:02.000Z',
      demand_kind: 'own_capacity',
      demand_source: 'khala_coding_delegation',
      owner_user_id: linkedOpenAuthUserId,
      schema_version: 'ATIF-v1.7',
      trace_uuid: 'trace-chunk-2',
      trajectory_id: 'pylon_codex:assignment-pylon-codex-1:turn:1:chunk:2',
      visibility: 'owner_only',
    })

    const inProgress = await store.readAssignmentTraceStatus({
      assignment: assignmentRecord(),
      closeoutPolicy: noSpendCloseoutPolicy,
      nowIso,
      ownerAgentUserId: agentUserId,
      ownerUserId: linkedOpenAuthUserId,
    })

    expect(inProgress.progress).toEqual({
      closeoutReady: false,
      hasFinalTrace: false,
      hasLiveChunks: true,
      hasTokenUsage: false,
      missingReadinessRefs: [
        'status.pylon_codex.final_trace.pending',
        'status.pylon_codex.token_usage.pending',
        'status.pylon_codex.closeout.pending',
      ],
      state: 'streaming_chunks',
    })
    expect(inProgress.rawEventChunks).toMatchObject({
      byteLength: 120,
      count: 2,
      eventCount: 5,
      latestChunkRef: 'raw_chunk.pylon_codex.002',
      latestObservedAt: '2026-06-26T12:00:02.000Z',
    })
    expect(inProgress.events).toMatchObject({
      count: 2,
      latestEventKind: 'assignment_progress',
      latestObservedAt: '2026-06-26T12:00:02.000Z',
      latestProgressObservedAt: '2026-06-26T12:00:02.000Z',
      latestProgressStatus: 'runtime_active',
      latestStatus: 'runtime_active',
      progressCount: 1,
    })
    expect(inProgress.tokenUsage.status).toBe('pending')

    db.tokenRows.push({
      account_ref: `agent:${agentUserId}`,
      actor_user_id: linkedOpenAuthUserId,
      cache_read_tokens: 3,
      demand_kind: 'own_capacity',
      demand_source: 'khala_coding_delegation',
      id: 'event.inference.served-tokens.pylon-codex.final',
      input_tokens: 100,
      model: 'openagents/pylon-codex',
      observed_at: '2026-06-26T12:00:05.000Z',
      output_tokens: 50,
      provider: 'pylon-codex-own-capacity',
      reasoning_tokens: 10,
      task_ref: 'assignment-pylon-codex-1',
      total_tokens: 150,
      usage_truth: 'exact',
    })
    db.traceRows.push({
      agent_ref: `agent:${agentUserId}`,
      created_at: '2026-06-26T12:00:05.000Z',
      demand_kind: 'own_capacity',
      demand_source: 'khala_coding_delegation',
      owner_user_id: linkedOpenAuthUserId,
      schema_version: 'ATIF-v1.7',
      trace_uuid: 'trace-final-turn',
      trajectory_id: 'pylon_codex:assignment-pylon-codex-1:turn:1',
      visibility: 'owner_only',
    })
    db.rawRows.push({
      assignment_ref: 'assignment-pylon-codex-1',
      byte_length: 500,
      demand_kind: 'own_capacity',
      demand_source: 'khala_coding_delegation',
      event_count: 5,
      observed_at: '2026-06-26T12:00:05.000Z',
      owner_user_id: linkedOpenAuthUserId,
      pylon_ref: 'pylon-local-codex-1',
      raw_event_ref: 'raw.pylon_codex.final',
      turn_index: 1,
    })

    const complete = await store.readAssignmentTraceStatus({
      assignment: assignmentRecord({
        acceptedWorkRefs: ['accepted.public.fixture'],
        closeoutRefs: ['assignment.closeout.fixture'],
        state: 'accepted_work',
      }),
      closeoutPolicy: noSpendCloseoutPolicy,
      nowIso,
      ownerAgentUserId: agentUserId,
      ownerUserId: linkedOpenAuthUserId,
    })

    expect(complete.progress).toEqual({
      closeoutReady: true,
      hasFinalTrace: true,
      hasLiveChunks: true,
      hasTokenUsage: true,
      missingReadinessRefs: [],
      state: 'closed_out',
    })
    expect(complete.tokenUsage).toMatchObject({
      rowCount: 1,
      refs: ['event.inference.served-tokens.pylon-codex.final'],
      status: 'recorded',
      totalTokens: 150,
    })
    expect(complete.traces).toMatchObject({
      finalTraceUuid: 'trace-final-turn',
      latestTraceUuid: 'trace-final-turn',
    })
    expect(complete.rawEvents).toMatchObject({
      count: 1,
      latestRawEventRef: 'raw.pylon_codex.final',
      visibility: 'owner_only',
    })
    expect(JSON.stringify(complete)).not.toMatch(
      /trajectory_json|safe_metadata_json|r2_key|prompt|command|shell|\/Users|secret|access[_-]?token|bearer/i,
    )
  })
})

describe('POST /api/pylon/codex/turns', () => {
  test('stores streaming raw event chunks and owner-only redacted chunk traces without token rows', async () => {
    const { deltas, ledger, rawEventChunkStore, routes, traceStore } =
      await makeHarness()
    const chunkBody = {
      schemaVersion: 'openagents.pylon.codex_event_chunk.v1',
      assignmentRef: 'assignment-pylon-codex-1',
      leaseRef: 'lease-pylon-codex-1',
      pylonRef: 'pylon-local-codex-1',
      runRef: 'run-pylon-codex-1',
      sessionRef: 'session-pylon-codex-1',
      workspaceRef: 'workspace.public.pylon-codex-1',
      turnIndex: 1,
      chunkIndex: 2,
      observedAt: nowIso,
      rawEvents: [
        {
          item: {
            aggregated_output: 'raw output with sk-proj-secret',
            command: 'cat /Users/chris/.codex/auth.json',
            type: 'command_execution',
          },
          type: 'item.completed',
        },
      ],
      items: [
        {
          itemType: 'agent_message',
          message:
            'Streamed alice@example.com and sk-proj-abcdefghijklmnopqrstuvwxyz.',
          ordinal: 1,
          status: 'completed',
        },
      ],
    }

    const first = await Effect.runPromise(
      routes.handlePylonCodexEventChunkIngestApi(postEventChunk(chunkBody), {}),
    )
    const firstBody = (await first.json()) as {
      rawEvents: {
        created?: boolean
        eventCount?: number
        ref?: string
        r2Key?: string
        visibility?: string
      }
      trace: { created?: boolean; visibility?: string }
    }
    const second = await Effect.runPromise(
      routes.handlePylonCodexEventChunkIngestApi(postEventChunk(chunkBody), {}),
    )
    const secondBody = (await second.json()) as {
      rawEvents: { created?: boolean }
      trace: { created?: boolean }
    }

    expect(first.status).toBe(200)
    expect(second.status).toBe(200)
    expect(firstBody.rawEvents).toMatchObject({
      created: true,
      eventCount: 1,
      visibility: 'owner_only',
    })
    expect(firstBody.rawEvents.ref).toMatch(/^raw_chunk\.pylon_codex\./)
    expect(firstBody.rawEvents.r2Key).toContain(
      'private/pylon-codex-raw-event-chunks/assignment-pylon-codex-1/turn-1/chunk-2/',
    )
    expect(firstBody.trace).toMatchObject({
      created: true,
      visibility: 'owner_only',
    })
    expect(secondBody.rawEvents.created).toBe(false)
    expect(secondBody.trace.created).toBe(false)
    expect(ledger.events).toHaveLength(0)
    expect(deltas).toHaveLength(0)
    expect(rawEventChunkStore.records.size).toBe(1)
    expect(rawEventChunkStore.inputs[0]).toMatchObject({
      assignmentRef: 'assignment-pylon-codex-1',
      chunkIndex: 2,
      eventCount: 1,
      leaseRef: 'lease-pylon-codex-1',
      ownerUserId: linkedOpenAuthUserId,
      pylonRef: 'pylon-local-codex-1',
      turnIndex: 1,
    })
    expect(rawEventChunkStore.inputs[0]?.eventsJson).toContain(
      'raw output with sk-proj-secret',
    )
    expect(traceStore.records).toHaveLength(1)
    const traceJson = JSON.stringify(traceStore.records[0]?.trajectory)
    expect(traceJson).not.toContain('alice@example.com')
    expect(traceJson).not.toContain('sk-proj-abcdefghijklmnopqrstuvwxyz')
    expect(traceJson).toContain('[REDACTED:')
  })

  test('stores exact downstream Codex tokens and an owner-only redacted trace', async () => {
    const { deltas, ledger, rawEventStore, routes, traceStore } =
      await makeHarness()
    const response = await Effect.runPromise(
      routes.handlePylonCodexTurnIngestApi(postTurn(requestBody()), {}),
    )
    const body = (await response.json()) as {
      insertedTokenUsage: boolean
      rawEvents: {
        byteLength?: number
        created?: boolean
        eventCount?: number
        ref?: string
        r2Key?: string
        visibility?: string
      }
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
    expect(body.rawEvents).toMatchObject({
      created: true,
      eventCount: 3,
      visibility: 'owner_only',
    })
    expect(body.rawEvents.ref).toMatch(/^raw\.pylon_codex\./)
    expect(body.rawEvents.r2Key).toContain(
      'private/pylon-codex-raw-events/assignment-pylon-codex-1/turn-1/',
    )

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

    const rawInput = rawEventStore.inputs[0]
    expect(rawInput).toMatchObject({
      assignmentRef: 'assignment-pylon-codex-1',
      eventCount: 3,
      leaseRef: 'lease-pylon-codex-1',
      ownerUserId: linkedOpenAuthUserId,
      pylonRef: 'pylon-local-codex-1',
      runRef: 'run-pylon-codex-1',
      sessionRef: 'session-pylon-codex-1',
      turnIndex: 1,
      workspaceRef: 'workspace.public.pylon-codex-1',
    })
    expect(rawInput?.eventsJson).toContain('/Users/chris/.codex/auth.json')
    expect(rawInput?.eventsJson).toContain(
      'raw shell output with sk-proj-secret',
    )
    expect(traceJson).not.toContain('raw shell output with sk-proj-secret')
  })

  test('publishes no second delta and creates no second row for an idempotent replay', async () => {
    const { deltas, ledger, rawEventStore, routes, traceStore } =
      await makeHarness()
    const first = await Effect.runPromise(
      routes.handlePylonCodexTurnIngestApi(postTurn(requestBody()), {}),
    )
    const second = await Effect.runPromise(
      routes.handlePylonCodexTurnIngestApi(postTurn(requestBody()), {}),
    )
    const secondBody = (await second.json()) as {
      insertedTokenUsage: boolean
      rawEvents: { created?: boolean }
      tokensServedDelta: number
      trace: { created?: boolean; uuid?: string }
    }

    expect(first.status).toBe(200)
    expect(second.status).toBe(200)
    expect(secondBody.insertedTokenUsage).toBe(false)
    expect(secondBody.tokensServedDelta).toBe(0)
    expect(secondBody.trace.created).toBe(false)
    expect(secondBody.rawEvents.created).toBe(false)
    expect(ledger.events).toHaveLength(1)
    expect(rawEventStore.records.size).toBe(1)
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

  test('keeps exact token ingest fail-soft when raw event storage is unavailable', async () => {
    const { deltas, ledger, rawEventStore, routes, traceStore } =
      await makeHarness()
    rawEventStore.failPut = true

    const response = await Effect.runPromise(
      routes.handlePylonCodexTurnIngestApi(postTurn(requestBody()), {}),
    )
    const body = (await response.json()) as {
      insertedTokenUsage: boolean
      rawEvents: {
        diagnostic?: { operation?: string; reason?: string }
        dropped?: boolean
        eventCount?: number
        visibility?: string
      }
      tokensServedDelta: number
    }

    expect(response.status).toBe(200)
    expect(body.insertedTokenUsage).toBe(true)
    expect(body.tokensServedDelta).toBe(132)
    expect(body.rawEvents).toMatchObject({
      diagnostic: {
        operation: 'raw_codex_events_store',
        reason: 'synthetic raw event store failure',
      },
      dropped: true,
      eventCount: 3,
      visibility: 'owner_only',
    })
    expect(ledger.events).toHaveLength(1)
    expect(traceStore.records).toHaveLength(1)
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

  test('D1+R2 raw event store writes private payload metadata idempotently', async () => {
    const db = makeFakeRawEventD1()
    const bucket = new MemoryRawEventsR2Bucket()
    const store = makeD1R2PylonCodexRawEventStore(
      db as unknown as D1Database,
      bucket as unknown as R2Bucket,
    )
    const input: PylonCodexRawEventStoreInput = {
      assignmentRef: 'assignment-pylon-codex-1',
      digest: 'abc123def4567890abc123def4567890abc123def4567890',
      eventCount: 1,
      eventsJson: JSON.stringify({
        events: [{ type: 'turn.completed', usage: { input_tokens: 1 } }],
      }),
      leaseRef: 'lease-pylon-codex-1',
      observedAt: nowIso,
      ownerUserId: linkedOpenAuthUserId,
      pylonRef: 'pylon-local-codex-1',
      runRef: 'run-pylon-codex-1',
      sessionRef: 'session-pylon-codex-1',
      turnIndex: 1,
      workspaceRef: 'workspace.public.pylon-codex-1',
    }

    const first = await store.putTurnEvents(input)
    const second = await store.putTurnEvents(input)

    expect(first).toMatchObject({
      created: true,
      ref: pylonCodexRawEventRef(input.digest),
    })
    expect(first.r2Key).toContain(
      'private/pylon-codex-raw-events/assignment-pylon-codex-1/turn-1/',
    )
    expect(second).toMatchObject({
      created: false,
      ref: first.ref,
      r2Key: first.r2Key,
    })
    expect(db.rows).toHaveLength(1)
    expect(db.rows[0]).toMatchObject({
      assignment_ref: input.assignmentRef,
      content_digest: input.digest,
      demand_kind: 'own_capacity',
      demand_source: 'khala_coding_delegation',
      event_count: 1,
      owner_user_id: linkedOpenAuthUserId,
      raw_event_ref: first.ref,
      r2_key: first.r2Key,
      turn_index: 1,
    })
    expect(bucket.objects.size).toBe(1)
    expect(bucket.objects.get(first.r2Key)?.body).toContain('turn.completed')
    expect(bucket.objects.get(first.r2Key)?.customMetadata).toMatchObject({
      assignmentRef: input.assignmentRef,
      ownerUserId: linkedOpenAuthUserId,
      rawEventRef: first.ref,
    })
  })

  test('D1+R2 raw event chunk store writes live chunk metadata idempotently', async () => {
    const db = makeFakeRawEventChunkD1()
    const bucket = new MemoryRawEventsR2Bucket()
    const store = makeD1R2PylonCodexRawEventChunkStore(
      db as unknown as D1Database,
      bucket as unknown as R2Bucket,
    )
    const input: PylonCodexRawEventChunkStoreInput = {
      assignmentRef: 'assignment-pylon-codex-1',
      chunkIndex: 3,
      digest: 'def456abc1237890def456abc1237890def456abc1237890',
      eventCount: 2,
      eventsJson: JSON.stringify({
        events: [{ type: 'item.completed' }, { type: 'turn.completed' }],
      }),
      leaseRef: 'lease-pylon-codex-1',
      observedAt: nowIso,
      ownerUserId: linkedOpenAuthUserId,
      pylonRef: 'pylon-local-codex-1',
      runRef: 'run-pylon-codex-1',
      sessionRef: 'session-pylon-codex-1',
      turnIndex: 1,
      workspaceRef: 'workspace.public.pylon-codex-1',
    }

    const first = await store.putEventChunk(input)
    const second = await store.putEventChunk(input)

    expect(first).toMatchObject({
      created: true,
      ref: pylonCodexRawEventChunkRef(input.digest),
    })
    expect(first.r2Key).toContain(
      'private/pylon-codex-raw-event-chunks/assignment-pylon-codex-1/turn-1/chunk-3/',
    )
    expect(second).toMatchObject({
      created: false,
      ref: first.ref,
      r2Key: first.r2Key,
    })
    expect(db.rows).toHaveLength(1)
    expect(db.rows[0]).toMatchObject({
      assignment_ref: input.assignmentRef,
      chunk_index: 3,
      chunk_ref: first.ref,
      content_digest: input.digest,
      demand_kind: 'own_capacity',
      demand_source: 'khala_coding_delegation',
      event_count: 2,
      owner_user_id: linkedOpenAuthUserId,
      r2_key: first.r2Key,
      turn_index: 1,
    })
    expect(bucket.objects.size).toBe(1)
    expect(bucket.objects.get(first.r2Key)?.body).toContain('item.completed')
    expect(bucket.objects.get(first.r2Key)?.customMetadata).toMatchObject({
      assignmentRef: input.assignmentRef,
      chunkIndex: '3',
      ownerUserId: linkedOpenAuthUserId,
      rawEventChunkRef: first.ref,
    })
  })
})

// #6388/#6391: the Claude own-capacity coding lane reuses the same authenticated,
// assignment-owned, idempotent token ingest helpers as Codex, but records the
// exact `token_usage_events` row under the Claude provider/model.
const claudeTurnBody = () => ({
  schemaVersion: 'openagents.pylon.claude_turn.v1',
  assignmentRef: 'assignment-pylon-codex-1',
  leaseRef: 'lease-pylon-claude-1',
  pylonRef: 'pylon-local-codex-1',
  runRef: 'run-pylon-claude-1',
  sessionRef: 'session-pylon-claude-1',
  workspaceRef: 'workspace.public.pylon-claude-1',
  turnIndex: 1,
  observedAt: nowIso,
  usage: {
    cachedInputTokens: 11,
    inputTokens: 200,
    outputTokens: 80,
  },
})

const postClaudeTurn = (body: unknown, token = agentToken): Request =>
  new Request(`https://openagents.com${PYLON_CLAUDE_TURN_INGEST_PATH}`, {
    body: JSON.stringify(body),
    headers: {
      authorization: `Bearer ${token}`,
      'content-type': 'application/json',
    },
    method: 'POST',
  })

describe('POST /api/pylon/claude/turns', () => {
  test('records the exact own-capacity Claude token row and a counter delta', async () => {
    const { deltas, ledger, routes } = await makeHarness()
    const response = await Effect.runPromise(
      routes.handlePylonClaudeTurnIngestApi(postClaudeTurn(claudeTurnBody()), {}),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      insertedTokenUsage: boolean
      tokensServedDelta: number
      tokenUsageEventRef: string
    }
    expect(body.insertedTokenUsage).toBe(true)
    // input(200) + output(80); cached input is not part of served tokens.
    expect(body.tokensServedDelta).toBe(280)
    expect(body.tokenUsageEventRef).toContain('pylon-claude')

    const event = ledger.events[0]
    expect(event).toMatchObject({
      backendProfile: 'pylon-claude-own-capacity',
      demand: {
        demandKind: 'own_capacity',
        demandSource: 'khala_coding_delegation',
      },
      model: 'openagents/pylon-claude',
      provider: 'pylon-claude-own-capacity',
      usageTruth: 'exact',
    })
    expect(event?.tokenCounts).toMatchObject({
      cacheReadTokens: 11,
      inputTokens: 200,
      outputTokens: 80,
      totalTokens: 280,
    })
    // Owner attribution: the row is owned by the linked OpenAuth user while the
    // account ref stays the local Pylon agent account.
    expect(event?.actor).toMatchObject({
      accountRef: `agent:${agentUserId}`,
      userId: linkedOpenAuthUserId,
    })
    expect(event?.safeMetadata).toMatchObject({
      claudeUsageSplit: {
        cachedInputTokens: 11,
        inputTokens: 200,
        outputTokens: 80,
      },
      usageBasis: 'claude_agent_sdk_turn_completed',
    })
    expect(deltas).toHaveLength(1)
    expect(deltas[0]?.tokensServedDelta).toBe(280)
  })

  test('is idempotent on replay (no second row, no second delta)', async () => {
    const { deltas, ledger, routes } = await makeHarness()
    await Effect.runPromise(
      routes.handlePylonClaudeTurnIngestApi(postClaudeTurn(claudeTurnBody()), {}),
    )
    const second = await Effect.runPromise(
      routes.handlePylonClaudeTurnIngestApi(postClaudeTurn(claudeTurnBody()), {}),
    )
    const secondBody = (await second.json()) as { insertedTokenUsage: boolean }
    expect(secondBody.insertedTokenUsage).toBe(false)
    expect(ledger.events).toHaveLength(1)
    expect(deltas).toHaveLength(1)
  })

  test('rejects a turn for an assignment owned by another agent', async () => {
    const { routes } = await makeHarness({
      assignment: assignmentRecord({ ownerAgentUserId: 'agent-user-someone-else' }),
    })
    const response = await Effect.runPromise(
      routes.handlePylonClaudeTurnIngestApi(postClaudeTurn(claudeTurnBody()), {}),
    )
    expect(response.status).toBe(403)
  })

  test('rejects an unauthenticated turn', async () => {
    const { routes } = await makeHarness()
    const response = await Effect.runPromise(
      routes.handlePylonClaudeTurnIngestApi(
        postClaudeTurn(claudeTurnBody(), 'oa_agent_wrong_token'),
        {},
      ),
    )
    expect(response.status).toBe(401)
  })
})
