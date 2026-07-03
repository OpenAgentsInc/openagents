import { Effect, Match as M, Schema as S } from 'effect'

import {
  AGENT_TOKEN_PREFIX,
  type AgentRegistrationStore,
  type ProgrammaticAgentSession,
  authenticateProgrammaticAgent,
  sha256Hex,
} from './agent-registration'
import {
  ATIF_PINNED_SCHEMA_VERSION,
  AtifStep,
  AtifTrajectory,
  atifTraceTripwire,
  validateAtifTrajectory,
} from './atif-trace-schema'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  unauthorized,
} from './http/responses'
import { decodeUnknownWithSchema, parseJsonUnknown } from './json-boundary'
import {
  type PylonApiAssignmentRecord,
  type PylonApiEventRecord,
  type PylonApiStore,
  pylonApiStoreErrorFromUnknown,
} from './pylon-api'
import { currentIsoTimestamp, randomUuid } from './runtime-primitives'
import {
  type TokenUsageLedgerShape,
  TokenUsageLedgerStorageError,
  TokenUsageLedgerUnsafePayload,
  TokenUsageLedgerValidationError,
} from './token-usage-ledger'
import {
  type TraceStore,
  traceStoreErrorFromUnknown,
} from './trace-store-d1'
import {
  redactTraceValue,
  type TraceRedactionReport,
} from './inference/trace-redaction'

type HttpResponse = globalThis.Response

export const PYLON_CODEX_TURN_INGEST_PATH = '/api/pylon/codex/turns'
export const PYLON_CODEX_LOCAL_USAGE_INGEST_PATH =
  '/api/pylon/codex/local-usage'
export const PYLON_CODEX_EVENT_CHUNK_INGEST_PATH =
  '/api/pylon/codex/event-chunks'
export const PYLON_CODEX_ASSIGNMENT_PROOF_PATH = '/api/pylon/codex/proof'
export const PYLON_CODEX_ASSIGNMENT_TRACE_STATUS_PATH =
  '/api/pylon/codex/trace-status'

// #6388/#6391: the Claude own-capacity coding lane mirrors the Codex token
// ingest. It reuses this module's authenticated, assignment-owned, idempotent
// `token_usage_events` insert path (the same `requireAgent` /
// `requireOwnedAssignment` / `ledger.ingestEvent` / `publishDelta` helpers) but
// records the row under the Claude own-capacity provider/model so the served
// counter and model-mix attribute Claude turns to their own family. The Codex
// constants and route are untouched.
export const PYLON_CLAUDE_TURN_INGEST_PATH = '/api/pylon/claude/turns'
const PYLON_CLAUDE_SCHEMA_VERSION = 'openagents.pylon.claude_turn.v1' as const
const PYLON_CLAUDE_MODEL_NAME = 'openagents/pylon-claude' as const
const PYLON_CLAUDE_PROVIDER = 'pylon-claude-own-capacity' as const

const PYLON_CODEX_SCHEMA_VERSION = 'openagents.pylon.codex_turn.v1' as const
const PYLON_CODEX_EVENT_CHUNK_SCHEMA_VERSION =
  'openagents.pylon.codex_event_chunk.v1' as const
const PYLON_CODEX_MODEL_NAME = 'openagents/pylon-codex' as const
const PYLON_CODEX_PROVIDER = 'pylon-codex-own-capacity' as const
const PYLON_CODEX_DIRECT_LOCAL_SCHEMA_VERSION =
  'openagents.pylon.codex_direct_local_usage.v1' as const
const PYLON_CODEX_DIRECT_LOCAL_MODEL_NAME =
  'openagents/codex-direct-local' as const
const PYLON_CODEX_DIRECT_LOCAL_PROVIDER =
  'pylon-codex-direct-local' as const
const PYLON_CODEX_PRODUCER_SYSTEM = 'omega' as const
const PYLON_CODEX_SOURCE_ROUTE = 'omega_hosted_gemini' as const
const PYLON_CODEX_DEMAND_KIND = 'own_capacity' as const
const PYLON_CODEX_DEMAND_SOURCE = 'khala_coding_delegation' as const
const PYLON_CODEX_DIRECT_LOCAL_PRODUCER_SYSTEM = 'pylon' as const
const PYLON_CODEX_DIRECT_LOCAL_SOURCE_ROUTE =
  'pylon_codex_direct_local' as const
const PYLON_CODEX_DIRECT_LOCAL_DEMAND_SOURCE =
  'direct_local_codex' as const
const PYLON_CODEX_DIRECT_LOCAL_DEMAND_CHANNEL = 'direct_local' as const
const MAX_BODY_BYTES = 8 * 1024 * 1024

const NonEmptyString = S.Trim.check(S.isMinLength(1), S.isMaxLength(512))
const BoundedText = S.String.check(S.isMaxLength(64 * 1024))
const NonNegativeInt = S.Int.check(S.isGreaterThanOrEqualTo(0))
const PositiveInt = S.Int.check(S.isGreaterThanOrEqualTo(1))
const RawCodexEventPayload = S.Record(S.String, S.Unknown)
const PylonModelRoleRef = S.Literals(['architect', 'coder', 'judge', 'advisor'])

class PylonCodexUsage extends S.Class<PylonCodexUsage>('PylonCodexUsage')({
  inputTokens: NonNegativeInt,
  cachedInputTokens: S.optionalKey(NonNegativeInt),
  outputTokens: NonNegativeInt,
  reasoningOutputTokens: S.optionalKey(NonNegativeInt),
}) {}

class PylonCodexDirectLocalUsage extends S.Class<PylonCodexDirectLocalUsage>(
  'PylonCodexDirectLocalUsage',
)({
  inputTokens: NonNegativeInt,
  outputTokens: NonNegativeInt,
  totalTokens: NonNegativeInt,
  usageTruth: S.Literals(['exact', 'estimated']),
}) {}

class PylonCodexTurnItem extends S.Class<PylonCodexTurnItem>(
  'PylonCodexTurnItem',
)({
  ordinal: PositiveInt,
  itemType: S.Literals([
    'agent_message',
    'reasoning',
    'command_execution',
    'file_change',
    'mcp_tool_call',
    'web_search',
    'error',
    'unknown',
  ]),
  status: S.optionalKey(S.String.check(S.isMaxLength(80))),
  message: S.optionalKey(BoundedText),
  reasoningSummary: S.optionalKey(BoundedText),
  commandLabel: S.optionalKey(S.String.check(S.isMaxLength(120))),
  exitCode: S.optionalKey(S.Number),
  outputBytes: S.optionalKey(NonNegativeInt),
  changeCount: S.optionalKey(NonNegativeInt),
  toolName: S.optionalKey(S.String.check(S.isMaxLength(120))),
}) {}

class PylonCodexTurnIngestBody extends S.Class<PylonCodexTurnIngestBody>(
  'PylonCodexTurnIngestBody',
)({
  schemaVersion: S.Literal(PYLON_CODEX_SCHEMA_VERSION),
  assignmentRef: NonEmptyString,
  leaseRef: NonEmptyString,
  pylonRef: NonEmptyString,
  runRef: S.optionalKey(NonEmptyString),
  sessionRef: S.optionalKey(NonEmptyString),
  workspaceRef: S.optionalKey(NonEmptyString),
  turnIndex: PositiveInt,
  observedAt: S.optionalKey(S.String.check(S.isMaxLength(80))),
  roleRef: S.optionalKey(PylonModelRoleRef),
  usage: PylonCodexUsage,
  items: S.Array(PylonCodexTurnItem),
  rawEvents: S.optionalKey(S.Array(RawCodexEventPayload)),
}) {}

class PylonCodexDirectLocalUsageIngestBody extends S.Class<PylonCodexDirectLocalUsageIngestBody>(
  'PylonCodexDirectLocalUsageIngestBody',
)({
  schemaVersion: S.Literal(PYLON_CODEX_DIRECT_LOCAL_SCHEMA_VERSION),
  accountRefHash: NonEmptyString,
  idempotencyKey: NonEmptyString,
  observedAt: S.optionalKey(S.String.check(S.isMaxLength(80))),
  pylonRef: S.optionalKey(NonEmptyString),
  sessionRef: S.optionalKey(NonEmptyString),
  roleRef: S.optionalKey(PylonModelRoleRef),
  usage: PylonCodexDirectLocalUsage,
}) {}

class PylonClaudeTurnIngestBody extends S.Class<PylonClaudeTurnIngestBody>(
  'PylonClaudeTurnIngestBody',
)({
  schemaVersion: S.Literal(PYLON_CLAUDE_SCHEMA_VERSION),
  assignmentRef: NonEmptyString,
  leaseRef: NonEmptyString,
  pylonRef: NonEmptyString,
  runRef: S.optionalKey(NonEmptyString),
  sessionRef: S.optionalKey(NonEmptyString),
  workspaceRef: S.optionalKey(NonEmptyString),
  turnIndex: PositiveInt,
  observedAt: S.optionalKey(S.String.check(S.isMaxLength(80))),
  roleRef: S.optionalKey(PylonModelRoleRef),
  usage: PylonCodexUsage,
  items: S.optionalKey(S.Array(PylonCodexTurnItem)),
}) {}

class PylonCodexEventChunkIngestBody extends S.Class<PylonCodexEventChunkIngestBody>(
  'PylonCodexEventChunkIngestBody',
)({
  schemaVersion: S.Literal(PYLON_CODEX_EVENT_CHUNK_SCHEMA_VERSION),
  assignmentRef: NonEmptyString,
  leaseRef: NonEmptyString,
  pylonRef: NonEmptyString,
  runRef: S.optionalKey(NonEmptyString),
  sessionRef: S.optionalKey(NonEmptyString),
  workspaceRef: S.optionalKey(NonEmptyString),
  turnIndex: PositiveInt,
  chunkIndex: PositiveInt,
  observedAt: S.optionalKey(S.String.check(S.isMaxLength(80))),
  rawEvents: S.Array(RawCodexEventPayload),
  items: S.optionalKey(S.Array(PylonCodexTurnItem)),
}) {}

export type PylonCodexTurnIngest =
  typeof PylonCodexTurnIngestBody.Type

export type PylonCodexTokenCounts = Readonly<{
  cacheReadTokens: number
  inputTokens: number
  outputTokens: number
  reasoningTokens: number
  totalTokens: number
}>

type PylonCodexTurnIngestDependencies<Bindings> = Readonly<{
  agentStore: (env: Bindings) => AgentRegistrationStore
  ledger: (env: Bindings) => TokenUsageLedgerShape
  makeId?: () => string
  nowIso?: () => string
  pylonStore: (
    env: Bindings,
  ) => Pick<PylonApiStore, 'listEventsForAssignment' | 'readAssignment'>
  proofStore?: (env: Bindings) => PylonCodexAssignmentProofStore
  traceStatusStore?: (
    env: Bindings,
  ) => PylonCodexAssignmentTraceStatusStore
  rawEventChunkStore?: (
    env: Bindings,
  ) => PylonCodexRawEventChunkStore | undefined
  rawEventStore?: (env: Bindings) => PylonCodexRawEventStore | undefined
  publishDelta?: (
    env: Bindings,
    input: Readonly<{
      eventRef: string
      observedAt: string
      tokensServedDelta: number
    }>,
  ) => Effect.Effect<void, unknown>
  traceStore: (env: Bindings) => TraceStore
}>

export type PylonCodexAssignmentProof = Readonly<{
  schemaVersion: 'openagents.pylon.codex_assignment_proof.v1'
  assignmentRef: string
  pylonRef: string
  owner: Readonly<{
    agentUserRef: string
    openauthUserRef: string
  }>
  tokenUsage: Readonly<{
    rowCount: number
    refs: ReadonlyArray<string>
    provider: typeof PYLON_CODEX_PROVIDER
    model: typeof PYLON_CODEX_MODEL_NAME
    usageTruth: 'exact'
    demandKind: typeof PYLON_CODEX_DEMAND_KIND
    demandSource: typeof PYLON_CODEX_DEMAND_SOURCE
    inputTokens: number
    outputTokens: number
    reasoningTokens: number
    cacheReadTokens: number
    totalTokens: number
  }>
  traces: Readonly<{
    count: number
    visibility: 'owner_only'
    schemaVersion: typeof ATIF_PINNED_SCHEMA_VERSION
    refs: ReadonlyArray<string>
  }>
  rawEvents: Readonly<{
    count: number
    eventCount: number
    byteLength: number
    visibility: 'owner_only'
    refs: ReadonlyArray<string>
  }>
  closeoutPolicy: PylonCodexAssignmentCloseoutPolicy
  generatedAt: string
}>

export type PylonCodexAssignmentCloseoutPolicy = Readonly<{
  paymentMode: 'no-spend' | 'paid' | 'unknown'
  payoutClaimAllowed: boolean | null
  settlementState: 'not_applicable' | 'pending' | 'settled' | 'unknown'
  source: 'worker_closeout_event' | 'unavailable'
}>

export type PylonCodexAssignmentTraceStatus = Readonly<{
  schemaVersion: 'openagents.pylon.codex_assignment_trace_status.v1'
  assignmentRef: string
  pylonRef: string
  owner: Readonly<{
    agentUserRef: string
    openauthUserRef: string
  }>
  lifecycle: Readonly<{
    state: PylonApiAssignmentRecord['state']
    createdAt: string
    updatedAt: string
    acceptedWorkRefs: ReadonlyArray<string>
    artifactRefs: ReadonlyArray<string>
    closeoutRefs: ReadonlyArray<string>
    proofRefs: ReadonlyArray<string>
    rejectionRefs: ReadonlyArray<string>
  }>
  events: Readonly<{
    count: number
    progressCount: number
    latestEventKind: string | null
    latestStatus: string | null
    latestObservedAt: string | null
    latestProgressStatus: string | null
    latestProgressObservedAt: string | null
  }>
  tokenUsage: PylonCodexAssignmentProof['tokenUsage'] &
    Readonly<{ status: 'pending' | 'recorded' }>
  traces: Readonly<{
    count: number
    visibility: 'owner_only'
    schemaVersion: typeof ATIF_PINNED_SCHEMA_VERSION
    latestTraceUuid: string | null
    finalTraceUuid: string | null
    refs: ReadonlyArray<string>
  }>
  rawEventChunks: Readonly<{
    count: number
    eventCount: number
    byteLength: number
    latestChunkRef: string | null
    latestObservedAt: string | null
    visibility: 'owner_only'
  }>
  rawEvents: Readonly<{
    count: number
    eventCount: number
    byteLength: number
    latestRawEventRef: string | null
    latestObservedAt: string | null
    visibility: 'owner_only'
    refs: ReadonlyArray<string>
  }>
  closeoutPolicy: PylonCodexAssignmentCloseoutPolicy
  progress: Readonly<{
    state:
      | 'assignment_created'
      | 'streaming_chunks'
      | 'final_trace_recorded'
      | 'tokens_recorded'
      | 'closed_out'
      | 'rejected'
    closeoutReady: boolean
    hasLiveChunks: boolean
    hasFinalTrace: boolean
    hasTokenUsage: boolean
    missingReadinessRefs: ReadonlyArray<string>
  }>
  generatedAt: string
}>

export type PylonCodexAssignmentProofStore = Readonly<{
  readAssignmentProof: (
    input: Readonly<{
      assignmentRef: string
      ownerAgentUserId: string
      ownerUserId: string
      pylonRef: string
      nowIso: string
      closeoutPolicy: PylonCodexAssignmentCloseoutPolicy
    }>,
  ) => Promise<PylonCodexAssignmentProof>
}>

export type PylonCodexAssignmentTraceStatusStore = Readonly<{
  readAssignmentTraceStatus: (
    input: Readonly<{
      assignment: PylonApiAssignmentRecord
      ownerAgentUserId: string
      ownerUserId: string
      nowIso: string
      closeoutPolicy: PylonCodexAssignmentCloseoutPolicy
    }>,
  ) => Promise<PylonCodexAssignmentTraceStatus>
}>

export type PylonCodexRawEventStoreInput = Readonly<{
  assignmentRef: string
  digest: string
  eventCount: number
  eventsJson: string
  leaseRef: string
  observedAt: string
  ownerUserId: string
  pylonRef: string
  runRef: string | null
  sessionRef: string | null
  turnIndex: number
  workspaceRef: string | null
}>

export type PylonCodexRawEventStoreResult = Readonly<{
  byteLength: number
  created: boolean
  ref: string
  r2Key: string
}>

export type PylonCodexRawEventChunkStoreInput = Readonly<{
  assignmentRef: string
  chunkIndex: number
  digest: string
  eventCount: number
  eventsJson: string
  leaseRef: string
  observedAt: string
  ownerUserId: string
  pylonRef: string
  runRef: string | null
  sessionRef: string | null
  turnIndex: number
  workspaceRef: string | null
}>

export type PylonCodexRawEventChunkStoreResult = Readonly<{
  byteLength: number
  created: boolean
  ref: string
  r2Key: string
}>

export type PylonCodexRawEventStore = Readonly<{
  putTurnEvents: (
    input: PylonCodexRawEventStoreInput,
  ) => Promise<PylonCodexRawEventStoreResult>
}>

export type PylonCodexRawEventChunkStore = Readonly<{
  putEventChunk: (
    input: PylonCodexRawEventChunkStoreInput,
  ) => Promise<PylonCodexRawEventChunkStoreResult>
}>

export const pylonCodexRawEventR2Key = (
  input: Pick<
    PylonCodexRawEventStoreInput,
    'assignmentRef' | 'digest' | 'turnIndex'
  >,
): string =>
  [
    'private',
    'pylon-codex-raw-events',
    input.assignmentRef,
    `turn-${input.turnIndex}`,
    `${input.digest}.json`,
  ].join('/')

export const pylonCodexRawEventRef = (digest: string): string =>
  `raw.pylon_codex.${digest.slice(0, 32)}`

export const pylonCodexRawEventChunkR2Key = (
  input: Pick<
    PylonCodexRawEventChunkStoreInput,
    'assignmentRef' | 'chunkIndex' | 'digest' | 'turnIndex'
  >,
): string =>
  [
    'private',
    'pylon-codex-raw-event-chunks',
    input.assignmentRef,
    `turn-${input.turnIndex}`,
    `chunk-${input.chunkIndex}`,
    `${input.digest}.json`,
  ].join('/')

export const pylonCodexRawEventChunkRef = (digest: string): string =>
  `raw_chunk.pylon_codex.${digest.slice(0, 32)}`

type PylonCodexRawEventMetadataRow = Readonly<{
  byte_length: number
  event_count: number
  r2_key: string
  raw_event_ref: string
}>

type PylonCodexRawEventChunkMetadataRow = Readonly<{
  byte_length: number
  chunk_ref: string
  event_count: number
  r2_key: string
}>

type PylonCodexTokenProofRow = Readonly<{
  row_count: number
  input_tokens: number | null
  output_tokens: number | null
  reasoning_tokens: number | null
  cache_read_tokens: number | null
  total_tokens: number | null
}>

type PylonCodexTokenUsageRefRow = Readonly<{
  id: string
}>

type PylonCodexTraceProofRow = Readonly<{
  trace_uuid: string
  trajectory_id?: string
}>

type PylonCodexCountProofRow = Readonly<{
  row_count: number
}>

type PylonCodexRawEventAggregateProofRow = Readonly<{
  row_count: number
  event_count: number | null
  byte_length: number | null
}>

type PylonCodexRawEventProofRow = Readonly<{
  raw_event_ref: string
  observed_at?: string
}>

type PylonCodexRawEventChunkProofRow = Readonly<{
  chunk_ref: string
  observed_at?: string
}>

type PylonCodexAssignmentEventAggregateRow = Readonly<{
  event_count: number
  progress_count: number
  latest_event_kind: string | null
  latest_status: string | null
  latest_observed_at: string | null
  latest_progress_status: string | null
  latest_progress_observed_at: string | null
}>

const boundedProofRefs = (
  rows: ReadonlyArray<{ readonly [key: string]: unknown }>,
  key: string,
): ReadonlyArray<string> =>
  rows
    .map(row => String(row[key] ?? '').trim())
    .filter(ref => ref !== '')
    .slice(0, 100)

const closeoutPolicyFromEvents = (
  events: ReadonlyArray<PylonApiEventRecord>,
  assignment: PylonApiAssignmentRecord,
): PylonCodexAssignmentCloseoutPolicy => {
  const closeoutEvent = events.find(row => row.eventKind === 'worker_closeout')
  if (closeoutEvent === undefined) {
    return {
      paymentMode: 'unknown',
      payoutClaimAllowed: null,
      settlementState: 'unknown',
      source: 'unavailable',
    }
  }
  const body = closeoutEvent.eventBody
  const codingAssignment = assignment.codingAssignment as
    | { readonly budget?: { readonly paymentMode?: unknown } }
    | null
  const paymentMode =
    body.paymentMode === 'no-spend' || body.paymentMode === 'paid'
      ? body.paymentMode
      : codingAssignment?.budget?.paymentMode === 'buyer_funded'
        ? 'paid'
        : 'no-spend'
  const settlementState =
    body.settlementState === 'not_applicable' ||
    body.settlementState === 'pending' ||
    body.settlementState === 'settled'
      ? body.settlementState
      : paymentMode === 'no-spend'
        ? 'not_applicable'
      : 'unknown'
  const payoutClaimAllowed =
    typeof body.payoutClaimAllowed === 'boolean'
      ? body.payoutClaimAllowed
      : paymentMode === 'no-spend'
        ? false
      : null
  return {
    paymentMode,
    payoutClaimAllowed,
    settlementState,
    source: 'worker_closeout_event',
  }
}

export const makeD1PylonCodexAssignmentProofStore = (
  db: D1Database,
): PylonCodexAssignmentProofStore & PylonCodexAssignmentTraceStatusStore => ({
  readAssignmentProof: async input => {
    const tokenRow = await db
      .prepare(
        `
          SELECT
            COUNT(*) AS row_count,
            COALESCE(SUM(input_tokens), 0) AS input_tokens,
            COALESCE(SUM(output_tokens), 0) AS output_tokens,
            COALESCE(SUM(reasoning_tokens), 0) AS reasoning_tokens,
            COALESCE(SUM(cache_read_tokens), 0) AS cache_read_tokens,
            COALESCE(SUM(total_tokens), 0) AS total_tokens
          FROM token_usage_events
          WHERE provider = ?
            AND model = ?
            AND usage_truth = 'exact'
            AND demand_kind = ?
            AND demand_source = ?
            AND task_ref = ?
            AND account_ref = ?
            AND actor_user_id = ?
        `,
      )
      .bind(
        PYLON_CODEX_PROVIDER,
        PYLON_CODEX_MODEL_NAME,
        PYLON_CODEX_DEMAND_KIND,
        PYLON_CODEX_DEMAND_SOURCE,
        input.assignmentRef,
        `agent:${input.ownerAgentUserId}`,
        input.ownerUserId,
      )
      .first<PylonCodexTokenProofRow>()

    const tokenRows = await db
      .prepare(
        `
          SELECT id
          FROM token_usage_events
          WHERE provider = ?
            AND model = ?
            AND usage_truth = 'exact'
            AND demand_kind = ?
            AND demand_source = ?
            AND task_ref = ?
            AND account_ref = ?
            AND actor_user_id = ?
          ORDER BY observed_at ASC, id ASC
          LIMIT 100
        `,
      )
      .bind(
        PYLON_CODEX_PROVIDER,
        PYLON_CODEX_MODEL_NAME,
        PYLON_CODEX_DEMAND_KIND,
        PYLON_CODEX_DEMAND_SOURCE,
        input.assignmentRef,
        `agent:${input.ownerAgentUserId}`,
        input.ownerUserId,
      )
      .all<PylonCodexTokenUsageRefRow>()

    const traceTrajectoryPrefix = `pylon_codex:${input.assignmentRef}:`
    const traceFilter = `
      owner_user_id = ?
      AND agent_ref = ?
      AND visibility = 'owner_only'
      AND schema_version = ?
      AND demand_kind = ?
      AND demand_source = ?
      AND substr(trajectory_id, 1, ?) = ?
    `
    const traceBindings = [
      input.ownerUserId,
      `agent:${input.ownerAgentUserId}`,
      ATIF_PINNED_SCHEMA_VERSION,
      PYLON_CODEX_DEMAND_KIND,
      PYLON_CODEX_DEMAND_SOURCE,
      traceTrajectoryPrefix.length,
      traceTrajectoryPrefix,
    ] as const

    const traceCountRow = await db
      .prepare(`SELECT COUNT(*) AS row_count FROM agent_traces WHERE ${traceFilter}`)
      .bind(...traceBindings)
      .first<PylonCodexCountProofRow>()

    const traceRows = await db
      .prepare(
        `
          SELECT trace_uuid
          FROM agent_traces
          WHERE ${traceFilter}
          ORDER BY created_at DESC
          LIMIT 100
        `,
      )
      .bind(...traceBindings)
      .all<PylonCodexTraceProofRow>()

    const rawEventFilter = `
      owner_user_id = ?
      AND assignment_ref = ?
      AND pylon_ref = ?
      AND demand_kind = ?
      AND demand_source = ?
    `
    const rawEventBindings = [
      input.ownerUserId,
      input.assignmentRef,
      input.pylonRef,
      PYLON_CODEX_DEMAND_KIND,
      PYLON_CODEX_DEMAND_SOURCE,
    ] as const

    const rawAggregateRow = await db
      .prepare(
        `
          SELECT
            COUNT(*) AS row_count,
            COALESCE(SUM(event_count), 0) AS event_count,
            COALESCE(SUM(byte_length), 0) AS byte_length
          FROM pylon_codex_raw_events
          WHERE ${rawEventFilter}
        `,
      )
      .bind(...rawEventBindings)
      .first<PylonCodexRawEventAggregateProofRow>()

    const rawRows = await db
      .prepare(
        `
          SELECT raw_event_ref
          FROM pylon_codex_raw_events
          WHERE ${rawEventFilter}
          ORDER BY turn_index ASC
          LIMIT 100
        `,
      )
      .bind(...rawEventBindings)
      .all<PylonCodexRawEventProofRow>()

    const traces = traceRows.results ?? []
    const rawEvents = rawRows.results ?? []
    const tokenRefs = tokenRows.results ?? []

    return {
      schemaVersion: 'openagents.pylon.codex_assignment_proof.v1',
      assignmentRef: input.assignmentRef,
      pylonRef: input.pylonRef,
      owner: {
        agentUserRef: `agent:${input.ownerAgentUserId}`,
        openauthUserRef: input.ownerUserId,
      },
      tokenUsage: {
        rowCount: Number(tokenRow?.row_count ?? 0),
        refs: boundedProofRefs(tokenRefs, 'id'),
        provider: PYLON_CODEX_PROVIDER,
        model: PYLON_CODEX_MODEL_NAME,
        usageTruth: 'exact',
        demandKind: PYLON_CODEX_DEMAND_KIND,
        demandSource: PYLON_CODEX_DEMAND_SOURCE,
        inputTokens: Number(tokenRow?.input_tokens ?? 0),
        outputTokens: Number(tokenRow?.output_tokens ?? 0),
        reasoningTokens: Number(tokenRow?.reasoning_tokens ?? 0),
        cacheReadTokens: Number(tokenRow?.cache_read_tokens ?? 0),
        totalTokens: Number(tokenRow?.total_tokens ?? 0),
      },
      traces: {
        count: Number(traceCountRow?.row_count ?? 0),
        visibility: 'owner_only',
        schemaVersion: ATIF_PINNED_SCHEMA_VERSION,
        refs: boundedProofRefs(traces, 'trace_uuid'),
      },
      rawEvents: {
        count: Number(rawAggregateRow?.row_count ?? 0),
        eventCount: Number(rawAggregateRow?.event_count ?? 0),
        byteLength: Number(rawAggregateRow?.byte_length ?? 0),
        visibility: 'owner_only',
        refs: boundedProofRefs(rawEvents, 'raw_event_ref'),
      },
      closeoutPolicy: input.closeoutPolicy,
      generatedAt: input.nowIso,
    }
  },
  readAssignmentTraceStatus: async input => {
    const tokenRow = await db
      .prepare(
        `
          SELECT
            COUNT(*) AS row_count,
            COALESCE(SUM(input_tokens), 0) AS input_tokens,
            COALESCE(SUM(output_tokens), 0) AS output_tokens,
            COALESCE(SUM(reasoning_tokens), 0) AS reasoning_tokens,
            COALESCE(SUM(cache_read_tokens), 0) AS cache_read_tokens,
            COALESCE(SUM(total_tokens), 0) AS total_tokens
          FROM token_usage_events
          WHERE provider = ?
            AND model = ?
            AND usage_truth = 'exact'
            AND demand_kind = ?
            AND demand_source = ?
            AND task_ref = ?
            AND account_ref = ?
            AND actor_user_id = ?
        `,
      )
      .bind(
        PYLON_CODEX_PROVIDER,
        PYLON_CODEX_MODEL_NAME,
        PYLON_CODEX_DEMAND_KIND,
        PYLON_CODEX_DEMAND_SOURCE,
        input.assignment.assignmentRef,
        `agent:${input.ownerAgentUserId}`,
        input.ownerUserId,
      )
      .first<PylonCodexTokenProofRow>()

    const tokenRows = await db
      .prepare(
        `
          SELECT id
          FROM token_usage_events
          WHERE provider = ?
            AND model = ?
            AND usage_truth = 'exact'
            AND demand_kind = ?
            AND demand_source = ?
            AND task_ref = ?
            AND account_ref = ?
            AND actor_user_id = ?
          ORDER BY observed_at ASC, id ASC
          LIMIT 100
        `,
      )
      .bind(
        PYLON_CODEX_PROVIDER,
        PYLON_CODEX_MODEL_NAME,
        PYLON_CODEX_DEMAND_KIND,
        PYLON_CODEX_DEMAND_SOURCE,
        input.assignment.assignmentRef,
        `agent:${input.ownerAgentUserId}`,
        input.ownerUserId,
      )
      .all<PylonCodexTokenUsageRefRow>()

    const eventRow = await db
      .prepare(
        `
          SELECT
            COUNT(*) AS event_count,
            COALESCE(
              SUM(CASE WHEN event_kind = 'assignment_progress' THEN 1 ELSE 0 END),
              0
            ) AS progress_count,
            (
              SELECT event_kind
              FROM pylon_api_events
              WHERE assignment_ref = ?
                AND owner_agent_user_id = ?
                AND archived_at IS NULL
              ORDER BY created_at DESC
              LIMIT 1
            ) AS latest_event_kind,
            (
              SELECT status
              FROM pylon_api_events
              WHERE assignment_ref = ?
                AND owner_agent_user_id = ?
                AND archived_at IS NULL
              ORDER BY created_at DESC
              LIMIT 1
            ) AS latest_status,
            (
              SELECT status
              FROM pylon_api_events
              WHERE assignment_ref = ?
                AND owner_agent_user_id = ?
                AND event_kind = 'assignment_progress'
                AND archived_at IS NULL
              ORDER BY created_at DESC
              LIMIT 1
            ) AS latest_progress_status,
            (
              SELECT created_at
              FROM pylon_api_events
              WHERE assignment_ref = ?
                AND owner_agent_user_id = ?
                AND event_kind = 'assignment_progress'
                AND archived_at IS NULL
              ORDER BY created_at DESC
              LIMIT 1
            ) AS latest_progress_observed_at,
            MAX(created_at) AS latest_observed_at
          FROM pylon_api_events
          WHERE assignment_ref = ?
            AND owner_agent_user_id = ?
            AND archived_at IS NULL
        `,
      )
      .bind(
        input.assignment.assignmentRef,
        input.ownerAgentUserId,
        input.assignment.assignmentRef,
        input.ownerAgentUserId,
        input.assignment.assignmentRef,
        input.ownerAgentUserId,
        input.assignment.assignmentRef,
        input.ownerAgentUserId,
        input.assignment.assignmentRef,
        input.ownerAgentUserId,
      )
      .first<PylonCodexAssignmentEventAggregateRow>()

    const traceTrajectoryPrefix = `pylon_codex:${input.assignment.assignmentRef}:`
    const traceFilter = `
      owner_user_id = ?
      AND agent_ref = ?
      AND visibility = 'owner_only'
      AND schema_version = ?
      AND demand_kind = ?
      AND demand_source = ?
      AND substr(trajectory_id, 1, ?) = ?
    `
    const traceBindings = [
      input.ownerUserId,
      `agent:${input.ownerAgentUserId}`,
      ATIF_PINNED_SCHEMA_VERSION,
      PYLON_CODEX_DEMAND_KIND,
      PYLON_CODEX_DEMAND_SOURCE,
      traceTrajectoryPrefix.length,
      traceTrajectoryPrefix,
    ] as const

    const traceCountRow = await db
      .prepare(`SELECT COUNT(*) AS row_count FROM agent_traces WHERE ${traceFilter}`)
      .bind(...traceBindings)
      .first<PylonCodexCountProofRow>()

    const traceRows = await db
      .prepare(
        `
          SELECT trace_uuid, trajectory_id
          FROM agent_traces
          WHERE ${traceFilter}
          ORDER BY created_at DESC
          LIMIT 100
        `,
      )
      .bind(...traceBindings)
      .all<PylonCodexTraceProofRow>()

    const rawEventFilter = `
      owner_user_id = ?
      AND assignment_ref = ?
      AND pylon_ref = ?
      AND demand_kind = ?
      AND demand_source = ?
    `
    const rawEventBindings = [
      input.ownerUserId,
      input.assignment.assignmentRef,
      input.assignment.pylonRef,
      PYLON_CODEX_DEMAND_KIND,
      PYLON_CODEX_DEMAND_SOURCE,
    ] as const

    const rawChunkAggregateRow = await db
      .prepare(
        `
          SELECT
            COUNT(*) AS row_count,
            COALESCE(SUM(event_count), 0) AS event_count,
            COALESCE(SUM(byte_length), 0) AS byte_length
          FROM pylon_codex_raw_event_chunks
          WHERE ${rawEventFilter}
        `,
      )
      .bind(...rawEventBindings)
      .first<PylonCodexRawEventAggregateProofRow>()

    const latestChunkRow = await db
      .prepare(
        `
          SELECT chunk_ref, observed_at
          FROM pylon_codex_raw_event_chunks
          WHERE ${rawEventFilter}
          ORDER BY turn_index DESC, chunk_index DESC
          LIMIT 1
        `,
      )
      .bind(...rawEventBindings)
      .first<PylonCodexRawEventChunkProofRow>()

    const rawAggregateRow = await db
      .prepare(
        `
          SELECT
            COUNT(*) AS row_count,
            COALESCE(SUM(event_count), 0) AS event_count,
            COALESCE(SUM(byte_length), 0) AS byte_length
          FROM pylon_codex_raw_events
          WHERE ${rawEventFilter}
        `,
      )
      .bind(...rawEventBindings)
      .first<PylonCodexRawEventAggregateProofRow>()

    const rawRows = await db
      .prepare(
        `
          SELECT raw_event_ref, observed_at
          FROM pylon_codex_raw_events
          WHERE ${rawEventFilter}
          ORDER BY turn_index ASC
          LIMIT 100
        `,
      )
      .bind(...rawEventBindings)
      .all<PylonCodexRawEventProofRow>()

    const traces = traceRows.results ?? []
    const rawEvents = rawRows.results ?? []
    const tokenRefs = tokenRows.results ?? []
    const finalTraceUuid =
      traces.find(row => !String(row.trajectory_id ?? '').includes(':chunk:'))
        ?.trace_uuid ?? null
    const latestTraceUuid = traces[0]?.trace_uuid ?? null
    const tokenRowCount = Number(tokenRow?.row_count ?? 0)
    const chunkCount = Number(rawChunkAggregateRow?.row_count ?? 0)
    const rawEventCount = Number(rawAggregateRow?.row_count ?? 0)
    const hasTokenUsage = tokenRowCount > 0
    const hasLiveChunks = chunkCount > 0
    const hasFinalTrace = finalTraceUuid !== null
    const rejected = input.assignment.rejectionRefs.length > 0
    const closedOut =
      input.assignment.closeoutRefs.length > 0 ||
      input.assignment.acceptedWorkRefs.length > 0
    const progressState =
      rejected
        ? 'rejected'
        : closedOut
          ? 'closed_out'
          : hasTokenUsage
            ? 'tokens_recorded'
            : hasFinalTrace
              ? 'final_trace_recorded'
              : hasLiveChunks
                ? 'streaming_chunks'
                : 'assignment_created'
    const missingReadinessRefs = [
      hasLiveChunks ? null : 'status.pylon_codex.raw_event_chunks.pending',
      hasFinalTrace ? null : 'status.pylon_codex.final_trace.pending',
      hasTokenUsage ? null : 'status.pylon_codex.token_usage.pending',
      closedOut ? null : 'status.pylon_codex.closeout.pending',
    ].filter((ref): ref is string => ref !== null)

    return {
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
        count: Number(eventRow?.event_count ?? 0),
        latestEventKind: eventRow?.latest_event_kind ?? null,
        latestObservedAt: eventRow?.latest_observed_at ?? null,
        latestProgressObservedAt:
          eventRow?.latest_progress_observed_at ?? null,
        latestProgressStatus: eventRow?.latest_progress_status ?? null,
        latestStatus: eventRow?.latest_status ?? null,
        progressCount: Number(eventRow?.progress_count ?? 0),
      },
      tokenUsage: {
        rowCount: tokenRowCount,
        refs: boundedProofRefs(tokenRefs, 'id'),
        provider: PYLON_CODEX_PROVIDER,
        model: PYLON_CODEX_MODEL_NAME,
        usageTruth: 'exact',
        demandKind: PYLON_CODEX_DEMAND_KIND,
        demandSource: PYLON_CODEX_DEMAND_SOURCE,
        inputTokens: Number(tokenRow?.input_tokens ?? 0),
        outputTokens: Number(tokenRow?.output_tokens ?? 0),
        reasoningTokens: Number(tokenRow?.reasoning_tokens ?? 0),
        cacheReadTokens: Number(tokenRow?.cache_read_tokens ?? 0),
        totalTokens: Number(tokenRow?.total_tokens ?? 0),
        status: hasTokenUsage ? 'recorded' : 'pending',
      },
      traces: {
        count: Number(traceCountRow?.row_count ?? 0),
        finalTraceUuid,
        latestTraceUuid,
        refs: boundedProofRefs(traces, 'trace_uuid'),
        schemaVersion: ATIF_PINNED_SCHEMA_VERSION,
        visibility: 'owner_only',
      },
      rawEventChunks: {
        byteLength: Number(rawChunkAggregateRow?.byte_length ?? 0),
        count: chunkCount,
        eventCount: Number(rawChunkAggregateRow?.event_count ?? 0),
        latestChunkRef: latestChunkRow?.chunk_ref ?? null,
        latestObservedAt: latestChunkRow?.observed_at ?? null,
        visibility: 'owner_only',
      },
      rawEvents: {
        byteLength: Number(rawAggregateRow?.byte_length ?? 0),
        count: rawEventCount,
        eventCount: Number(rawAggregateRow?.event_count ?? 0),
        latestObservedAt: rawEvents[rawEvents.length - 1]?.observed_at ?? null,
        latestRawEventRef:
          rawEvents[rawEvents.length - 1]?.raw_event_ref ?? null,
        refs: boundedProofRefs(rawEvents, 'raw_event_ref'),
        visibility: 'owner_only',
      },
      closeoutPolicy: input.closeoutPolicy,
      progress: {
        closeoutReady: closedOut,
        hasFinalTrace,
        hasLiveChunks,
        hasTokenUsage,
        missingReadinessRefs,
        state: progressState,
      },
      generatedAt: input.nowIso,
    }
  },
})

export const makeD1R2PylonCodexRawEventStore = (
  db: D1Database,
  bucket: R2Bucket,
): PylonCodexRawEventStore => ({
  putTurnEvents: async input => {
    const ref = pylonCodexRawEventRef(input.digest)
    const existing = await db
      .prepare(
        `
          SELECT raw_event_ref, r2_key, byte_length, event_count
          FROM pylon_codex_raw_events
          WHERE raw_event_ref = ?
          LIMIT 1
        `,
      )
      .bind(ref)
      .first<PylonCodexRawEventMetadataRow>()
    if (existing !== null) {
      return {
        byteLength: Number(existing.byte_length),
        created: false,
        ref: existing.raw_event_ref,
        r2Key: existing.r2_key,
      }
    }

    const r2Key = pylonCodexRawEventR2Key(input)
    const existingObject = await bucket.head(r2Key)
    const byteLength = new TextEncoder().encode(input.eventsJson).byteLength
    if (existingObject === null) {
      await bucket.put(r2Key, input.eventsJson, {
        customMetadata: {
          assignmentRef: input.assignmentRef,
          demandKind: PYLON_CODEX_DEMAND_KIND,
          demandSource: PYLON_CODEX_DEMAND_SOURCE,
          ownerUserId: input.ownerUserId,
          rawEventRef: ref,
          turnIndex: String(input.turnIndex),
        },
        httpMetadata: { contentType: 'application/json; charset=utf-8' },
      })
    }

    const now = currentIsoTimestamp()
    const result = await db
      .prepare(
        `
          INSERT OR IGNORE INTO pylon_codex_raw_events (
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
            demand_source
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        `,
      )
      .bind(
        ref,
        input.assignmentRef,
        input.leaseRef,
        input.pylonRef,
        input.ownerUserId,
        input.runRef,
        input.sessionRef,
        input.workspaceRef,
        input.turnIndex,
        input.eventCount,
        byteLength,
        input.digest,
        r2Key,
        input.observedAt,
        now,
        now,
        PYLON_CODEX_DEMAND_KIND,
        PYLON_CODEX_DEMAND_SOURCE,
      )
      .run()

    return {
      byteLength,
      created: (result.meta.changes ?? 0) > 0,
      ref,
      r2Key,
    }
  },
})

export const makeD1R2PylonCodexRawEventChunkStore = (
  db: D1Database,
  bucket: R2Bucket,
): PylonCodexRawEventChunkStore => ({
  putEventChunk: async input => {
    const ref = pylonCodexRawEventChunkRef(input.digest)
    const existing = await db
      .prepare(
        `
          SELECT chunk_ref, r2_key, byte_length, event_count
          FROM pylon_codex_raw_event_chunks
          WHERE chunk_ref = ?
          LIMIT 1
        `,
      )
      .bind(ref)
      .first<PylonCodexRawEventChunkMetadataRow>()
    if (existing !== null) {
      return {
        byteLength: Number(existing.byte_length),
        created: false,
        ref: existing.chunk_ref,
        r2Key: existing.r2_key,
      }
    }

    const r2Key = pylonCodexRawEventChunkR2Key(input)
    const existingObject = await bucket.head(r2Key)
    const byteLength = new TextEncoder().encode(input.eventsJson).byteLength
    if (existingObject === null) {
      await bucket.put(r2Key, input.eventsJson, {
        customMetadata: {
          assignmentRef: input.assignmentRef,
          chunkIndex: String(input.chunkIndex),
          demandKind: PYLON_CODEX_DEMAND_KIND,
          demandSource: PYLON_CODEX_DEMAND_SOURCE,
          ownerUserId: input.ownerUserId,
          rawEventChunkRef: ref,
          turnIndex: String(input.turnIndex),
        },
        httpMetadata: { contentType: 'application/json; charset=utf-8' },
      })
    }

    const now = currentIsoTimestamp()
    const result = await db
      .prepare(
        `
          INSERT OR IGNORE INTO pylon_codex_raw_event_chunks (
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
            demand_source
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        `,
      )
      .bind(
        ref,
        input.assignmentRef,
        input.leaseRef,
        input.pylonRef,
        input.ownerUserId,
        input.runRef,
        input.sessionRef,
        input.workspaceRef,
        input.turnIndex,
        input.chunkIndex,
        input.eventCount,
        byteLength,
        input.digest,
        r2Key,
        input.observedAt,
        now,
        now,
        PYLON_CODEX_DEMAND_KIND,
        PYLON_CODEX_DEMAND_SOURCE,
      )
      .run()

    return {
      byteLength,
      created: (result.meta.changes ?? 0) > 0,
      ref,
      r2Key,
    }
  },
})

class PylonCodexUnauthorized extends S.TaggedErrorClass<PylonCodexUnauthorized>()(
  'PylonCodexUnauthorized',
  {},
) {}

class PylonCodexForbidden extends S.TaggedErrorClass<PylonCodexForbidden>()(
  'PylonCodexForbidden',
  { reason: S.String },
) {}

class PylonCodexNotFound extends S.TaggedErrorClass<PylonCodexNotFound>()(
  'PylonCodexNotFound',
  { reason: S.String },
) {}

class PylonCodexValidationError extends S.TaggedErrorClass<PylonCodexValidationError>()(
  'PylonCodexValidationError',
  { reason: S.String },
) {}

class PylonCodexTraceRejected extends S.TaggedErrorClass<PylonCodexTraceRejected>()(
  'PylonCodexTraceRejected',
  { findings: S.Array(S.String), redactionReport: S.optionalKey(S.Unknown) },
) {}

class PylonCodexStorageError extends S.TaggedErrorClass<PylonCodexStorageError>()(
  'PylonCodexStorageError',
  { operation: S.String, reason: S.String },
) {}

type PylonCodexRouteError =
  | PylonCodexForbidden
  | PylonCodexNotFound
  | PylonCodexStorageError
  | PylonCodexTraceRejected
  | PylonCodexUnauthorized
  | PylonCodexValidationError

const routeErrorResponse = (error: PylonCodexRouteError): HttpResponse =>
  M.value(error).pipe(
    M.tags({
      PylonCodexUnauthorized: () => unauthorized(),
      PylonCodexForbidden: error =>
        noStoreJsonResponse(
          { error: 'pylon_codex_forbidden', reason: error.reason },
          { status: 403 },
        ),
      PylonCodexNotFound: error =>
        noStoreJsonResponse(
          { error: 'pylon_codex_not_found', reason: error.reason },
          { status: 404 },
        ),
      PylonCodexValidationError: error =>
        noStoreJsonResponse(
          { error: 'pylon_codex_validation_error', reason: error.reason },
          { status: 400 },
        ),
      PylonCodexTraceRejected: error =>
        noStoreJsonResponse(
          {
            error: 'pylon_codex_trace_rejected',
            findings: error.findings,
            redactionReport: error.redactionReport,
          },
          { status: 422 },
        ),
      PylonCodexStorageError: error =>
        noStoreJsonResponse(
          {
            error: 'pylon_codex_storage_error',
            operation: error.operation,
            reason: error.reason,
          },
          { status: 503 },
        ),
    }),
    M.exhaustive,
  )

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

const requireAgent = <Bindings>(
  dependencies: PylonCodexTurnIngestDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<ProgrammaticAgentSession, PylonCodexUnauthorized> => {
  const token = bearerTokenFromRequest(request)
  if (token === undefined) {
    return Effect.fail(new PylonCodexUnauthorized({}))
  }
  return Effect.flatMap(
    Effect.tryPromise({
      catch: () => new PylonCodexUnauthorized({}),
      try: () =>
        authenticateProgrammaticAgent(
          dependencies.agentStore(env),
          token,
          dependencies.nowIso,
        ),
    }),
    session =>
      session === undefined
        ? Effect.fail(new PylonCodexUnauthorized({}))
        : Effect.succeed(session),
  )
}

const routeNowIso = <Bindings>(
  dependencies: PylonCodexTurnIngestDependencies<Bindings>,
): string => dependencies.nowIso?.() ?? currentIsoTimestamp()

const routeMakeId = <Bindings>(
  dependencies: PylonCodexTurnIngestDependencies<Bindings>,
): string => (dependencies.makeId ?? randomUuid)()

const ownerUserIdForAgent = (session: ProgrammaticAgentSession): string => {
  const linked = session.credential.openauthUserId?.trim()
  return linked === undefined || linked === '' ? session.user.id : linked
}

const storageReason = (error: unknown): string =>
  error instanceof TokenUsageLedgerStorageError ||
  error instanceof TokenUsageLedgerUnsafePayload ||
  error instanceof TokenUsageLedgerValidationError
    ? error._tag
    : error instanceof Error
      ? error.message
      : String(error)

const requireOwnedAssignment = <Bindings>(
  dependencies: PylonCodexTurnIngestDependencies<Bindings>,
  env: Bindings,
  session: ProgrammaticAgentSession,
  input: Readonly<{
    assignmentRef: string
    pylonRef?: string
  }>,
): Effect.Effect<
  PylonApiAssignmentRecord,
  PylonCodexForbidden | PylonCodexNotFound | PylonCodexStorageError
> =>
  Effect.gen(function* () {
    const assignment = yield* Effect.tryPromise({
      catch: error =>
        new PylonCodexStorageError({
          operation: 'pylon_assignment_read',
          reason: pylonApiStoreErrorFromUnknown(error).reason,
        }),
      try: () => dependencies.pylonStore(env).readAssignment(input.assignmentRef),
    })
    if (assignment === undefined) {
      return yield* new PylonCodexNotFound({
        reason: 'Pylon assignment was not found.',
      })
    }
    if (assignment.ownerAgentUserId !== session.user.id) {
      return yield* new PylonCodexForbidden({
        reason: 'Pylon assignment belongs to another agent.',
      })
    }
    if (input.pylonRef !== undefined && assignment.pylonRef !== input.pylonRef) {
      return yield* new PylonCodexForbidden({
        reason: 'Pylon assignment is not assigned to this pylon.',
      })
    }
    return assignment
  })

const boundedText = (value: string | undefined, fallback: string): string => {
  const trimmed = value?.trim()
  if (trimmed === undefined || trimmed === '') {
    return fallback
  }
  return trimmed.length > 16_000
    ? `${trimmed.slice(0, 16_000)}\n[TRUNCATED]`
    : trimmed
}

export const codexTurnUsageTokenCounts = (
  usage: PylonCodexUsage,
): PylonCodexTokenCounts => {
  const inputTokens = Math.max(0, Math.trunc(usage.inputTokens))
  const reasoningTokens = Math.max(
    0,
    Math.trunc(usage.reasoningOutputTokens ?? 0),
  )
  const outputTokens =
    Math.max(0, Math.trunc(usage.outputTokens)) + reasoningTokens
  return {
    cacheReadTokens: Math.max(0, Math.trunc(usage.cachedInputTokens ?? 0)),
    inputTokens,
    outputTokens,
    reasoningTokens,
    totalTokens: inputTokens + outputTokens,
  }
}

const itemToAtifStep = (
  item: PylonCodexTurnItem,
  stepId: number,
): AtifStep => {
  if (item.itemType === 'agent_message') {
    return new AtifStep({
      step_id: stepId,
      source: 'agent',
      message: boundedText(item.message, 'Codex produced an agent message.'),
      model_name: PYLON_CODEX_MODEL_NAME,
    })
  }

  if (item.itemType === 'reasoning') {
    return new AtifStep({
      step_id: stepId,
      source: 'agent',
      message: 'Codex produced a reasoning summary.',
      reasoning_content: boundedText(
        item.reasoningSummary ?? item.message,
        'Codex reasoning summary was present.',
      ),
      model_name: PYLON_CODEX_MODEL_NAME,
    })
  }

  if (item.itemType === 'command_execution') {
    const callId = `codex-command-${item.ordinal}`
    return new AtifStep({
      step_id: stepId,
      source: 'agent',
      message: `Codex completed a command execution${typeof item.exitCode === 'number' ? ` with exit code ${item.exitCode}` : ''}.`,
      model_name: PYLON_CODEX_MODEL_NAME,
      tool_calls: [
        {
          tool_call_id: callId,
          function_name: 'command_execution',
          arguments: {
            commandLabel: item.commandLabel ?? 'shell_command',
          },
        },
      ],
      observation: {
        results: [
          {
            source_call_id: callId,
            content: `exitCode=${typeof item.exitCode === 'number' ? item.exitCode : 'unknown'} outputBytes=${item.outputBytes ?? 0}`,
          },
        ],
      },
    })
  }

  if (item.itemType === 'file_change') {
    return new AtifStep({
      step_id: stepId,
      source: 'agent',
      message: `Codex reported ${item.changeCount ?? 0} file change(s).`,
      model_name: PYLON_CODEX_MODEL_NAME,
    })
  }

  if (item.itemType === 'mcp_tool_call' || item.itemType === 'web_search') {
    const callId = `codex-tool-${item.ordinal}`
    return new AtifStep({
      step_id: stepId,
      source: 'agent',
      message:
        item.itemType === 'web_search'
          ? 'Codex performed a web search.'
          : 'Codex performed an MCP tool call.',
      model_name: PYLON_CODEX_MODEL_NAME,
      tool_calls: [
        {
          tool_call_id: callId,
          function_name:
            item.itemType === 'web_search'
              ? 'web_search'
              : 'mcp_tool_call',
          arguments: {
            toolName: item.toolName ?? item.itemType,
          },
        },
      ],
      observation: {
        results: [
          {
            source_call_id: callId,
            content: item.status ?? 'completed',
          },
        ],
      },
    })
  }

  return new AtifStep({
    step_id: stepId,
    source: 'agent',
    message:
      item.itemType === 'error'
        ? 'Codex reported an execution error.'
        : 'Codex emitted a structured event.',
    model_name: PYLON_CODEX_MODEL_NAME,
  })
}

export const pylonCodexTurnToAtifTrajectory = (
  body: PylonCodexTurnIngestBody,
): AtifTrajectory => {
  const counts = codexTurnUsageTokenCounts(body.usage)
  const items =
    body.items.length === 0
      ? [
          new AtifStep({
            step_id: 1,
            source: 'agent',
            message: 'Codex completed a turn.',
            model_name: PYLON_CODEX_MODEL_NAME,
          }),
        ]
      : body.items.map((item, index) => itemToAtifStep(item, index + 1))

  const finalIndex = items.length - 1
  const final = items[finalIndex]
  const steps =
    final === undefined
      ? items
      : items.map((step, index) =>
          index === finalIndex
            ? new AtifStep({
                ...step,
                metrics: {
                  prompt_tokens: counts.inputTokens,
                  completion_tokens: counts.outputTokens,
                },
              })
            : step,
        )

  return new AtifTrajectory({
    schema_version: ATIF_PINNED_SCHEMA_VERSION,
    trajectory_id: `pylon_codex:${body.assignmentRef}:turn:${body.turnIndex}`,
    session_id: body.sessionRef ?? body.assignmentRef,
    visibility: 'owner_only',
    agent: {
      name: 'Pylon Codex',
      version: 'pylon-codex-v1',
      model_name: PYLON_CODEX_MODEL_NAME,
    },
    steps,
    final_metrics: {
      total_prompt_tokens: counts.inputTokens,
      total_completion_tokens: counts.outputTokens,
      total_steps: steps.length,
    },
  })
}

export const pylonCodexEventChunkToAtifTrajectory = (
  body: PylonCodexEventChunkIngestBody,
): AtifTrajectory => {
  const items =
    body.items === undefined || body.items.length === 0
      ? [
          new AtifStep({
            step_id: 1,
            source: 'agent',
            message: `Codex streamed ${body.rawEvents.length} raw event(s).`,
            model_name: PYLON_CODEX_MODEL_NAME,
          }),
        ]
      : body.items.map((item, index) => itemToAtifStep(item, index + 1))

  return new AtifTrajectory({
    schema_version: ATIF_PINNED_SCHEMA_VERSION,
    trajectory_id: `pylon_codex:${body.assignmentRef}:turn:${body.turnIndex}:chunk:${body.chunkIndex}`,
    session_id: body.sessionRef ?? body.assignmentRef,
    visibility: 'owner_only',
    agent: {
      name: 'Pylon Codex',
      version: 'pylon-codex-v1',
      model_name: PYLON_CODEX_MODEL_NAME,
    },
    steps: items,
    final_metrics: {
      total_prompt_tokens: 0,
      total_completion_tokens: 0,
      total_steps: items.length,
    },
  })
}

const stableTurnDigest = (
  body: PylonCodexTurnIngestBody,
): Effect.Effect<string> =>
  Effect.promise(() =>
    sha256Hex(
      [
        body.assignmentRef,
        body.leaseRef,
        body.pylonRef,
        body.sessionRef ?? 'session.pending',
        String(body.turnIndex),
      ].join(':'),
    ),
  )

const stableDirectLocalUsageDigest = (
  body: PylonCodexDirectLocalUsageIngestBody,
): Effect.Effect<string> =>
  Effect.promise(() =>
    sha256Hex(
      [
        body.accountRefHash,
        body.sessionRef ?? 'session.pending',
        body.observedAt ?? 'observed.pending',
        body.idempotencyKey,
        String(body.usage.inputTokens),
        String(body.usage.outputTokens),
        String(body.usage.totalTokens),
        body.usage.usageTruth,
      ].join(':'),
    ),
  )

const validateDirectLocalAccountRefHash = (
  value: string,
): Effect.Effect<string, PylonCodexValidationError> =>
  /^account\.pylon\.codex\.[a-f0-9]{6,64}$/.test(value)
    ? Effect.succeed(value)
    : Effect.fail(
        new PylonCodexValidationError({
          reason: 'accountRefHash must be a public-safe account.pylon.codex.<hex> ref.',
        }),
      )

const directLocalUsageTokenCounts = (
  usage: PylonCodexDirectLocalUsage,
): PylonCodexTokenCounts => {
  const inputTokens = Math.max(0, Math.trunc(usage.inputTokens))
  const outputTokens = Math.max(0, Math.trunc(usage.outputTokens))
  const totalTokens = Math.max(
    inputTokens + outputTokens,
    Math.trunc(usage.totalTokens),
  )

  return {
    cacheReadTokens: 0,
    inputTokens,
    outputTokens,
    reasoningTokens: 0,
    totalTokens,
  }
}

const directLocalTokenUsageEventBody = (
  input: Readonly<{
    body: PylonCodexDirectLocalUsageIngestBody
    digest: string
    observedAt: string
    ownerUserId: string
    session: ProgrammaticAgentSession
  }>,
) => {
  const counts = directLocalUsageTokenCounts(input.body.usage)
  return {
    schemaVersion: 'openagents.token_usage_event.v1' as const,
    actor: {
      accountRef: `agent:${input.session.user.id}`,
      userId: input.ownerUserId,
    },
    backendProfile: PYLON_CODEX_DIRECT_LOCAL_PROVIDER,
    demand: {
      demandChannel: PYLON_CODEX_DIRECT_LOCAL_DEMAND_CHANNEL,
      demandClient: 'pylon',
      demandKind: PYLON_CODEX_DEMAND_KIND,
      demandSource: PYLON_CODEX_DIRECT_LOCAL_DEMAND_SOURCE,
    },
    eventId: `event.inference.served-tokens.codex-direct-local.${input.digest.slice(0, 32)}`,
    idempotencyKey: input.body.idempotencyKey,
    model: PYLON_CODEX_DIRECT_LOCAL_MODEL_NAME,
    observedAt: input.observedAt,
    privacy: { leaderboardEligible: false, privacyOptOut: false },
    producerSystem: PYLON_CODEX_DIRECT_LOCAL_PRODUCER_SYSTEM,
    provider: PYLON_CODEX_DIRECT_LOCAL_PROVIDER,
    roleRef: input.body.roleRef ?? 'coder',
    safeMetadata: {
      accountRefHash: input.body.accountRefHash,
      demandChannel: PYLON_CODEX_DIRECT_LOCAL_DEMAND_CHANNEL,
      role_ref: input.body.roleRef ?? 'coder',
      roleRef: input.body.roleRef ?? 'coder',
      ...(input.body.pylonRef === undefined
        ? {}
        : { pylonRef: input.body.pylonRef }),
      telemetryOptIn: 'pylon_accounts_usage_explicit',
      usageBasis:
        input.body.usage.usageTruth === 'exact'
          ? 'codex_sdk_local_session_delta'
          : 'codex_rate_window_estimate',
    },
    sourceRefs: {
      anonymizedSourceRef: input.body.accountRefHash,
      ...(input.body.sessionRef === undefined
        ? {}
        : { sessionRef: input.body.sessionRef }),
    },
    sourceRoute: PYLON_CODEX_DIRECT_LOCAL_SOURCE_ROUTE,
    tokenCounts: {
      cacheReadTokens: counts.cacheReadTokens,
      cacheWrite1hTokens: 0,
      cacheWrite5mTokens: 0,
      inputTokens: counts.inputTokens,
      outputTokens: counts.outputTokens,
      reasoningTokens: counts.reasoningTokens,
      totalTokens: counts.totalTokens,
    },
    usageTruth: input.body.usage.usageTruth,
  }
}

const stableEventChunkDigest = (
  body: PylonCodexEventChunkIngestBody,
): Effect.Effect<string> =>
  Effect.promise(() =>
    sha256Hex(
      [
        body.assignmentRef,
        body.leaseRef,
        body.pylonRef,
        body.sessionRef ?? 'session.pending',
        String(body.turnIndex),
        String(body.chunkIndex),
      ].join(':'),
    ),
  )

const tokenUsageEventBody = (
  input: Readonly<{
    body: PylonCodexTurnIngestBody
    digest: string
    observedAt: string
    ownerUserId: string
    session: ProgrammaticAgentSession
  }>,
) => {
  const counts = codexTurnUsageTokenCounts(input.body.usage)
  return {
    schemaVersion: 'openagents.token_usage_event.v1' as const,
    actor: {
      accountRef: `agent:${input.session.user.id}`,
      userId: input.ownerUserId,
    },
    backendProfile: PYLON_CODEX_PROVIDER,
    demand: {
      demandKind: PYLON_CODEX_DEMAND_KIND,
      demandSource: PYLON_CODEX_DEMAND_SOURCE,
    },
    eventId: `event.inference.served-tokens.pylon-codex.${input.digest.slice(0, 32)}`,
    idempotencyKey: `khala:pylon-codex:turn:${input.digest}`,
    model: PYLON_CODEX_MODEL_NAME,
    observedAt: input.observedAt,
    privacy: { leaderboardEligible: false, privacyOptOut: false },
    producerSystem: PYLON_CODEX_PRODUCER_SYSTEM,
    provider: PYLON_CODEX_PROVIDER,
    roleRef: input.body.roleRef ?? 'coder',
    safeMetadata: {
      assignmentRef: input.body.assignmentRef,
      leaseRef: input.body.leaseRef,
      pylonRef: input.body.pylonRef,
      role_ref: input.body.roleRef ?? 'coder',
      roleRef: input.body.roleRef ?? 'coder',
      codexUsageSplit: {
        cachedInputTokens: input.body.usage.cachedInputTokens ?? 0,
        inputTokens: input.body.usage.inputTokens,
        outputTokens: input.body.usage.outputTokens,
        reasoningOutputTokens: input.body.usage.reasoningOutputTokens ?? 0,
      },
      costCaveat: 'owner_capacity_provider_cost_unknown',
      usageBasis: 'codex_sdk_turn_completed',
    },
    sourceRefs: {
      ...(input.body.runRef === undefined ? {} : { runRef: input.body.runRef }),
      ...(input.body.sessionRef === undefined
        ? {}
        : { sessionRef: input.body.sessionRef }),
      taskRef: input.body.assignmentRef,
      ...(input.body.workspaceRef === undefined
        ? {}
        : { repositoryRef: input.body.workspaceRef }),
    },
    sourceRoute: PYLON_CODEX_SOURCE_ROUTE,
    tokenCounts: {
      cacheReadTokens: counts.cacheReadTokens,
      cacheWrite1hTokens: 0,
      cacheWrite5mTokens: 0,
      inputTokens: counts.inputTokens,
      outputTokens: counts.outputTokens,
      reasoningTokens: counts.reasoningTokens,
      totalTokens: counts.totalTokens,
    },
    usageTruth: 'exact' as const,
  }
}

const storeTrace = <Bindings>(
  dependencies: PylonCodexTurnIngestDependencies<Bindings>,
  env: Bindings,
  input: Readonly<{
    body: PylonCodexTurnIngestBody
    digest: string
    nowIso: string
    ownerUserId: string
    session: ProgrammaticAgentSession
  }>,
): Effect.Effect<
  Readonly<{
    created: boolean
    redactionReport: TraceRedactionReport
    uuid: string
  }>,
  PylonCodexStorageError | PylonCodexTraceRejected | PylonCodexValidationError
> =>
  Effect.gen(function* () {
    const mapped = pylonCodexTurnToAtifTrajectory(input.body)
    const mappedIssues = validateAtifTrajectory(mapped)
    if (mappedIssues.length > 0) {
      return yield* new PylonCodexValidationError({
        reason: mappedIssues.map(issue => issue.message).join(' '),
      })
    }

    const { value: redacted, report: redactionReport } =
      redactTraceValue(mapped)
    const trajectory = redacted as AtifTrajectory
    const redactedIssues = validateAtifTrajectory(trajectory)
    if (redactedIssues.length > 0) {
      return yield* new PylonCodexValidationError({
        reason: redactedIssues.map(issue => issue.message).join(' '),
      })
    }

    const tripwireFindings = atifTraceTripwire(trajectory)
    if (tripwireFindings.length > 0) {
      return yield* new PylonCodexTraceRejected({
        findings: tripwireFindings.map(finding => finding.code),
        redactionReport,
      })
    }

    const stored = yield* Effect.tryPromise({
      catch: error =>
        new PylonCodexStorageError({
          operation: 'trace_store_create',
          reason: traceStoreErrorFromUnknown(error).reason,
        }),
      try: () =>
        dependencies.traceStore(env).createTrace({
          traceUuid: routeMakeId(dependencies),
          ownerUserId: input.ownerUserId,
          agentRef: `agent:${input.session.user.id}`,
          schemaVersion: trajectory.schema_version,
          trajectoryId: trajectory.trajectory_id,
          sessionId: trajectory.session_id ?? null,
          visibility: 'owner_only',
          stepCount: trajectory.steps.length,
          trajectory,
          trajectoryR2Key: null,
          blobRefs: [],
          idempotencyKey: `pylon-codex:${input.digest}`,
          trainingConsent: false,
          license: null,
          contentDigest: null,
          rewardEligible: false,
          rewardAmountSats: null,
          uploadSource: 'agent',
          demandKind: PYLON_CODEX_DEMAND_KIND,
          demandSource: PYLON_CODEX_DEMAND_SOURCE,
          nowIso: input.nowIso,
        }),
    })

    return {
      created: stored.created,
      redactionReport,
      uuid: stored.record.traceUuid,
    }
  })

const storeEventChunkTrace = <Bindings>(
  dependencies: PylonCodexTurnIngestDependencies<Bindings>,
  env: Bindings,
  input: Readonly<{
    body: PylonCodexEventChunkIngestBody
    digest: string
    nowIso: string
    ownerUserId: string
    session: ProgrammaticAgentSession
  }>,
): Effect.Effect<
  Readonly<{
    created: boolean
    redactionReport: TraceRedactionReport
    uuid: string
  }>,
  PylonCodexStorageError | PylonCodexTraceRejected | PylonCodexValidationError
> =>
  Effect.gen(function* () {
    const mapped = pylonCodexEventChunkToAtifTrajectory(input.body)
    const mappedIssues = validateAtifTrajectory(mapped)
    if (mappedIssues.length > 0) {
      return yield* new PylonCodexValidationError({
        reason: mappedIssues.map(issue => issue.message).join(' '),
      })
    }

    const { value: redacted, report: redactionReport } =
      redactTraceValue(mapped)
    const trajectory = redacted as AtifTrajectory
    const redactedIssues = validateAtifTrajectory(trajectory)
    if (redactedIssues.length > 0) {
      return yield* new PylonCodexValidationError({
        reason: redactedIssues.map(issue => issue.message).join(' '),
      })
    }

    const tripwireFindings = atifTraceTripwire(trajectory)
    if (tripwireFindings.length > 0) {
      return yield* new PylonCodexTraceRejected({
        findings: tripwireFindings.map(finding => finding.code),
        redactionReport,
      })
    }

    const stored = yield* Effect.tryPromise({
      catch: error =>
        new PylonCodexStorageError({
          operation: 'trace_store_create',
          reason: traceStoreErrorFromUnknown(error).reason,
        }),
      try: () =>
        dependencies.traceStore(env).createTrace({
          traceUuid: routeMakeId(dependencies),
          ownerUserId: input.ownerUserId,
          agentRef: `agent:${input.session.user.id}`,
          schemaVersion: trajectory.schema_version,
          trajectoryId: trajectory.trajectory_id,
          sessionId: trajectory.session_id ?? null,
          visibility: 'owner_only',
          stepCount: trajectory.steps.length,
          trajectory,
          trajectoryR2Key: null,
          blobRefs: [],
          idempotencyKey: `pylon-codex-event-chunk:${input.digest}`,
          trainingConsent: false,
          license: null,
          contentDigest: null,
          rewardEligible: false,
          rewardAmountSats: null,
          uploadSource: 'agent',
          demandKind: PYLON_CODEX_DEMAND_KIND,
          demandSource: PYLON_CODEX_DEMAND_SOURCE,
          nowIso: input.nowIso,
        }),
    })

    return {
      created: stored.created,
      redactionReport,
      uuid: stored.record.traceUuid,
    }
  })

type PylonCodexTraceDropDiagnostic = Readonly<{
  findings?: ReadonlyArray<string>
  operation?: string
  reason: string
  redactionReport?: TraceRedactionReport
}>

type PylonCodexTraceOutcome =
  | Readonly<{
      kind: 'stored'
      trace: Readonly<{
        created: boolean
        redactionReport: TraceRedactionReport
        uuid: string
      }>
    }>
  | Readonly<{
      kind: 'dropped'
      diagnostic: PylonCodexTraceDropDiagnostic
    }>

type PylonCodexRawEventsDropDiagnostic = Readonly<{
  operation: 'raw_codex_events_store'
  reason: string
}>

type PylonCodexRawEventsOutcome =
  | Readonly<{
      kind: 'stored'
      ref: string
      created: boolean
      byteLength: number
      eventCount: number
      r2Key: string
    }>
  | Readonly<{
      kind: 'dropped'
      diagnostic: PylonCodexRawEventsDropDiagnostic
      eventCount: number
    }>
  | Readonly<{
      kind: 'not_submitted'
      eventCount: 0
    }>

type PylonCodexRawEventChunkOutcome =
  | Readonly<{
      kind: 'stored'
      ref: string
      created: boolean
      byteLength: number
      eventCount: number
      r2Key: string
    }>
  | Readonly<{
      kind: 'dropped'
      diagnostic: PylonCodexRawEventsDropDiagnostic
      eventCount: number
    }>

const traceDropDiagnostic = (
  error:
    | PylonCodexStorageError
    | PylonCodexTraceRejected
    | PylonCodexValidationError,
): PylonCodexTraceDropDiagnostic =>
  M.value(error).pipe(
    M.tags({
      PylonCodexStorageError: error => ({
        operation: error.operation,
        reason: 'trace_store_unavailable',
      }),
      PylonCodexTraceRejected: error => ({
        findings: error.findings,
        reason: 'trace_rejected_after_redaction',
      }),
      PylonCodexValidationError: () => ({
        reason: 'trace_projection_invalid',
      }),
    }),
    M.exhaustive,
  )

const storeRawCodexEvents = <Bindings>(
  dependencies: PylonCodexTurnIngestDependencies<Bindings>,
  env: Bindings,
  input: Readonly<{
    body: PylonCodexTurnIngestBody
    digest: string
    observedAt: string
    ownerUserId: string
  }>,
): Effect.Effect<PylonCodexRawEventsOutcome> => {
  const events = input.body.rawEvents
  if (events === undefined || events.length === 0) {
    return Effect.succeed({ eventCount: 0, kind: 'not_submitted' as const })
  }

  const store = dependencies.rawEventStore?.(env)
  if (store === undefined) {
    return Effect.succeed({
      diagnostic: {
        operation: 'raw_codex_events_store',
        reason: 'raw_event_store_unconfigured',
      },
      eventCount: events.length,
      kind: 'dropped' as const,
    })
  }

  const eventsJson = Effect.try({
    catch: error => ({
      diagnostic: {
        operation: 'raw_codex_events_store' as const,
        reason:
          error instanceof Error
            ? error.message
            : 'raw_event_payload_serialize_failed',
      },
      eventCount: events.length,
      kind: 'dropped' as const,
    }),
    try: () =>
      JSON.stringify({
        schemaVersion: 'openagents.pylon.codex_raw_events.v1',
        assignmentRef: input.body.assignmentRef,
        leaseRef: input.body.leaseRef,
        pylonRef: input.body.pylonRef,
        runRef: input.body.runRef ?? null,
        sessionRef: input.body.sessionRef ?? null,
        workspaceRef: input.body.workspaceRef ?? null,
        turnIndex: input.body.turnIndex,
        observedAt: input.observedAt,
        eventCount: events.length,
        events,
        usage: input.body.usage,
      }),
  })

  return eventsJson.pipe(
    Effect.flatMap(serialized =>
      Effect.tryPromise({
        catch: error => ({
          diagnostic: {
            operation: 'raw_codex_events_store' as const,
            reason:
              error instanceof Error
                ? error.message
                : 'raw_event_store_unavailable',
          },
          eventCount: events.length,
          kind: 'dropped' as const,
        }),
        try: () =>
          store.putTurnEvents({
            assignmentRef: input.body.assignmentRef,
            digest: input.digest,
            eventCount: events.length,
            eventsJson: serialized,
            leaseRef: input.body.leaseRef,
            observedAt: input.observedAt,
            ownerUserId: input.ownerUserId,
            pylonRef: input.body.pylonRef,
            runRef: input.body.runRef ?? null,
            sessionRef: input.body.sessionRef ?? null,
            turnIndex: input.body.turnIndex,
            workspaceRef: input.body.workspaceRef ?? null,
          }),
      }),
    ),
    Effect.match({
      onFailure: outcome => outcome,
      onSuccess: result => ({
        byteLength: result.byteLength,
        created: result.created,
        eventCount: events.length,
        kind: 'stored' as const,
        ref: result.ref,
        r2Key: result.r2Key,
      }),
    }),
  )
}

const storeRawCodexEventChunk = <Bindings>(
  dependencies: PylonCodexTurnIngestDependencies<Bindings>,
  env: Bindings,
  input: Readonly<{
    body: PylonCodexEventChunkIngestBody
    digest: string
    observedAt: string
    ownerUserId: string
  }>,
): Effect.Effect<PylonCodexRawEventChunkOutcome> => {
  const store = dependencies.rawEventChunkStore?.(env)
  if (store === undefined) {
    return Effect.succeed({
      diagnostic: {
        operation: 'raw_codex_events_store',
        reason: 'raw_event_chunk_store_unconfigured',
      },
      eventCount: input.body.rawEvents.length,
      kind: 'dropped' as const,
    })
  }

  const eventsJson = Effect.try({
    catch: error => ({
      diagnostic: {
        operation: 'raw_codex_events_store' as const,
        reason:
          error instanceof Error
            ? error.message
            : 'raw_event_chunk_payload_serialize_failed',
      },
      eventCount: input.body.rawEvents.length,
      kind: 'dropped' as const,
    }),
    try: () =>
      JSON.stringify({
        schemaVersion: 'openagents.pylon.codex_raw_event_chunk.v1',
        assignmentRef: input.body.assignmentRef,
        leaseRef: input.body.leaseRef,
        pylonRef: input.body.pylonRef,
        runRef: input.body.runRef ?? null,
        sessionRef: input.body.sessionRef ?? null,
        workspaceRef: input.body.workspaceRef ?? null,
        turnIndex: input.body.turnIndex,
        chunkIndex: input.body.chunkIndex,
        observedAt: input.observedAt,
        eventCount: input.body.rawEvents.length,
        events: input.body.rawEvents,
      }),
  })

  return eventsJson.pipe(
    Effect.flatMap(serialized =>
      Effect.tryPromise({
        catch: error => ({
          diagnostic: {
            operation: 'raw_codex_events_store' as const,
            reason:
              error instanceof Error
                ? error.message
                : 'raw_event_chunk_store_unavailable',
          },
          eventCount: input.body.rawEvents.length,
          kind: 'dropped' as const,
        }),
        try: () =>
          store.putEventChunk({
            assignmentRef: input.body.assignmentRef,
            chunkIndex: input.body.chunkIndex,
            digest: input.digest,
            eventCount: input.body.rawEvents.length,
            eventsJson: serialized,
            leaseRef: input.body.leaseRef,
            observedAt: input.observedAt,
            ownerUserId: input.ownerUserId,
            pylonRef: input.body.pylonRef,
            runRef: input.body.runRef ?? null,
            sessionRef: input.body.sessionRef ?? null,
            turnIndex: input.body.turnIndex,
            workspaceRef: input.body.workspaceRef ?? null,
          }),
      }),
    ),
    Effect.match({
      onFailure: outcome => outcome,
      onSuccess: result => ({
        byteLength: result.byteLength,
        created: result.created,
        eventCount: input.body.rawEvents.length,
        kind: 'stored' as const,
        ref: result.ref,
        r2Key: result.r2Key,
      }),
    }),
  )
}

const assignmentRefFromProofRequest = (
  request: Request,
): Effect.Effect<string, PylonCodexValidationError> => {
  const value = new URL(request.url).searchParams.get('assignmentRef')?.trim()
  if (value === undefined || value === '') {
    return Effect.fail(
      new PylonCodexValidationError({
        reason: 'Missing required assignmentRef query parameter.',
      }),
    )
  }
  if (!/^[A-Za-z0-9][A-Za-z0-9_.:-]{2,180}$/.test(value)) {
    return Effect.fail(
      new PylonCodexValidationError({
        reason: 'assignmentRef is not a bounded public-safe ref.',
      }),
    )
  }
  return Effect.succeed(value)
}

const readCloseoutPolicy = <Bindings>(
  dependencies: PylonCodexTurnIngestDependencies<Bindings>,
  env: Bindings,
  assignment: PylonApiAssignmentRecord,
  assignmentRef: string,
  operation: string,
): Effect.Effect<PylonCodexAssignmentCloseoutPolicy, PylonCodexStorageError> =>
  Effect.tryPromise({
    catch: error =>
      new PylonCodexStorageError({
        operation,
        reason: error instanceof Error ? error.message : String(error),
      }),
    try: async () => {
      const events = await dependencies
        .pylonStore(env)
        .listEventsForAssignment(assignmentRef, 25)
      return closeoutPolicyFromEvents(events, assignment)
    },
  })

const routeProof = <Bindings>(
  dependencies: PylonCodexTurnIngestDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<HttpResponse, PylonCodexRouteError> =>
  Effect.gen(function* () {
    if (request.method !== 'GET') {
      return methodNotAllowed(['GET'])
    }
    const proofStore = dependencies.proofStore?.(env)
    if (proofStore === undefined) {
      return yield* new PylonCodexStorageError({
        operation: 'pylon_codex_assignment_proof_read',
        reason: 'proof_store_unconfigured',
      })
    }

    const session = yield* requireAgent(dependencies, request, env)
    const assignmentRef = yield* assignmentRefFromProofRequest(request)
    const assignment = yield* requireOwnedAssignment(dependencies, env, session, {
      assignmentRef,
    })
    const ownerUserId = ownerUserIdForAgent(session)
    const closeoutPolicy = yield* readCloseoutPolicy(
      dependencies,
      env,
      assignment,
      assignmentRef,
      'pylon_codex_assignment_proof_read',
    )
    const proof = yield* Effect.tryPromise({
      catch: error =>
        new PylonCodexStorageError({
          operation: 'pylon_codex_assignment_proof_read',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () =>
        proofStore.readAssignmentProof({
          assignmentRef,
          closeoutPolicy,
          nowIso: routeNowIso(dependencies),
          ownerAgentUserId: session.user.id,
          ownerUserId,
          pylonRef: assignment.pylonRef,
        }),
    })

    return noStoreJsonResponse(proof)
  })

const routeTraceStatus = <Bindings>(
  dependencies: PylonCodexTurnIngestDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<HttpResponse, PylonCodexRouteError> =>
  Effect.gen(function* () {
    if (request.method !== 'GET') {
      return methodNotAllowed(['GET'])
    }
    const statusStore = dependencies.traceStatusStore?.(env)
    if (statusStore === undefined) {
      return yield* new PylonCodexStorageError({
        operation: 'pylon_codex_assignment_trace_status_read',
        reason: 'trace_status_store_unconfigured',
      })
    }

    const session = yield* requireAgent(dependencies, request, env)
    const assignmentRef = yield* assignmentRefFromProofRequest(request)
    const assignment = yield* requireOwnedAssignment(dependencies, env, session, {
      assignmentRef,
    })
    const ownerUserId = ownerUserIdForAgent(session)
    const closeoutPolicy = yield* readCloseoutPolicy(
      dependencies,
      env,
      assignment,
      assignmentRef,
      'pylon_codex_assignment_trace_status_read',
    )
    const status = yield* Effect.tryPromise({
      catch: error =>
        new PylonCodexStorageError({
          operation: 'pylon_codex_assignment_trace_status_read',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () =>
        statusStore.readAssignmentTraceStatus({
          assignment,
          closeoutPolicy,
          nowIso: routeNowIso(dependencies),
          ownerAgentUserId: session.user.id,
          ownerUserId,
        }),
    })

    return noStoreJsonResponse(status)
  })

const routeEventChunkIngest = <Bindings>(
  dependencies: PylonCodexTurnIngestDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<HttpResponse, PylonCodexRouteError> =>
  Effect.gen(function* () {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const session = yield* requireAgent(dependencies, request, env)

    const rawBody = yield* Effect.tryPromise({
      catch: () =>
        new PylonCodexValidationError({
          reason: 'Request body could not be read.',
        }),
      try: () => request.text(),
    })
    if (new TextEncoder().encode(rawBody).length > MAX_BODY_BYTES) {
      return yield* new PylonCodexValidationError({
        reason: `Pylon Codex event chunk payload exceeds the ${MAX_BODY_BYTES}-byte limit.`,
      })
    }

    const body = yield* Effect.try({
      catch: error =>
        new PylonCodexValidationError({
          reason:
            error instanceof Error
              ? error.message
              : 'Request body does not match the Pylon Codex event chunk schema.',
        }),
      try: () =>
        decodeUnknownWithSchema(
          PylonCodexEventChunkIngestBody,
          rawBody.trim() === '' ? {} : parseJsonUnknown(rawBody),
        ),
    })
    if (body.rawEvents.length === 0) {
      return yield* new PylonCodexValidationError({
        reason: 'Pylon Codex event chunk must include at least one raw event.',
      })
    }

    yield* requireOwnedAssignment(dependencies, env, session, {
      assignmentRef: body.assignmentRef,
      pylonRef: body.pylonRef,
    })

    const ownerUserId = ownerUserIdForAgent(session)
    const observedAt = body.observedAt ?? routeNowIso(dependencies)
    const digest = yield* stableEventChunkDigest(body)
    const rawEventsOutcome = yield* storeRawCodexEventChunk(dependencies, env, {
      body,
      digest,
      observedAt,
      ownerUserId,
    })

    const traceOutcome: PylonCodexTraceOutcome = yield* storeEventChunkTrace(
      dependencies,
      env,
      {
        body,
        digest,
        nowIso: observedAt,
        ownerUserId,
        session,
      },
    ).pipe(
      Effect.match({
        onFailure: error => ({
          diagnostic: traceDropDiagnostic(error),
          kind: 'dropped' as const,
        }),
        onSuccess: trace => ({
          kind: 'stored' as const,
          trace,
        }),
      }),
    )

    const trace =
      traceOutcome.kind === 'stored'
        ? {
            created: traceOutcome.trace.created,
            uuid: traceOutcome.trace.uuid,
            visibility: 'owner_only' as const,
          }
        : {
            diagnostic: traceOutcome.diagnostic,
            dropped: true,
            visibility: 'owner_only' as const,
          }

    const redactionReport =
      traceOutcome.kind === 'stored'
        ? traceOutcome.trace.redactionReport
        : traceOutcome.diagnostic.redactionReport

    return noStoreJsonResponse({
      schemaVersion: 'openagents.pylon.codex_event_chunk_ingest_result.v1',
      assignmentRef: body.assignmentRef,
      chunkIndex: body.chunkIndex,
      rawEvents:
        rawEventsOutcome.kind === 'stored'
          ? {
              byteLength: rawEventsOutcome.byteLength,
              created: rawEventsOutcome.created,
              eventCount: rawEventsOutcome.eventCount,
              ref: rawEventsOutcome.ref,
              r2Key: rawEventsOutcome.r2Key,
              visibility: 'owner_only' as const,
            }
          : {
              diagnostic: rawEventsOutcome.diagnostic,
              dropped: true,
              eventCount: rawEventsOutcome.eventCount,
              visibility: 'owner_only' as const,
            },
      trace,
      turnIndex: body.turnIndex,
      ...(redactionReport === undefined ? {} : { redactionReport }),
    })
  })

const routeIngest = <Bindings>(
  dependencies: PylonCodexTurnIngestDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<HttpResponse, PylonCodexRouteError> =>
  Effect.gen(function* () {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const session = yield* requireAgent(dependencies, request, env)

    const rawBody = yield* Effect.tryPromise({
      catch: () =>
        new PylonCodexValidationError({
          reason: 'Request body could not be read.',
        }),
      try: () => request.text(),
    })
    if (new TextEncoder().encode(rawBody).length > MAX_BODY_BYTES) {
      return yield* new PylonCodexValidationError({
        reason: `Pylon Codex turn payload exceeds the ${MAX_BODY_BYTES}-byte limit.`,
      })
    }

    const body = yield* Effect.try({
      catch: error =>
        new PylonCodexValidationError({
          reason:
            error instanceof Error
              ? error.message
              : 'Request body does not match the Pylon Codex turn schema.',
        }),
      try: () =>
        decodeUnknownWithSchema(
          PylonCodexTurnIngestBody,
          rawBody.trim() === '' ? {} : parseJsonUnknown(rawBody),
        ),
    })

    yield* requireOwnedAssignment(dependencies, env, session, {
      assignmentRef: body.assignmentRef,
      pylonRef: body.pylonRef,
    })

    const ownerUserId = ownerUserIdForAgent(session)
    const observedAt = body.observedAt ?? routeNowIso(dependencies)
    const digest = yield* stableTurnDigest(body)
    const tokenBody = tokenUsageEventBody({
      body,
      digest,
      observedAt,
      ownerUserId,
      session,
    })
    const counts = codexTurnUsageTokenCounts(body.usage)

    const tokenResult = yield* dependencies
      .ledger(env)
      .ingestEvent(tokenBody)
      .pipe(
        Effect.mapError(
          error =>
            new PylonCodexStorageError({
              operation: 'token_usage_ingest',
              reason: storageReason(error),
            }),
        ),
      )

    const rawEventsOutcome = yield* storeRawCodexEvents(dependencies, env, {
      body,
      digest,
      observedAt,
      ownerUserId,
    })

    if (
      tokenResult.inserted &&
      dependencies.publishDelta !== undefined &&
      counts.inputTokens + counts.outputTokens > 0
    ) {
      yield* dependencies
        .publishDelta(env, {
          eventRef: tokenBody.eventId,
          observedAt,
          tokensServedDelta: counts.inputTokens + counts.outputTokens,
        })
        .pipe(Effect.catch(() => Effect.void))
    }

    const traceOutcome: PylonCodexTraceOutcome = yield* storeTrace(
      dependencies,
      env,
      {
        body,
        digest,
        nowIso: observedAt,
        ownerUserId,
        session,
      },
    ).pipe(
      Effect.match({
        onFailure: error => ({
          diagnostic: traceDropDiagnostic(error),
          kind: 'dropped' as const,
        }),
        onSuccess: trace => ({
          kind: 'stored' as const,
          trace,
        }),
      }),
    )

    const trace =
      traceOutcome.kind === 'stored'
        ? {
            created: traceOutcome.trace.created,
            uuid: traceOutcome.trace.uuid,
            visibility: 'owner_only' as const,
          }
        : {
            diagnostic: traceOutcome.diagnostic,
            dropped: true,
            visibility: 'owner_only' as const,
          }

    const redactionReport =
      traceOutcome.kind === 'stored'
        ? traceOutcome.trace.redactionReport
        : traceOutcome.diagnostic.redactionReport

    return noStoreJsonResponse({
      schemaVersion: 'openagents.pylon.codex_turn_ingest_result.v1',
      assignmentRef: body.assignmentRef,
      insertedTokenUsage: tokenResult.inserted,
      tokensServedDelta: tokenResult.inserted
        ? counts.inputTokens + counts.outputTokens
        : 0,
      tokenUsageEventRef: tokenBody.eventId,
      rawEvents:
        rawEventsOutcome.kind === 'stored'
          ? {
              byteLength: rawEventsOutcome.byteLength,
              created: rawEventsOutcome.created,
              eventCount: rawEventsOutcome.eventCount,
              ref: rawEventsOutcome.ref,
              r2Key: rawEventsOutcome.r2Key,
              visibility: 'owner_only' as const,
            }
          : rawEventsOutcome.kind === 'dropped'
            ? {
                diagnostic: rawEventsOutcome.diagnostic,
                dropped: true,
                eventCount: rawEventsOutcome.eventCount,
                visibility: 'owner_only' as const,
              }
            : {
                eventCount: 0,
                submitted: false,
                visibility: 'owner_only' as const,
              },
      trace,
      ...(redactionReport === undefined ? {} : { redactionReport }),
    })
  })

const routeDirectLocalUsageIngest = <Bindings>(
  dependencies: PylonCodexTurnIngestDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<HttpResponse, PylonCodexRouteError> =>
  Effect.gen(function* () {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const session = yield* requireAgent(dependencies, request, env)

    const rawBody = yield* Effect.tryPromise({
      catch: () =>
        new PylonCodexValidationError({
          reason: 'Request body could not be read.',
        }),
      try: () => request.text(),
    })
    if (new TextEncoder().encode(rawBody).length > MAX_BODY_BYTES) {
      return yield* new PylonCodexValidationError({
        reason: `Pylon Codex direct-local usage payload exceeds the ${MAX_BODY_BYTES}-byte limit.`,
      })
    }

    const body = yield* Effect.try({
      catch: error =>
        new PylonCodexValidationError({
          reason:
            error instanceof Error
              ? error.message
              : 'Request body does not match the Pylon Codex direct-local usage schema.',
        }),
      try: () =>
        decodeUnknownWithSchema(
          PylonCodexDirectLocalUsageIngestBody,
          rawBody.trim() === '' ? {} : parseJsonUnknown(rawBody),
        ),
    })

    yield* validateDirectLocalAccountRefHash(body.accountRefHash)

    const ownerUserId = ownerUserIdForAgent(session)
    const observedAt = body.observedAt ?? routeNowIso(dependencies)
    const counts = directLocalUsageTokenCounts(body.usage)
    const tokensServed = counts.inputTokens + counts.outputTokens
    if (tokensServed <= 0) {
      return yield* new PylonCodexValidationError({
        reason: 'Pylon Codex direct-local usage must include at least one input or output token.',
      })
    }

    const digest = yield* stableDirectLocalUsageDigest(body)
    const tokenBody = directLocalTokenUsageEventBody({
      body,
      digest,
      observedAt,
      ownerUserId,
      session,
    })

    const tokenResult = yield* dependencies
      .ledger(env)
      .ingestEvent(tokenBody)
      .pipe(
        Effect.mapError(
          error =>
            new PylonCodexStorageError({
              operation: 'token_usage_ingest',
              reason: storageReason(error),
            }),
        ),
      )

    if (
      tokenResult.inserted &&
      dependencies.publishDelta !== undefined &&
      tokensServed > 0
    ) {
      yield* dependencies
        .publishDelta(env, {
          eventRef: tokenBody.eventId,
          observedAt,
          tokensServedDelta: tokensServed,
        })
        .pipe(Effect.catch(() => Effect.void))
    }

    return noStoreJsonResponse({
      schemaVersion:
        'openagents.pylon.codex_direct_local_usage_ingest_result.v1',
      accountRefHash: body.accountRefHash,
      demandChannel: PYLON_CODEX_DIRECT_LOCAL_DEMAND_CHANNEL,
      insertedTokenUsage: tokenResult.inserted,
      tokenUsageEventRef: tokenBody.eventId,
      tokensServedDelta: tokenResult.inserted ? tokensServed : 0,
      usageTruth: body.usage.usageTruth,
    })
  })

const claudeTokenUsageEventBody = (
  input: Readonly<{
    body: PylonClaudeTurnIngestBody
    digest: string
    observedAt: string
    ownerUserId: string
    session: ProgrammaticAgentSession
  }>,
) => {
  const counts = codexTurnUsageTokenCounts(input.body.usage)
  return {
    schemaVersion: 'openagents.token_usage_event.v1' as const,
    actor: {
      accountRef: `agent:${input.session.user.id}`,
      userId: input.ownerUserId,
    },
    backendProfile: PYLON_CLAUDE_PROVIDER,
    demand: {
      demandKind: PYLON_CODEX_DEMAND_KIND,
      demandSource: PYLON_CODEX_DEMAND_SOURCE,
    },
    eventId: `event.inference.served-tokens.pylon-claude.${input.digest.slice(0, 32)}`,
    idempotencyKey: `khala:pylon-claude:turn:${input.digest}`,
    model: PYLON_CLAUDE_MODEL_NAME,
    observedAt: input.observedAt,
    privacy: { leaderboardEligible: false, privacyOptOut: false },
    producerSystem: PYLON_CODEX_PRODUCER_SYSTEM,
    provider: PYLON_CLAUDE_PROVIDER,
    roleRef: input.body.roleRef ?? 'coder',
    safeMetadata: {
      assignmentRef: input.body.assignmentRef,
      leaseRef: input.body.leaseRef,
      pylonRef: input.body.pylonRef,
      role_ref: input.body.roleRef ?? 'coder',
      roleRef: input.body.roleRef ?? 'coder',
      claudeUsageSplit: {
        cachedInputTokens: input.body.usage.cachedInputTokens ?? 0,
        inputTokens: input.body.usage.inputTokens,
        outputTokens: input.body.usage.outputTokens,
        reasoningOutputTokens: input.body.usage.reasoningOutputTokens ?? 0,
      },
      costCaveat: 'owner_capacity_provider_cost_unknown',
      usageBasis: 'claude_agent_sdk_turn_completed',
    },
    sourceRefs: {
      ...(input.body.runRef === undefined ? {} : { runRef: input.body.runRef }),
      ...(input.body.sessionRef === undefined
        ? {}
        : { sessionRef: input.body.sessionRef }),
      taskRef: input.body.assignmentRef,
      ...(input.body.workspaceRef === undefined
        ? {}
        : { repositoryRef: input.body.workspaceRef }),
    },
    sourceRoute: PYLON_CODEX_SOURCE_ROUTE,
    tokenCounts: {
      cacheReadTokens: counts.cacheReadTokens,
      cacheWrite1hTokens: 0,
      cacheWrite5mTokens: 0,
      inputTokens: counts.inputTokens,
      outputTokens: counts.outputTokens,
      reasoningTokens: counts.reasoningTokens,
      totalTokens: counts.totalTokens,
    },
    usageTruth: 'exact' as const,
  }
}

const stableClaudeTurnDigest = (
  body: PylonClaudeTurnIngestBody,
): Effect.Effect<string> =>
  Effect.promise(() =>
    sha256Hex(
      [
        'pylon-claude',
        body.assignmentRef,
        body.leaseRef,
        body.pylonRef,
        body.sessionRef ?? 'session.pending',
        String(body.turnIndex),
      ].join(':'),
    ),
  )

// #6391: the Claude own-capacity turn ingest. Authenticates the posting Pylon
// agent, confirms it owns the assignment, then inserts the exact own-capacity
// `token_usage_events` row for the completed Claude Agent SDK turn (idempotent
// by stable digest) and publishes the served-counter delta. Token accounting is
// the proof of this lane; raw-event/trace parity with the Codex lane is tracked
// as follow-up and is fail-soft.
const routeClaudeIngest = <Bindings>(
  dependencies: PylonCodexTurnIngestDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<HttpResponse, PylonCodexRouteError> =>
  Effect.gen(function* () {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const session = yield* requireAgent(dependencies, request, env)

    const rawBody = yield* Effect.tryPromise({
      catch: () =>
        new PylonCodexValidationError({
          reason: 'Request body could not be read.',
        }),
      try: () => request.text(),
    })
    if (new TextEncoder().encode(rawBody).length > MAX_BODY_BYTES) {
      return yield* new PylonCodexValidationError({
        reason: `Pylon Claude turn payload exceeds the ${MAX_BODY_BYTES}-byte limit.`,
      })
    }

    const body = yield* Effect.try({
      catch: error =>
        new PylonCodexValidationError({
          reason:
            error instanceof Error
              ? error.message
              : 'Request body does not match the Pylon Claude turn schema.',
        }),
      try: () =>
        decodeUnknownWithSchema(
          PylonClaudeTurnIngestBody,
          rawBody.trim() === '' ? {} : parseJsonUnknown(rawBody),
        ),
    })

    yield* requireOwnedAssignment(dependencies, env, session, {
      assignmentRef: body.assignmentRef,
      pylonRef: body.pylonRef,
    })

    const ownerUserId = ownerUserIdForAgent(session)
    const observedAt = body.observedAt ?? routeNowIso(dependencies)
    const digest = yield* stableClaudeTurnDigest(body)
    const tokenBody = claudeTokenUsageEventBody({
      body,
      digest,
      observedAt,
      ownerUserId,
      session,
    })
    const counts = codexTurnUsageTokenCounts(body.usage)

    const tokenResult = yield* dependencies
      .ledger(env)
      .ingestEvent(tokenBody)
      .pipe(
        Effect.mapError(
          error =>
            new PylonCodexStorageError({
              operation: 'token_usage_ingest',
              reason: storageReason(error),
            }),
        ),
      )

    if (
      tokenResult.inserted &&
      dependencies.publishDelta !== undefined &&
      counts.inputTokens + counts.outputTokens > 0
    ) {
      yield* dependencies
        .publishDelta(env, {
          eventRef: tokenBody.eventId,
          observedAt,
          tokensServedDelta: counts.inputTokens + counts.outputTokens,
        })
        .pipe(Effect.catch(() => Effect.void))
    }

    return noStoreJsonResponse({
      schemaVersion: 'openagents.pylon.claude_turn_ingest_result.v1',
      assignmentRef: body.assignmentRef,
      insertedTokenUsage: tokenResult.inserted,
      tokensServedDelta: tokenResult.inserted
        ? counts.inputTokens + counts.outputTokens
        : 0,
      tokenUsageEventRef: tokenBody.eventId,
      turnIndex: body.turnIndex,
    })
  })

export const makePylonCodexTurnIngestRoutes = <Bindings>(
  dependencies: PylonCodexTurnIngestDependencies<Bindings>,
) => ({
  handlePylonCodexEventChunkIngestApi: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> => {
    if (new URL(request.url).pathname !== PYLON_CODEX_EVENT_CHUNK_INGEST_PATH) {
      return Effect.succeed(noStoreJsonResponse({ error: 'not_found' }, { status: 404 }))
    }
    return routeEventChunkIngest(dependencies, request, env).pipe(
      Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
    )
  },
  handlePylonCodexAssignmentProofApi: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> => {
    if (new URL(request.url).pathname !== PYLON_CODEX_ASSIGNMENT_PROOF_PATH) {
      return Effect.succeed(noStoreJsonResponse({ error: 'not_found' }, { status: 404 }))
    }
    return routeProof(dependencies, request, env).pipe(
      Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
    )
  },
  handlePylonCodexAssignmentTraceStatusApi: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> => {
    if (
      new URL(request.url).pathname !== PYLON_CODEX_ASSIGNMENT_TRACE_STATUS_PATH
    ) {
      return Effect.succeed(noStoreJsonResponse({ error: 'not_found' }, { status: 404 }))
    }
    return routeTraceStatus(dependencies, request, env).pipe(
      Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
    )
  },
  handlePylonCodexTurnIngestApi: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> => {
    if (new URL(request.url).pathname !== PYLON_CODEX_TURN_INGEST_PATH) {
      return Effect.succeed(noStoreJsonResponse({ error: 'not_found' }, { status: 404 }))
    }
    return routeIngest(dependencies, request, env).pipe(
      Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
    )
  },
  handlePylonCodexLocalUsageIngestApi: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> => {
    if (new URL(request.url).pathname !== PYLON_CODEX_LOCAL_USAGE_INGEST_PATH) {
      return Effect.succeed(noStoreJsonResponse({ error: 'not_found' }, { status: 404 }))
    }
    return routeDirectLocalUsageIngest(dependencies, request, env).pipe(
      Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
    )
  },
  handlePylonClaudeTurnIngestApi: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> => {
    if (new URL(request.url).pathname !== PYLON_CLAUDE_TURN_INGEST_PATH) {
      return Effect.succeed(noStoreJsonResponse({ error: 'not_found' }, { status: 404 }))
    }
    return routeClaudeIngest(dependencies, request, env).pipe(
      Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
    )
  },
})
