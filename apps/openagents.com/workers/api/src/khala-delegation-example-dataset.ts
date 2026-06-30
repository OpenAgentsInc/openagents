import { Schema as S } from 'effect'

import { redactTraceValue } from './inference/trace-redaction'
import { parseJsonRecord, parseJsonUnknown } from './json-boundary'
import { currentIsoTimestamp } from './runtime-primitives'

export const KHALA_DELEGATION_EXAMPLE_SCHEMA =
  'openagents.khala.delegation_example.v0'
export const KHALA_DELEGATION_EXAMPLE_DATASET_SCHEMA =
  'openagents.khala.delegation_example.dataset.v0'

const MAX_ASSIGNMENTS = 100
const MAX_EVENTS_PER_ASSIGNMENT = 100
const MAX_TOKEN_ROWS_PER_ASSIGNMENT = 100
const MAX_TRACE_ROWS_PER_ASSIGNMENT = 25

const SOURCE_TABLES = [
  'pylon_api_assignments',
  'pylon_api_events',
  'token_usage_events',
  'agent_traces',
] as const

export const KhalaDelegationExampleInput = S.Struct({
  taskRefs: S.Array(S.String),
  acceptanceCriteriaRefs: S.Array(S.String),
  resultExpectationRefs: S.Array(S.String),
  artifactRefs: S.Array(S.String),
  proofRefs: S.Array(S.String),
  closeoutRefs: S.Array(S.String),
  publicProjection: S.Record(S.String, S.Unknown),
})
export type KhalaDelegationExampleInput =
  typeof KhalaDelegationExampleInput.Type

export const KhalaDelegationLifecycleEvent = S.Struct({
  eventRef: S.String,
  eventKind: S.String,
  status: S.String,
  createdAt: S.String,
  publicProjection: S.Record(S.String, S.Unknown),
})
export type KhalaDelegationLifecycleEvent =
  typeof KhalaDelegationLifecycleEvent.Type

export const KhalaDelegationTokenUsage = S.Struct({
  eventRef: S.String,
  observedAt: S.String,
  provider: S.String,
  model: S.String,
  usageTruth: S.String,
  demandKind: S.NullOr(S.String),
  demandSource: S.NullOr(S.String),
  demandClient: S.NullOr(S.String),
  runRef: S.NullOr(S.String),
  sessionRef: S.NullOr(S.String),
  taskRef: S.NullOr(S.String),
  repositoryRef: S.NullOr(S.String),
  inputTokens: S.Number,
  outputTokens: S.Number,
  reasoningTokens: S.Number,
  cacheReadTokens: S.Number,
  cacheWriteTokens: S.Number,
  totalTokens: S.Number,
})
export type KhalaDelegationTokenUsage =
  typeof KhalaDelegationTokenUsage.Type

export const KhalaDelegationAtifTrace = S.Struct({
  traceUuid: S.String,
  schemaVersion: S.String,
  trajectoryId: S.String,
  sessionId: S.NullOr(S.String),
  visibility: S.String,
  stepCount: S.Number,
  demandKind: S.NullOr(S.String),
  demandSource: S.NullOr(S.String),
  createdAt: S.String,
  trajectory: S.Unknown,
})
export type KhalaDelegationAtifTrace =
  typeof KhalaDelegationAtifTrace.Type

export const KhalaDelegationRolloutTrace = S.Struct({
  lifecycleEvents: S.Array(KhalaDelegationLifecycleEvent),
  exactTokenUsage: S.Array(KhalaDelegationTokenUsage),
  redactedAtif: S.Array(KhalaDelegationAtifTrace),
})
export type KhalaDelegationRolloutTrace =
  typeof KhalaDelegationRolloutTrace.Type

export const KhalaDelegationOutcome = S.Struct({
  state: S.String,
  acceptedWorkRefs: S.Array(S.String),
  rejectionRefs: S.Array(S.String),
  closeoutRefs: S.Array(S.String),
  pullRequestRefs: S.Array(S.String),
  mergeRefs: S.Array(S.String),
})
export type KhalaDelegationOutcome = typeof KhalaDelegationOutcome.Type

export const KhalaDelegationExample = S.Struct({
  schemaVersion: S.Literal(KHALA_DELEGATION_EXAMPLE_SCHEMA),
  exampleRef: S.String,
  assignmentRef: S.String,
  pylonRef: S.String,
  jobKind: S.String,
  state: S.String,
  paymentMode: S.NullOr(S.String),
  createdAt: S.String,
  updatedAt: S.String,
  input: KhalaDelegationExampleInput,
  rolloutTrace: KhalaDelegationRolloutTrace,
  outcome: KhalaDelegationOutcome,
  evidenceRefs: S.Array(S.String),
})
export type KhalaDelegationExample = typeof KhalaDelegationExample.Type

export const KhalaDelegationExampleDataset = S.Struct({
  schemaVersion: S.Literal(KHALA_DELEGATION_EXAMPLE_DATASET_SCHEMA),
  generatedAt: S.String,
  sourceTables: S.Array(S.Literals(SOURCE_TABLES)),
  publicSafety: S.Struct({
    redactor: S.Literal('@openagentsinc/atif/redaction'),
    rawPromptsIncluded: S.Literal(false),
    rawSecretsIncluded: S.Literal(false),
    rawLocalPathsIncluded: S.Literal(false),
  }),
  examples: S.Array(KhalaDelegationExample),
})
export type KhalaDelegationExampleDataset =
  typeof KhalaDelegationExampleDataset.Type

export class KhalaDelegationExampleDatasetUnsafe extends S.TaggedErrorClass<KhalaDelegationExampleDatasetUnsafe>()(
  'KhalaDelegationExampleDatasetUnsafe',
  {
    reason: S.String,
  },
) {}

const decodeDataset = S.decodeUnknownSync(KhalaDelegationExampleDataset)

const unsafeDatasetMaterialPattern =
  /(\/Users\/|\/home\/|file:\/\/|\.secrets\/|auth\.json|bearer\s+[A-Za-z0-9._~+/=-]{8,}|authorization\s*[:=]|gh[pousr]_[A-Za-z0-9_]{8,}|sk-(?:or-|proj-|ant-)?[A-Za-z0-9_-]{8,}|raw(?:[_\s-]+)(prompt|runner|run[_-]?log|payload|source)|mnemonic|private[_-]?key|provider[_-]?(account|grant|payload|token)|wallet[_\s-]?(seed|private|secret|mnemonic|address|payment))/i

const normalizeString = (value: string | null | undefined): string | null =>
  value === null || value === undefined || value.trim() === ''
    ? null
    : value.trim()

const normalizeCount = (value: number | null | undefined): number =>
  typeof value === 'number' && Number.isFinite(value)
    ? Math.max(0, Math.trunc(value))
    : 0

const uniqueStrings = (
  values: ReadonlyArray<string | null | undefined>,
): ReadonlyArray<string> =>
  [
    ...new Set(
      values.map(normalizeString).filter((v): v is string => v !== null),
    ),
  ]
    .sort()

const safeRefSegment = (value: string): string =>
  value
    .toLowerCase()
    .replace(/[^a-z0-9_.:-]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 96) || 'unknown'

const parseJsonArray = (
  value: string | null | undefined,
): ReadonlyArray<string> => {
  if (value === null || value === undefined || value.trim() === '') {
    return []
  }

  try {
    const parsed = parseJsonUnknown(value)
    return Array.isArray(parsed)
      ? uniqueStrings(
          parsed.filter((item): item is string => typeof item === 'string'),
        )
      : []
  } catch {
    return []
  }
}

const parseOptionalJson = (value: string | null | undefined): unknown => {
  if (value === null || value === undefined || value.trim() === '') {
    return {}
  }

  try {
    return parseJsonUnknown(value)
  } catch {
    return {}
  }
}

const publicProjectionFromJson = (
  value: string | null | undefined,
): Record<string, unknown> => parseJsonRecord(value) ?? {}

const extractRefsFromUnknown = (value: unknown): ReadonlyArray<string> => {
  const refs: Array<string> = []

  const walk = (current: unknown, keyHint = ''): void => {
    if (typeof current === 'string') {
      if (
        /(^|[._:-])(pr|pull|pull_request|merge|merged)([._:-]|$)/i.test(
          current,
        ) ||
        /github\.com\/OpenAgentsInc\/[^/\s]+\/pull\/\d+/i.test(current) ||
        /#\d{1,6}\b/.test(current) ||
        /pull|merge/i.test(keyHint)
      ) {
        refs.push(current)
      }
      return
    }

    if (Array.isArray(current)) {
      current.forEach(item => walk(item, keyHint))
      return
    }

    if (current !== null && typeof current === 'object') {
      for (const [key, child] of Object.entries(
        current as Record<string, unknown>,
      )) {
        walk(child, key)
      }
    }
  }

  walk(value)
  return uniqueStrings(refs)
}

const extractPrOutcomeRefs = (
  assignment: KhalaDelegationAssignmentRow,
): Pick<KhalaDelegationOutcome, 'pullRequestRefs' | 'mergeRefs'> => {
  const candidateRefs = [
    ...parseJsonArray(assignment.artifact_refs_json),
    ...parseJsonArray(assignment.proof_refs_json),
    ...parseJsonArray(assignment.accepted_work_refs_json),
    ...parseJsonArray(assignment.closeout_refs_json),
    ...extractRefsFromUnknown(
      publicProjectionFromJson(assignment.public_projection_json),
    ),
  ]

  return {
    pullRequestRefs: uniqueStrings(
      candidateRefs.filter(ref =>
        /(pull|pull_request|github\.com\/OpenAgentsInc\/[^/\s]+\/pull\/\d+|#\d{1,6})/i.test(
          ref,
        ),
      ),
    ),
    mergeRefs: uniqueStrings(
      candidateRefs.filter(ref => /(merge|merged)/i.test(ref)),
    ),
  }
}

export type KhalaDelegationAssignmentRow = Readonly<{
  assignment_ref: string
  pylon_ref: string
  job_kind: string
  state: string
  payment_mode: string | null
  task_refs_json: string | null
  acceptance_criteria_refs_json: string | null
  result_expectation_refs_json: string | null
  artifact_refs_json: string | null
  proof_refs_json: string | null
  accepted_work_refs_json: string | null
  rejection_refs_json: string | null
  closeout_refs_json: string | null
  public_projection_json: string | null
  created_at: string
  updated_at: string
}>

export type KhalaDelegationEventRow = Readonly<{
  assignment_ref: string
  event_ref: string
  event_kind: string
  status: string
  public_projection_json: string | null
  created_at: string
}>

export type KhalaDelegationTokenUsageRow = Readonly<{
  id: string
  observed_at: string
  run_ref: string | null
  session_ref: string | null
  task_ref: string | null
  repository_ref: string | null
  provider: string
  model: string
  input_tokens: number | null
  output_tokens: number | null
  reasoning_tokens: number | null
  cache_read_tokens: number | null
  cache_write_tokens: number | null
  total_tokens: number | null
  usage_truth: string
  demand_kind: string | null
  demand_source: string | null
  demand_client: string | null
}>

export type KhalaDelegationTraceRow = Readonly<{
  trace_uuid: string
  schema_version: string
  trajectory_id: string
  session_id: string | null
  visibility: string
  step_count: number | null
  trajectory_json: string | null
  demand_kind: string | null
  demand_source: string | null
  created_at: string
}>

export type BuildKhalaDelegationExampleDatasetInput = Readonly<{
  generatedAt: string
  assignments: ReadonlyArray<KhalaDelegationAssignmentRow>
  eventsByAssignmentRef: Readonly<
    Record<string, ReadonlyArray<KhalaDelegationEventRow>>
  >
  tokenRowsByAssignmentRef: Readonly<
    Record<string, ReadonlyArray<KhalaDelegationTokenUsageRow>>
  >
  traceRowsByAssignmentRef: Readonly<
    Record<string, ReadonlyArray<KhalaDelegationTraceRow>>
  >
}>

export type ReadKhalaDelegationExampleDatasetOptions = Readonly<{
  generatedAt?: string
  limit?: number
}>

const buildExample = (
  assignment: KhalaDelegationAssignmentRow,
  events: ReadonlyArray<KhalaDelegationEventRow>,
  tokenRows: ReadonlyArray<KhalaDelegationTokenUsageRow>,
  traceRows: ReadonlyArray<KhalaDelegationTraceRow>,
): KhalaDelegationExample => {
  const outcomeRefs = extractPrOutcomeRefs(assignment)
  const acceptedWorkRefs = parseJsonArray(assignment.accepted_work_refs_json)
  const rejectionRefs = parseJsonArray(assignment.rejection_refs_json)
  const closeoutRefs = parseJsonArray(assignment.closeout_refs_json)
  const artifactRefs = parseJsonArray(assignment.artifact_refs_json)
  const proofRefs = parseJsonArray(assignment.proof_refs_json)

  return {
    schemaVersion: KHALA_DELEGATION_EXAMPLE_SCHEMA,
    exampleRef: `delegation_example.${safeRefSegment(assignment.assignment_ref)}`,
    assignmentRef: assignment.assignment_ref,
    pylonRef: assignment.pylon_ref,
    jobKind: assignment.job_kind,
    state: assignment.state,
    paymentMode: normalizeString(assignment.payment_mode),
    createdAt: assignment.created_at,
    updatedAt: assignment.updated_at,
    input: {
      taskRefs: parseJsonArray(assignment.task_refs_json),
      acceptanceCriteriaRefs: parseJsonArray(
        assignment.acceptance_criteria_refs_json,
      ),
      resultExpectationRefs: parseJsonArray(
        assignment.result_expectation_refs_json,
      ),
      artifactRefs,
      proofRefs,
      closeoutRefs,
      publicProjection: publicProjectionFromJson(
        assignment.public_projection_json,
      ),
    },
    rolloutTrace: {
      lifecycleEvents: events.map(event => ({
        eventRef: event.event_ref,
        eventKind: event.event_kind,
        status: event.status,
        createdAt: event.created_at,
        publicProjection: publicProjectionFromJson(event.public_projection_json),
      })),
      exactTokenUsage: tokenRows.map(row => ({
        eventRef: `token_usage_event:${row.id}`,
        observedAt: row.observed_at,
        provider: row.provider,
        model: row.model,
        usageTruth: row.usage_truth,
        demandKind: normalizeString(row.demand_kind),
        demandSource: normalizeString(row.demand_source),
        demandClient: normalizeString(row.demand_client),
        runRef: normalizeString(row.run_ref),
        sessionRef: normalizeString(row.session_ref),
        taskRef: normalizeString(row.task_ref),
        repositoryRef: normalizeString(row.repository_ref),
        inputTokens: normalizeCount(row.input_tokens),
        outputTokens: normalizeCount(row.output_tokens),
        reasoningTokens: normalizeCount(row.reasoning_tokens),
        cacheReadTokens: normalizeCount(row.cache_read_tokens),
        cacheWriteTokens: normalizeCount(row.cache_write_tokens),
        totalTokens: normalizeCount(row.total_tokens),
      })),
      redactedAtif: traceRows.map(row => ({
        traceUuid: row.trace_uuid,
        schemaVersion: row.schema_version,
        trajectoryId: row.trajectory_id,
        sessionId: normalizeString(row.session_id),
        visibility: row.visibility,
        stepCount: normalizeCount(row.step_count),
        demandKind: normalizeString(row.demand_kind),
        demandSource: normalizeString(row.demand_source),
        createdAt: row.created_at,
        trajectory: parseOptionalJson(row.trajectory_json),
      })),
    },
    outcome: {
      state: assignment.state,
      acceptedWorkRefs,
      rejectionRefs,
      closeoutRefs,
      pullRequestRefs: outcomeRefs.pullRequestRefs,
      mergeRefs: outcomeRefs.mergeRefs,
    },
    evidenceRefs: uniqueStrings([
      ...artifactRefs,
      ...proofRefs,
      ...acceptedWorkRefs,
      ...closeoutRefs,
      ...outcomeRefs.pullRequestRefs,
      ...outcomeRefs.mergeRefs,
    ]),
  }
}

export const assertKhalaDelegationExampleDatasetPublicSafe = (
  dataset: KhalaDelegationExampleDataset,
): void => {
  let unsafe = false
  const walk = (value: unknown): void => {
    if (unsafe) {
      return
    }
    if (typeof value === 'string') {
      unsafe = unsafeDatasetMaterialPattern.test(value)
      return
    }
    if (Array.isArray(value)) {
      value.forEach(walk)
      return
    }
    if (value !== null && typeof value === 'object') {
      Object.values(value as Record<string, unknown>).forEach(walk)
    }
  }

  walk(dataset)

  if (unsafe) {
    throw new KhalaDelegationExampleDatasetUnsafe({
      reason:
        'delegation_example dataset contains raw prompts, secrets, provider tokens, wallet material, private keys, local paths, or raw runner/source payloads.',
    })
  }
}

export const buildKhalaDelegationExampleDataset = (
  input: BuildKhalaDelegationExampleDatasetInput,
): KhalaDelegationExampleDataset => {
  const redacted = redactTraceValue({
    schemaVersion: KHALA_DELEGATION_EXAMPLE_DATASET_SCHEMA,
    generatedAt: input.generatedAt,
    sourceTables: [...SOURCE_TABLES],
    publicSafety: {
      redactor: '@openagentsinc/atif/redaction',
      rawPromptsIncluded: false,
      rawSecretsIncluded: false,
      rawLocalPathsIncluded: false,
    },
    examples: input.assignments.map(assignment =>
      buildExample(
        assignment,
        input.eventsByAssignmentRef[assignment.assignment_ref] ?? [],
        input.tokenRowsByAssignmentRef[assignment.assignment_ref] ?? [],
        input.traceRowsByAssignmentRef[assignment.assignment_ref] ?? [],
      ),
    ),
  }).value
  const dataset = decodeDataset(redacted)
  assertKhalaDelegationExampleDatasetPublicSafe(dataset)
  return dataset
}

export const readKhalaDelegationExampleDataset = async (
  db: D1Database,
  options: ReadKhalaDelegationExampleDatasetOptions = {},
): Promise<KhalaDelegationExampleDataset> => {
  const limit = Math.max(
    1,
    Math.min(MAX_ASSIGNMENTS, Math.trunc(options.limit ?? 25)),
  )
  const assignments = (
    await db
      .prepare(
        `
          SELECT
            assignment_ref,
            pylon_ref,
            job_kind,
            state,
            payment_mode,
            task_refs_json,
            acceptance_criteria_refs_json,
            result_expectation_refs_json,
            artifact_refs_json,
            proof_refs_json,
            accepted_work_refs_json,
            rejection_refs_json,
            closeout_refs_json,
            public_projection_json,
            created_at,
            updated_at
          FROM pylon_api_assignments
          WHERE archived_at IS NULL
            AND job_kind IN ('codex_agent_task', 'claude_agent_task')
          ORDER BY updated_at DESC, assignment_ref ASC
          LIMIT ?
        `,
      )
      .bind(limit)
      .all<KhalaDelegationAssignmentRow>()
  ).results ?? []

  const eventsByAssignmentRef: Record<string, ReadonlyArray<KhalaDelegationEventRow>> =
    {}
  const tokenRowsByAssignmentRef: Record<
    string,
    ReadonlyArray<KhalaDelegationTokenUsageRow>
  > = {}
  const traceRowsByAssignmentRef: Record<string, ReadonlyArray<KhalaDelegationTraceRow>> =
    {}

  for (const assignment of assignments) {
    eventsByAssignmentRef[assignment.assignment_ref] =
      (
        await db
          .prepare(
            `
              SELECT
                assignment_ref,
                event_ref,
                event_kind,
                status,
                public_projection_json,
                created_at
              FROM pylon_api_events
              WHERE archived_at IS NULL
                AND assignment_ref = ?
              ORDER BY created_at ASC, event_ref ASC
              LIMIT ?
            `,
          )
          .bind(assignment.assignment_ref, MAX_EVENTS_PER_ASSIGNMENT)
          .all<KhalaDelegationEventRow>()
      ).results ?? []

    tokenRowsByAssignmentRef[assignment.assignment_ref] =
      (
        await db
          .prepare(
            `
              SELECT
                id,
                observed_at,
                run_ref,
                session_ref,
                task_ref,
                repository_ref,
                provider,
                model,
                input_tokens,
                output_tokens,
                reasoning_tokens,
                cache_read_tokens,
                cache_write_tokens,
                total_tokens,
                usage_truth,
                demand_kind,
                demand_source,
                demand_client
              FROM token_usage_events
              WHERE task_ref = ?
                AND usage_truth = 'exact'
              ORDER BY observed_at ASC, id ASC
              LIMIT ?
            `,
          )
          .bind(assignment.assignment_ref, MAX_TOKEN_ROWS_PER_ASSIGNMENT)
          .all<KhalaDelegationTokenUsageRow>()
      ).results ?? []

    traceRowsByAssignmentRef[assignment.assignment_ref] =
      (
        await db
          .prepare(
            `
              SELECT
                trace_uuid,
                schema_version,
                trajectory_id,
                session_id,
                visibility,
                step_count,
                trajectory_json,
                demand_kind,
                demand_source,
                created_at
              FROM agent_traces
              WHERE substr(trajectory_id, 1, ?) = ?
                 OR substr(trajectory_id, 1, ?) = ?
              ORDER BY created_at ASC, trace_uuid ASC
              LIMIT ?
            `,
          )
          .bind(
            `pylon_codex:${assignment.assignment_ref}:`.length,
            `pylon_codex:${assignment.assignment_ref}:`,
            `pylon_claude:${assignment.assignment_ref}:`.length,
            `pylon_claude:${assignment.assignment_ref}:`,
            MAX_TRACE_ROWS_PER_ASSIGNMENT,
          )
          .all<KhalaDelegationTraceRow>()
      ).results ?? []
  }

  return buildKhalaDelegationExampleDataset({
    generatedAt: options.generatedAt ?? currentIsoTimestamp(),
    assignments,
    eventsByAssignmentRef,
    tokenRowsByAssignmentRef,
    traceRowsByAssignmentRef,
  })
}
