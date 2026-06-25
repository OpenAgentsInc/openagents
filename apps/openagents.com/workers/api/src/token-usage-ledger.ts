import {
  InferenceAnalyticsResponse,
  PublicKhalaTokensServedAggregate,
  PublicKhalaTokensServedHistory,
  type PublicKhalaTokensServedHistoryBucket,
  TokenUsageAggregateResponse,
  TokenUsageCounts,
  type TokenUsageDemandKind,
  TokenUsageEventIngestBody,
  TokenUsageEventRecord,
  type TokenUsageEventRecord as TokenUsageEventRecordType,
  TokenUsageLeaderboardPreferenceResponse,
  TokenUsageLeaderboardPreferenceUpdateBody,
  TokenUsageLeaderboardsResponse,
} from '@openagentsinc/sync-schema'
import { Effect, Layer, Schema as S } from 'effect'
import * as Context from 'effect/Context'

import { OpenAgentsDatabase } from './bindings'
import { isRecord, parseJsonRecord } from './json-boundary'
import { openAgentsDatabase } from './runtime'
import {
  currentIsoTimestamp,
  isoTimestampAfterIso,
  utcStartOfDayIsoTimestamp,
} from './runtime-primitives'

export type TokenUsageLedgerRuntime = Readonly<{
  isoTimestampAfterIso: (timestamp: string, milliseconds: number) => string
  nowIso: () => string
  utcStartOfDayIsoTimestamp: (timestamp: string) => string
}>

export const systemTokenUsageLedgerRuntime: TokenUsageLedgerRuntime = {
  isoTimestampAfterIso,
  nowIso: currentIsoTimestamp,
  utcStartOfDayIsoTimestamp,
}

export type TokenUsageLedgerFilters = Readonly<{
  accountRef?: string | undefined
  actorTeamId?: string | undefined
  actorUserId?: string | undefined
  leaderboardEligible?: boolean | string | undefined
  model?: string | undefined
  privacyOptOut?: boolean | string | undefined
  producerSystem?: string | undefined
  provider?: string | undefined
  since?: string | undefined
  sourceRoute?: string | undefined
  until?: string | undefined
  usageTruth?: string | undefined
}>

export type TokenUsageLeaderboardFilters = Readonly<{
  now?: string | undefined
  until?: string | undefined
  window?: '7d' | '30d' | 'all' | 'today' | string | undefined
}>

export type TokenUsageHistoryFilters = Readonly<{
  bucket?: 'day' | string | undefined
  now?: string | undefined
  window?: '7d' | '30d' | 'all' | 'today' | string | undefined
}>

export type InferenceAnalyticsFilters = Readonly<{
  now?: string | undefined
  window?: '7d' | '30d' | 'all' | 'today' | string | undefined
}>

export type TokenUsageLeaderboardPreferenceInput = Readonly<{
  actorUserId?: string | undefined
  subjectKind: 'account' | 'team' | 'user'
  subjectRef: string
}>

export type TokenUsageIngestResult = Readonly<{
  event: TokenUsageEventRecordType
  inserted: boolean
}>

export class TokenUsageLedgerValidationError extends S.TaggedErrorClass<TokenUsageLedgerValidationError>()(
  'TokenUsageLedgerValidationError',
  {
    field: S.String,
    message: S.String,
  },
) {}

export class TokenUsageLedgerUnsafePayload extends S.TaggedErrorClass<TokenUsageLedgerUnsafePayload>()(
  'TokenUsageLedgerUnsafePayload',
  {
    field: S.String,
    reason: S.String,
  },
) {}

export class TokenUsageLedgerStorageError extends S.TaggedErrorClass<TokenUsageLedgerStorageError>()(
  'TokenUsageLedgerStorageError',
  {
    operation: S.String,
    error: S.Defect,
  },
) {}

export type TokenUsageLedgerError =
  | TokenUsageLedgerStorageError
  | TokenUsageLedgerUnsafePayload
  | TokenUsageLedgerValidationError

export type TokenUsageLedgerShape = Readonly<{
  ingestEvent: (
    body: unknown,
  ) => Effect.Effect<TokenUsageIngestResult, TokenUsageLedgerError>
  readAggregates: (
    filters?: TokenUsageLedgerFilters,
  ) => Effect.Effect<
    typeof TokenUsageAggregateResponse.Type,
    TokenUsageLedgerStorageError | TokenUsageLedgerValidationError
  >
  readInferenceAnalytics: (
    filters?: InferenceAnalyticsFilters,
  ) => Effect.Effect<
    typeof InferenceAnalyticsResponse.Type,
    TokenUsageLedgerStorageError | TokenUsageLedgerValidationError
  >
  readPublicTokensServed: () => Effect.Effect<
    typeof PublicKhalaTokensServedAggregate.Type,
    TokenUsageLedgerStorageError | TokenUsageLedgerValidationError
  >
  readPublicTokensServedHistory: (
    filters?: TokenUsageHistoryFilters,
  ) => Effect.Effect<
    typeof PublicKhalaTokensServedHistory.Type,
    TokenUsageLedgerStorageError | TokenUsageLedgerValidationError
  >
  readLeaderboardPreference: (
    input: TokenUsageLeaderboardPreferenceInput,
  ) => Effect.Effect<
    typeof TokenUsageLeaderboardPreferenceResponse.Type,
    TokenUsageLedgerStorageError | TokenUsageLedgerValidationError
  >
  readLeaderboards: (
    filters?: TokenUsageLeaderboardFilters,
  ) => Effect.Effect<
    typeof TokenUsageLeaderboardsResponse.Type,
    TokenUsageLedgerStorageError | TokenUsageLedgerValidationError
  >
  updateLeaderboardPreference: (
    input: TokenUsageLeaderboardPreferenceInput,
    body: unknown,
  ) => Effect.Effect<
    typeof TokenUsageLeaderboardPreferenceResponse.Type,
    TokenUsageLedgerStorageError | TokenUsageLedgerValidationError
  >
}>

type TokenUsageEventRow = Readonly<{
  id: string
  idempotency_key: string
  observed_at: string
  ingested_at: string
  producer_system: string
  source_route: string
  actor_user_id: string | null
  actor_team_id: string | null
  account_ref: string | null
  anonymized_source_ref: string | null
  run_ref: string | null
  session_ref: string | null
  task_ref: string | null
  repository_ref: string | null
  provider: string | null
  model: string | null
  backend_profile: string | null
  input_tokens: number | null
  output_tokens: number | null
  reasoning_tokens: number | null
  cache_read_tokens: number | null
  cache_write_5m_tokens: number | null
  cache_write_1h_tokens: number | null
  total_tokens: number | null
  usage_truth: string
  cost_amount: number | null
  currency: string | null
  demand_kind: string | null
  demand_source: string | null
  demand_client: string | null
  leaderboard_eligible: number | null
  privacy_opt_out: number | null
  safe_metadata_json: string | null
}>

type TokenUsageCountRow = Readonly<{
  input_tokens: number | null
  output_tokens: number | null
  reasoning_tokens: number | null
  cache_read_tokens: number | null
  cache_write_5m_tokens: number | null
  cache_write_1h_tokens: number | null
  total_tokens: number | null
  usage_events?: number | null
}>

type TokenUsageGroupRow = TokenUsageCountRow &
  Readonly<{
    key: string | null
    label: string | null
  }>

type TokenUsageActorGroupRow = TokenUsageCountRow &
  Readonly<{
    account_ref: string | null
    actor_team_id: string | null
    actor_user_id: string | null
  }>

type TokenUsageLeaderboardPreferenceRow = Readonly<{
  leaderboard_participation: string
  leaderboard_visibility: string
  subject_kind: string
  subject_ref: string
  updated_at: string
  updated_by_user_id: string | null
}>

const unsafeKeyPattern =
  /(^|[_-])(access[_-]?token|api[_-]?key|authorization|bearer[_-]?token|callback[_-]?(token|url)|code[_-]?verifier|completion|cookie|credential|device[_-]?auth|private[_-]?(key|path|repo|source|trace)|prompt|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(auth|completion|log|payload|prompt|provider|response|source|text|trace)|refresh[_-]?token|secret|source[_-]?code|tool[_-]?args)$/i

const unsafeValuePattern =
  /(@|\/Users\/|\/home\/|Bearer\s+[A-Za-z0-9._-]{8,}|authorization:\s*bearer|access[_-]?token=|api[_-]?key=|callback[_-]?token|callback[_-]?url|cookie=|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|opencode_auth_content|private[_-]?(key|repo|source)|provider[_-]?(credential|grant|payload|secret|token)|raw[_-]?(completion|payload|prompt|provider|response|source|text|trace)|refresh[_-]?token|secret|sk-[a-z0-9]|source[_-]?archive|wallet[_-]?(key|mnemonic|secret|seed))/i

const optionalText = (value: string | undefined): string | undefined => {
  const trimmed = value?.trim()

  return trimmed === undefined || trimmed === '' ? undefined : trimmed
}

const demandKindFromText = (
  value: string | null | undefined,
): TokenUsageDemandKind => {
  const normalized = value?.trim().toLowerCase()

  return normalized === 'external' || normalized === 'internal'
    ? normalized
    : 'unlabeled'
}

const demandAttributionFromInput = (
  body: typeof TokenUsageEventIngestBody.Type,
): Readonly<{
  demandClient: string | null
  demandKind: TokenUsageDemandKind
  demandSource: string | null
}> => ({
  demandClient: optionalText(body.demand?.demandClient) ?? null,
  demandKind: demandKindFromText(body.demand?.demandKind),
  demandSource: optionalText(body.demand?.demandSource) ?? null,
})

const validateTimestamp = (
  field: string,
  value: string | undefined,
): Effect.Effect<string | undefined, TokenUsageLedgerValidationError> => {
  const text = optionalText(value)

  if (text === undefined) {
    return Effect.sync((): string | undefined => undefined)
  }

  return Number.isFinite(Date.parse(text))
    ? Effect.succeed(text)
    : Effect.fail(
        new TokenUsageLedgerValidationError({
          field,
          message: `${field} must be an ISO-compatible timestamp.`,
        }),
      )
}

const requireTimestamp = (
  field: string,
  value: string,
): Effect.Effect<string, TokenUsageLedgerValidationError> =>
  Effect.flatMap(validateTimestamp(field, value), timestamp =>
    timestamp === undefined
      ? Effect.fail(
          new TokenUsageLedgerValidationError({
            field,
            message: `${field} is required.`,
          }),
        )
      : Effect.succeed(timestamp),
  )

const nonNegativeInteger = (
  field: string,
  value: number,
): Effect.Effect<number, TokenUsageLedgerValidationError> =>
  Number.isInteger(value) && value >= 0
    ? Effect.succeed(value)
    : Effect.fail(
        new TokenUsageLedgerValidationError({
          field,
          message: `${field} must be a non-negative integer.`,
        }),
      )

const normalizeCounts = (
  value: typeof TokenUsageCounts.Type,
): Effect.Effect<
  typeof TokenUsageCounts.Type,
  TokenUsageLedgerValidationError
> =>
  Effect.gen(function* () {
    const inputTokens = yield* nonNegativeInteger(
      'tokenCounts.inputTokens',
      value.inputTokens,
    )
    const outputTokens = yield* nonNegativeInteger(
      'tokenCounts.outputTokens',
      value.outputTokens,
    )
    const reasoningTokens = yield* nonNegativeInteger(
      'tokenCounts.reasoningTokens',
      value.reasoningTokens,
    )
    const cacheReadTokens = yield* nonNegativeInteger(
      'tokenCounts.cacheReadTokens',
      value.cacheReadTokens,
    )
    const cacheWrite5mTokens = yield* nonNegativeInteger(
      'tokenCounts.cacheWrite5mTokens',
      value.cacheWrite5mTokens,
    )
    const cacheWrite1hTokens = yield* nonNegativeInteger(
      'tokenCounts.cacheWrite1hTokens',
      value.cacheWrite1hTokens,
    )
    const explicitTotalTokens = yield* nonNegativeInteger(
      'tokenCounts.totalTokens',
      value.totalTokens,
    )
    const computedTotalTokens =
      inputTokens +
      outputTokens +
      reasoningTokens +
      cacheReadTokens +
      cacheWrite5mTokens +
      cacheWrite1hTokens

    return {
      cacheReadTokens,
      cacheWrite1hTokens,
      cacheWrite5mTokens,
      inputTokens,
      outputTokens,
      reasoningTokens,
      totalTokens:
        explicitTotalTokens > 0 ? explicitTotalTokens : computedTotalTokens,
    }
  })

const countsFromRow = (
  row: TokenUsageCountRow | null | undefined,
): typeof TokenUsageCounts.Type => ({
  cacheReadTokens: row?.cache_read_tokens ?? 0,
  cacheWrite1hTokens: row?.cache_write_1h_tokens ?? 0,
  cacheWrite5mTokens: row?.cache_write_5m_tokens ?? 0,
  inputTokens: row?.input_tokens ?? 0,
  outputTokens: row?.output_tokens ?? 0,
  reasoningTokens: row?.reasoning_tokens ?? 0,
  totalTokens: row?.total_tokens ?? 0,
})

// Round a USD cost SUM to a stable 6 dp so a stored REAL carries no
// floating-point noise across the analytics rollups. Non-negative.
const roundCostUsd = (value: number): number =>
  Math.round(Math.max(0, value) * 1_000_000) / 1_000_000

const isUnsafeKey = (key: string): boolean => unsafeKeyPattern.test(key)

const validateSafePayload = (
  value: unknown,
  path = '$',
): Effect.Effect<void, TokenUsageLedgerUnsafePayload> => {
  if (typeof value === 'string') {
    return unsafeValuePattern.test(value)
      ? Effect.fail(
          new TokenUsageLedgerUnsafePayload({
            field: path,
            reason:
              'Value resembles private source, prompt, credential, or provider material.',
          }),
        )
      : Effect.void
  }

  if (Array.isArray(value)) {
    return Effect.forEach(
      value,
      (item, index) => validateSafePayload(item, `${path}[${index}]`),
      { discard: true },
    )
  }

  if (!isRecord(value)) {
    return Effect.sync((): void => undefined)
  }

  return Effect.forEach(
    Object.entries(value),
    ([key, item]) =>
      isUnsafeKey(key)
        ? Effect.fail(
            new TokenUsageLedgerUnsafePayload({
              field: `${path}.${key}`,
              reason: 'Field name is not allowed in token usage events.',
            }),
          )
        : validateSafePayload(item, `${path}.${key}`),
    { discard: true },
  )
}

const jsonStringifyRecord = (
  field: string,
  value: Record<string, unknown>,
): Effect.Effect<string, TokenUsageLedgerValidationError> =>
  Effect.try({
    try: () => JSON.stringify(value),
    catch: error =>
      new TokenUsageLedgerValidationError({
        field,
        message:
          error instanceof Error
            ? error.message
            : 'Value is not JSON serializable.',
      }),
  })

const decodeIngestBody = (
  value: unknown,
): Effect.Effect<
  typeof TokenUsageEventIngestBody.Type,
  TokenUsageLedgerValidationError
> =>
  Effect.try({
    try: () => S.decodeUnknownSync(TokenUsageEventIngestBody)(value),
    catch: error =>
      new TokenUsageLedgerValidationError({
        field: 'body',
        message: error instanceof Error ? error.message : String(error),
      }),
  })

const decodeRecord = (
  value: unknown,
): Effect.Effect<TokenUsageEventRecordType, TokenUsageLedgerValidationError> =>
  Effect.try({
    try: () => S.decodeUnknownSync(TokenUsageEventRecord)(value),
    catch: error =>
      new TokenUsageLedgerValidationError({
        field: 'stored_event',
        message: error instanceof Error ? error.message : String(error),
      }),
  })

const decodeAggregateResponse = (
  value: unknown,
): Effect.Effect<
  typeof TokenUsageAggregateResponse.Type,
  TokenUsageLedgerValidationError
> =>
  Effect.try({
    try: () => S.decodeUnknownSync(TokenUsageAggregateResponse)(value),
    catch: error =>
      new TokenUsageLedgerValidationError({
        field: 'aggregate_response',
        message: error instanceof Error ? error.message : String(error),
      }),
  })

const decodeInferenceAnalyticsResponse = (
  value: unknown,
): Effect.Effect<
  typeof InferenceAnalyticsResponse.Type,
  TokenUsageLedgerValidationError
> =>
  Effect.try({
    try: () => S.decodeUnknownSync(InferenceAnalyticsResponse)(value),
    catch: error =>
      new TokenUsageLedgerValidationError({
        field: 'inference_analytics_response',
        message: error instanceof Error ? error.message : String(error),
      }),
  })

const decodePublicTokensServedAggregate = (
  value: unknown,
): Effect.Effect<
  typeof PublicKhalaTokensServedAggregate.Type,
  TokenUsageLedgerValidationError
> =>
  Effect.try({
    try: () => S.decodeUnknownSync(PublicKhalaTokensServedAggregate)(value),
    catch: error =>
      new TokenUsageLedgerValidationError({
        field: 'public_tokens_served_aggregate',
        message: error instanceof Error ? error.message : String(error),
      }),
  })

const decodePublicTokensServedHistory = (
  value: unknown,
): Effect.Effect<
  typeof PublicKhalaTokensServedHistory.Type,
  TokenUsageLedgerValidationError
> =>
  Effect.try({
    try: () => S.decodeUnknownSync(PublicKhalaTokensServedHistory)(value),
    catch: error =>
      new TokenUsageLedgerValidationError({
        field: 'public_tokens_served_history',
        message: error instanceof Error ? error.message : String(error),
      }),
  })

const decodeLeaderboardsResponse = (
  value: unknown,
): Effect.Effect<
  typeof TokenUsageLeaderboardsResponse.Type,
  TokenUsageLedgerValidationError
> =>
  Effect.try({
    try: () => S.decodeUnknownSync(TokenUsageLeaderboardsResponse)(value),
    catch: error =>
      new TokenUsageLedgerValidationError({
        field: 'leaderboards_response',
        message: error instanceof Error ? error.message : String(error),
      }),
  })

const decodeLeaderboardPreferenceResponse = (
  value: unknown,
): Effect.Effect<
  typeof TokenUsageLeaderboardPreferenceResponse.Type,
  TokenUsageLedgerValidationError
> =>
  Effect.try({
    try: () =>
      S.decodeUnknownSync(TokenUsageLeaderboardPreferenceResponse)(value),
    catch: error =>
      new TokenUsageLedgerValidationError({
        field: 'leaderboard_preference_response',
        message: error instanceof Error ? error.message : String(error),
      }),
  })

const decodeLeaderboardPreferenceUpdateBody = (
  value: unknown,
): Effect.Effect<
  typeof TokenUsageLeaderboardPreferenceUpdateBody.Type,
  TokenUsageLedgerValidationError
> =>
  Effect.try({
    try: () =>
      S.decodeUnknownSync(TokenUsageLeaderboardPreferenceUpdateBody)(value),
    catch: error =>
      new TokenUsageLedgerValidationError({
        field: 'leaderboard_preference_body',
        message: error instanceof Error ? error.message : String(error),
      }),
  })

const rowToRecord = (
  row: TokenUsageEventRow,
): Effect.Effect<
  TokenUsageEventRecordType,
  TokenUsageLedgerValidationError
> => {
  const safeMetadata = parseJsonRecord(row.safe_metadata_json) ?? {}

  return decodeRecord({
    schemaVersion: 'openagents.token_usage_event.record.v1',
    actor: {
      ...(row.account_ref === null ? {} : { accountRef: row.account_ref }),
      ...(row.actor_team_id === null ? {} : { teamId: row.actor_team_id }),
      ...(row.actor_user_id === null ? {} : { userId: row.actor_user_id }),
    },
    backendProfile: row.backend_profile,
    cost:
      row.cost_amount === null || row.currency === null
        ? null
        : { amount: row.cost_amount, currency: row.currency },
    demand: {
      ...(row.demand_client == null ? {} : { demandClient: row.demand_client }),
      demandKind: demandKindFromText(row.demand_kind),
      ...(row.demand_source == null ? {} : { demandSource: row.demand_source }),
    },
    eventId: row.id,
    idempotencyKey: row.idempotency_key,
    ingestedAt: row.ingested_at,
    model: row.model,
    observedAt: row.observed_at,
    privacy: {
      leaderboardEligible: (row.leaderboard_eligible ?? 0) === 1,
      privacyOptOut: (row.privacy_opt_out ?? 0) === 1,
    },
    producerSystem: row.producer_system,
    provider: row.provider,
    safeMetadata,
    sourceRefs: {
      ...(row.anonymized_source_ref === null
        ? {}
        : { anonymizedSourceRef: row.anonymized_source_ref }),
      ...(row.repository_ref === null
        ? {}
        : { repositoryRef: row.repository_ref }),
      ...(row.run_ref === null ? {} : { runRef: row.run_ref }),
      ...(row.session_ref === null ? {} : { sessionRef: row.session_ref }),
      ...(row.task_ref === null ? {} : { taskRef: row.task_ref }),
    },
    sourceRoute: row.source_route,
    tokenCounts: countsFromRow(row),
    usageTruth: row.usage_truth,
  })
}

const storedRowFromInput = (
  body: typeof TokenUsageEventIngestBody.Type,
  input: Readonly<{
    ingestedAt: string
    safeMetadataJson: string
    tokenCounts: typeof TokenUsageCounts.Type
  }>,
): TokenUsageEventRow => {
  const demand = demandAttributionFromInput(body)

  return {
    account_ref: body.actor?.accountRef ?? null,
    actor_team_id: body.actor?.teamId ?? null,
    actor_user_id: body.actor?.userId ?? null,
    anonymized_source_ref: body.sourceRefs?.anonymizedSourceRef ?? null,
    backend_profile: body.backendProfile ?? null,
    cache_read_tokens: input.tokenCounts.cacheReadTokens,
    cache_write_1h_tokens: input.tokenCounts.cacheWrite1hTokens,
    cache_write_5m_tokens: input.tokenCounts.cacheWrite5mTokens,
    cost_amount: body.cost?.amount ?? null,
    currency: body.cost?.currency ?? null,
    demand_client: demand.demandClient,
    demand_kind: demand.demandKind,
    demand_source: demand.demandSource,
    id: body.eventId,
    idempotency_key: body.idempotencyKey,
    ingested_at: input.ingestedAt,
    input_tokens: input.tokenCounts.inputTokens,
    leaderboard_eligible: body.privacy?.leaderboardEligible === false ? 0 : 1,
    model: body.model ?? null,
    observed_at: body.observedAt,
    output_tokens: input.tokenCounts.outputTokens,
    privacy_opt_out: body.privacy?.privacyOptOut === true ? 1 : 0,
    producer_system: body.producerSystem,
    provider: body.provider ?? null,
    reasoning_tokens: input.tokenCounts.reasoningTokens,
    repository_ref: body.sourceRefs?.repositoryRef ?? null,
    run_ref: body.sourceRefs?.runRef ?? null,
    safe_metadata_json: input.safeMetadataJson,
    session_ref: body.sourceRefs?.sessionRef ?? null,
    source_route: body.sourceRoute,
    task_ref: body.sourceRefs?.taskRef ?? null,
    total_tokens: input.tokenCounts.totalTokens,
    usage_truth: body.usageTruth,
  }
}

const insertBindings = (
  row: TokenUsageEventRow,
): ReadonlyArray<string | number | null> => [
  row.id,
  row.idempotency_key,
  row.observed_at,
  row.ingested_at,
  row.producer_system,
  row.source_route,
  row.actor_user_id,
  row.actor_team_id,
  row.account_ref,
  row.anonymized_source_ref,
  row.run_ref,
  row.session_ref,
  row.task_ref,
  row.repository_ref,
  row.provider,
  row.model,
  row.backend_profile,
  row.input_tokens,
  row.output_tokens,
  row.reasoning_tokens,
  row.cache_read_tokens,
  row.cache_write_5m_tokens,
  row.cache_write_1h_tokens,
  row.total_tokens,
  row.usage_truth,
  row.cost_amount,
  row.currency,
  row.demand_kind,
  row.demand_source,
  row.demand_client,
  row.leaderboard_eligible,
  row.privacy_opt_out,
  row.safe_metadata_json,
]

const isUniqueConstraintError = (error: unknown): boolean =>
  error instanceof Error && error.message.includes('UNIQUE constraint failed')

const validateBooleanFilter = (
  field: string,
  value: boolean | string | undefined,
): Effect.Effect<boolean | void, TokenUsageLedgerValidationError> => {
  if (value === undefined || typeof value === 'boolean') {
    return Effect.succeed(value)
  }

  const normalized = value.trim().toLowerCase()

  if (normalized === '') {
    return Effect.void
  }

  if (normalized === 'true' || normalized === '1') {
    return Effect.succeed(true)
  }

  if (normalized === 'false' || normalized === '0') {
    return Effect.succeed(false)
  }

  return Effect.fail(
    new TokenUsageLedgerValidationError({
      field,
      message: `${field} must be true or false.`,
    }),
  )
}

const applyTextFilter = (
  clauses: Array<string>,
  values: Array<number | string>,
  column: string,
  value: string | undefined,
): void => {
  const text = optionalText(value)

  if (text === undefined) {
    return
  }

  clauses.push(`${column} = ?`)
  values.push(text)
}

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, TokenUsageLedgerStorageError> =>
  Effect.tryPromise({
    try: run,
    catch: error => new TokenUsageLedgerStorageError({ operation, error }),
  })

const findExistingEvent = (
  db: D1Database,
  input: Readonly<{ eventId: string; idempotencyKey: string }>,
): Effect.Effect<
  TokenUsageEventRecordType | undefined,
  TokenUsageLedgerStorageError | TokenUsageLedgerValidationError
> =>
  Effect.gen(function* () {
    const row = yield* d1Effect('tokenUsageEvents.findExisting', () =>
      db
        .prepare(
          `SELECT *
             FROM token_usage_events
            WHERE idempotency_key = ? OR id = ?
            ORDER BY ingested_at ASC
            LIMIT 1`,
        )
        .bind(input.idempotencyKey, input.eventId)
        .first<TokenUsageEventRow>(),
    )

    if (row === null) {
      return undefined
    }

    return yield* rowToRecord(row)
  })

const insertEventRow = (
  db: D1Database,
  row: TokenUsageEventRow,
): Effect.Effect<void, TokenUsageLedgerStorageError> =>
  d1Effect('tokenUsageEvents.insert', () =>
    db
      .prepare(
        `INSERT INTO token_usage_events (
          id,
          idempotency_key,
          observed_at,
          ingested_at,
          producer_system,
          source_route,
          actor_user_id,
          actor_team_id,
          account_ref,
          anonymized_source_ref,
          run_ref,
          session_ref,
          task_ref,
          repository_ref,
          provider,
          model,
          backend_profile,
          input_tokens,
          output_tokens,
          reasoning_tokens,
          cache_read_tokens,
          cache_write_5m_tokens,
          cache_write_1h_tokens,
          total_tokens,
          usage_truth,
          cost_amount,
          currency,
          demand_kind,
          demand_source,
          demand_client,
          leaderboard_eligible,
          privacy_opt_out,
          safe_metadata_json
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(...insertBindings(row))
      .run(),
  ).pipe(Effect.asVoid)

const aggregateWhere = (
  filters: TokenUsageLedgerFilters,
): Effect.Effect<
  Readonly<{
    filters: TokenUsageLedgerFilters
    sql: string
    values: ReadonlyArray<number | string>
  }>,
  TokenUsageLedgerValidationError
> =>
  Effect.gen(function* () {
    const since = yield* validateTimestamp('since', filters.since)
    const until = yield* validateTimestamp('until', filters.until)
    const leaderboardEligible = yield* validateBooleanFilter(
      'leaderboardEligible',
      filters.leaderboardEligible,
    )
    const privacyOptOut = yield* validateBooleanFilter(
      'privacyOptOut',
      filters.privacyOptOut,
    )
    const clauses: Array<string> = []
    const values: Array<number | string> = []

    if (since !== undefined) {
      clauses.push('observed_at >= ?')
      values.push(since)
    }

    if (until !== undefined) {
      clauses.push('observed_at <= ?')
      values.push(until)
    }

    applyTextFilter(clauses, values, 'provider', filters.provider)
    applyTextFilter(clauses, values, 'model', filters.model)
    applyTextFilter(clauses, values, 'producer_system', filters.producerSystem)
    applyTextFilter(clauses, values, 'source_route', filters.sourceRoute)
    applyTextFilter(clauses, values, 'actor_user_id', filters.actorUserId)
    applyTextFilter(clauses, values, 'actor_team_id', filters.actorTeamId)
    applyTextFilter(clauses, values, 'account_ref', filters.accountRef)
    applyTextFilter(clauses, values, 'usage_truth', filters.usageTruth)

    if (leaderboardEligible !== undefined) {
      clauses.push('leaderboard_eligible = ?')
      values.push(leaderboardEligible ? 1 : 0)
    }

    if (privacyOptOut !== undefined) {
      clauses.push('privacy_opt_out = ?')
      values.push(privacyOptOut ? 1 : 0)
    }

    return {
      filters: {
        ...(optionalText(filters.accountRef) === undefined
          ? {}
          : { accountRef: optionalText(filters.accountRef) }),
        ...(optionalText(filters.actorTeamId) === undefined
          ? {}
          : { actorTeamId: optionalText(filters.actorTeamId) }),
        ...(optionalText(filters.actorUserId) === undefined
          ? {}
          : { actorUserId: optionalText(filters.actorUserId) }),
        ...(leaderboardEligible === undefined ? {} : { leaderboardEligible }),
        ...(optionalText(filters.model) === undefined
          ? {}
          : { model: optionalText(filters.model) }),
        ...(privacyOptOut === undefined ? {} : { privacyOptOut }),
        ...(optionalText(filters.producerSystem) === undefined
          ? {}
          : { producerSystem: optionalText(filters.producerSystem) }),
        ...(optionalText(filters.provider) === undefined
          ? {}
          : { provider: optionalText(filters.provider) }),
        ...(since === undefined ? {} : { since }),
        ...(optionalText(filters.sourceRoute) === undefined
          ? {}
          : { sourceRoute: optionalText(filters.sourceRoute) }),
        ...(until === undefined ? {} : { until }),
        ...(optionalText(filters.usageTruth) === undefined
          ? {}
          : { usageTruth: optionalText(filters.usageTruth) }),
      },
      sql: clauses.length === 0 ? '' : `WHERE ${clauses.join(' AND ')}`,
      values,
    }
  })

const aggregateGroupRow = (row: TokenUsageGroupRow) => ({
  key: row.key ?? 'unknown',
  label: row.label ?? 'Unknown',
  tokenCounts: countsFromRow(row),
  usageEvents: row.usage_events ?? 0,
})

const aggregateActorRow = (row: TokenUsageActorGroupRow) => ({
  accountRef: row.account_ref,
  anonymous:
    row.account_ref === null &&
    row.actor_team_id === null &&
    row.actor_user_id === null,
  teamId: row.actor_team_id,
  tokenCounts: countsFromRow(row),
  usageEvents: row.usage_events ?? 0,
  userId: row.actor_user_id,
})

const defaultPreferenceRow = (
  input: TokenUsageLeaderboardPreferenceInput,
  updatedAt: string,
): TokenUsageLeaderboardPreferenceRow => ({
  leaderboard_participation: 'eligible',
  leaderboard_visibility: 'internal',
  subject_kind: input.subjectKind,
  subject_ref: input.subjectRef,
  updated_at: updatedAt,
  updated_by_user_id: input.actorUserId ?? null,
})

const preferenceResponseFromRow = (row: TokenUsageLeaderboardPreferenceRow) =>
  decodeLeaderboardPreferenceResponse({
    schemaVersion: 'openagents.token_usage_leaderboard_preference.v1',
    preference: {
      leaderboardParticipation: row.leaderboard_participation,
      leaderboardVisibility: row.leaderboard_visibility,
      subjectKind: row.subject_kind,
      subjectRef: row.subject_ref,
      updatedAt: row.updated_at,
      updatedByUserId: row.updated_by_user_id,
    },
  })

const normalizeLeaderboardWindow = (
  value: string | undefined,
): Effect.Effect<
  '7d' | '30d' | 'all' | 'today',
  TokenUsageLedgerValidationError
> => {
  const window = optionalText(value) ?? '7d'

  return window === 'today' ||
    window === '7d' ||
    window === '30d' ||
    window === 'all'
    ? Effect.succeed(window)
    : Effect.fail(
        new TokenUsageLedgerValidationError({
          field: 'window',
          message: 'window must be today, 7d, 30d, or all.',
        }),
      )
}

const leaderboardWindowSince = (
  window: '7d' | '30d' | 'all' | 'today',
  nowIso: string,
  runtime: TokenUsageLedgerRuntime,
): string | undefined => {
  if (window === 'all') {
    return undefined
  }

  if (window === 'today') {
    return runtime.utcStartOfDayIsoTimestamp(nowIso)
  }

  const dayMilliseconds = 24 * 60 * 60 * 1000
  const days = window === '30d' ? 30 : 7

  return runtime.isoTimestampAfterIso(nowIso, -days * dayMilliseconds)
}

const normalizeHistoryBucket = (
  value: string | undefined,
): Effect.Effect<
  PublicKhalaTokensServedHistoryBucket,
  TokenUsageLedgerValidationError
> => {
  const bucket = optionalText(value) ?? 'day'

  return bucket === 'day'
    ? Effect.succeed('day')
    : Effect.fail(
        new TokenUsageLedgerValidationError({
          field: 'bucket',
          message: 'bucket must be day.',
        }),
      )
}

type AggregateWhere = Readonly<{
  filters: TokenUsageLeaderboardFilters | TokenUsageLedgerFilters
  sql: string
  values: ReadonlyArray<number | string>
}>

const whereSqlWithExtra = (where: AggregateWhere, extra: string): string =>
  where.sql === '' ? `WHERE ${extra}` : `${where.sql} AND ${extra}`

const leaderboardEligibleSql = (where: AggregateWhere, extra: string): string =>
  `${whereSqlWithExtra(where, extra)}
     AND leaderboard_eligible = 1
     AND privacy_opt_out = 0
     AND NOT EXISTS (
       SELECT 1
         FROM token_usage_leaderboard_preferences preferences
        WHERE preferences.subject_kind = 'user'
          AND preferences.subject_ref = token_usage_events.actor_user_id
          AND (
            preferences.leaderboard_participation = 'opted_out'
            OR preferences.leaderboard_visibility = 'private'
          )
     )
     AND NOT EXISTS (
       SELECT 1
         FROM token_usage_leaderboard_preferences preferences
        WHERE preferences.subject_kind = 'team'
          AND preferences.subject_ref = token_usage_events.actor_team_id
          AND (
            preferences.leaderboard_participation = 'opted_out'
            OR preferences.leaderboard_visibility = 'private'
          )
     )
     AND NOT EXISTS (
       SELECT 1
         FROM token_usage_leaderboard_preferences preferences
        WHERE preferences.subject_kind = 'account'
          AND preferences.subject_ref = token_usage_events.account_ref
          AND (
            preferences.leaderboard_participation = 'opted_out'
            OR preferences.leaderboard_visibility = 'private'
          )
     )`

const leaderboardWhere = (
  filters: TokenUsageLeaderboardFilters,
  runtime: TokenUsageLedgerRuntime,
): Effect.Effect<AggregateWhere, TokenUsageLedgerValidationError> =>
  Effect.gen(function* () {
    const window = yield* normalizeLeaderboardWindow(filters.window)
    const nowIso = yield* requireTimestamp(
      'now',
      filters.now ?? runtime.nowIso(),
    )
    const since = leaderboardWindowSince(window, nowIso, runtime)
    const until = yield* validateTimestamp('until', filters.until)
    const clauses: Array<string> = []
    const values: Array<number | string> = []

    if (since !== undefined) {
      clauses.push('observed_at >= ?')
      values.push(since)
    }

    if (until !== undefined) {
      clauses.push('observed_at <= ?')
      values.push(until)
    }

    return {
      filters: {
        ...(since === undefined ? {} : { since }),
        ...(until === undefined ? {} : { until }),
        window,
      },
      sql: clauses.length === 0 ? '' : `WHERE ${clauses.join(' AND ')}`,
      values,
    }
  })

const sourceRefSpecs = [
  {
    column: 'anonymized_source_ref',
    kind: 'anonymized',
    label: 'anonymized',
  },
  { column: 'repository_ref', kind: 'repository', label: 'repository' },
  { column: 'run_ref', kind: 'run', label: 'run' },
  { column: 'session_ref', kind: 'session', label: 'session' },
  { column: 'task_ref', kind: 'task', label: 'task' },
] as const

const aggregateSourceRefRows = (
  db: D1Database,
  where: AggregateWhere,
): Effect.Effect<
  ReadonlyArray<TokenUsageGroupRow>,
  TokenUsageLedgerStorageError
> =>
  Effect.gen(function* () {
    const rows = yield* Effect.forEach(sourceRefSpecs, spec =>
      d1Effect(`tokenUsageEvents.aggregate.${spec.kind}Ref`, () =>
        db
          .prepare(
            `SELECT
                '${spec.kind}:' || ${spec.column} AS key,
                '${spec.label} / ' || ${spec.column} AS label,
                COALESCE(SUM(input_tokens), 0) AS input_tokens,
                COALESCE(SUM(output_tokens), 0) AS output_tokens,
                COALESCE(SUM(reasoning_tokens), 0) AS reasoning_tokens,
                COALESCE(SUM(cache_read_tokens), 0) AS cache_read_tokens,
                COALESCE(SUM(cache_write_5m_tokens), 0) AS cache_write_5m_tokens,
                COALESCE(SUM(cache_write_1h_tokens), 0) AS cache_write_1h_tokens,
                COALESCE(SUM(total_tokens), 0) AS total_tokens,
                COUNT(*) AS usage_events
               FROM token_usage_events
              ${whereSqlWithExtra(where, `${spec.column} IS NOT NULL`)}
              GROUP BY ${spec.column}
              ORDER BY total_tokens DESC, key ASC
              LIMIT 25`,
          )
          .bind(...where.values)
          .all<TokenUsageGroupRow>(),
      ),
    )

    return rows
      .flatMap(result => result.results)
      .sort(
        (left, right) =>
          (right.total_tokens ?? 0) - (left.total_tokens ?? 0) ||
          (left.key ?? '').localeCompare(right.key ?? ''),
      )
      .slice(0, 100)
  })

export const makeD1TokenUsageLedger = (
  db: D1Database,
  runtime: TokenUsageLedgerRuntime = systemTokenUsageLedgerRuntime,
): TokenUsageLedgerShape => ({
  ingestEvent: body =>
    Effect.gen(function* () {
      yield* validateSafePayload(body)
      const decoded = yield* decodeIngestBody(body)
      const observedAt = yield* requireTimestamp(
        'observedAt',
        decoded.observedAt,
      )
      const tokenCounts = yield* normalizeCounts(decoded.tokenCounts)
      const safeMetadata = decoded.safeMetadata ?? {}
      const safeMetadataJson = yield* jsonStringifyRecord(
        'safeMetadata',
        safeMetadata,
      )
      const normalized = {
        ...decoded,
        observedAt,
        safeMetadata,
        tokenCounts,
      }
      const existing = yield* findExistingEvent(db, {
        eventId: normalized.eventId,
        idempotencyKey: normalized.idempotencyKey,
      })

      if (existing !== undefined) {
        return {
          event: existing,
          inserted: false,
        }
      }

      const row = storedRowFromInput(normalized, {
        ingestedAt: runtime.nowIso(),
        safeMetadataJson,
        tokenCounts,
      })

      const inserted = yield* insertEventRow(db, row).pipe(
        Effect.matchEffect({
          onFailure: error =>
            isUniqueConstraintError(error.error)
              ? Effect.succeed(false)
              : Effect.fail(error),
          onSuccess: () => Effect.succeed(true),
        }),
      )

      if (!inserted) {
        const racedExisting = yield* findExistingEvent(db, {
          eventId: normalized.eventId,
          idempotencyKey: normalized.idempotencyKey,
        })

        if (racedExisting !== undefined) {
          return {
            event: racedExisting,
            inserted: false,
          }
        }
      }

      const event = yield* rowToRecord(row)

      return {
        event,
        inserted: true,
      }
    }),

  readAggregates: (filters = {}) =>
    Effect.gen(function* () {
      const where = yield* aggregateWhere(filters)
      const totals = yield* d1Effect('tokenUsageEvents.aggregate.totals', () =>
        db
          .prepare(
            `SELECT
                COALESCE(SUM(input_tokens), 0) AS input_tokens,
                COALESCE(SUM(output_tokens), 0) AS output_tokens,
                COALESCE(SUM(reasoning_tokens), 0) AS reasoning_tokens,
                COALESCE(SUM(cache_read_tokens), 0) AS cache_read_tokens,
                COALESCE(SUM(cache_write_5m_tokens), 0) AS cache_write_5m_tokens,
                COALESCE(SUM(cache_write_1h_tokens), 0) AS cache_write_1h_tokens,
                COALESCE(SUM(total_tokens), 0) AS total_tokens,
                COUNT(*) AS usage_events
               FROM token_usage_events
              ${where.sql}`,
          )
          .bind(...where.values)
          .first<TokenUsageCountRow>(),
      )
      const byProviderModel = yield* d1Effect(
        'tokenUsageEvents.aggregate.providerModel',
        () =>
          db
            .prepare(
              `SELECT
                  COALESCE(provider, 'unknown') || ':' || COALESCE(model, 'unknown') AS key,
                  COALESCE(provider, 'unknown') || ' / ' || COALESCE(model, 'unknown') AS label,
                  COALESCE(SUM(input_tokens), 0) AS input_tokens,
                  COALESCE(SUM(output_tokens), 0) AS output_tokens,
                  COALESCE(SUM(reasoning_tokens), 0) AS reasoning_tokens,
                  COALESCE(SUM(cache_read_tokens), 0) AS cache_read_tokens,
                  COALESCE(SUM(cache_write_5m_tokens), 0) AS cache_write_5m_tokens,
                  COALESCE(SUM(cache_write_1h_tokens), 0) AS cache_write_1h_tokens,
                  COALESCE(SUM(total_tokens), 0) AS total_tokens,
                  COUNT(*) AS usage_events
                 FROM token_usage_events
                ${where.sql}
                GROUP BY COALESCE(provider, 'unknown'), COALESCE(model, 'unknown')
                ORDER BY total_tokens DESC, key ASC
                LIMIT 50`,
            )
            .bind(...where.values)
            .all<TokenUsageGroupRow>(),
      )
      const bySourceRoute = yield* d1Effect(
        'tokenUsageEvents.aggregate.sourceRoute',
        () =>
          db
            .prepare(
              `SELECT
                  producer_system || ':' || source_route AS key,
                  producer_system || ' / ' || source_route AS label,
                  COALESCE(SUM(input_tokens), 0) AS input_tokens,
                  COALESCE(SUM(output_tokens), 0) AS output_tokens,
                  COALESCE(SUM(reasoning_tokens), 0) AS reasoning_tokens,
                  COALESCE(SUM(cache_read_tokens), 0) AS cache_read_tokens,
                  COALESCE(SUM(cache_write_5m_tokens), 0) AS cache_write_5m_tokens,
                  COALESCE(SUM(cache_write_1h_tokens), 0) AS cache_write_1h_tokens,
                  COALESCE(SUM(total_tokens), 0) AS total_tokens,
                  COUNT(*) AS usage_events
                 FROM token_usage_events
                ${where.sql}
                GROUP BY producer_system, source_route
                ORDER BY total_tokens DESC, key ASC
                LIMIT 50`,
            )
            .bind(...where.values)
            .all<TokenUsageGroupRow>(),
      )
      const byActor = yield* d1Effect('tokenUsageEvents.aggregate.actor', () =>
        db
          .prepare(
            `SELECT
                actor_user_id,
                actor_team_id,
                account_ref,
                COALESCE(SUM(input_tokens), 0) AS input_tokens,
                COALESCE(SUM(output_tokens), 0) AS output_tokens,
                COALESCE(SUM(reasoning_tokens), 0) AS reasoning_tokens,
                COALESCE(SUM(cache_read_tokens), 0) AS cache_read_tokens,
                COALESCE(SUM(cache_write_5m_tokens), 0) AS cache_write_5m_tokens,
                COALESCE(SUM(cache_write_1h_tokens), 0) AS cache_write_1h_tokens,
                COALESCE(SUM(total_tokens), 0) AS total_tokens,
                COUNT(*) AS usage_events
               FROM token_usage_events
              ${where.sql}
              GROUP BY actor_user_id, actor_team_id, account_ref
              ORDER BY total_tokens DESC
              LIMIT 100`,
          )
          .bind(...where.values)
          .all<TokenUsageActorGroupRow>(),
      )
      const byUsageTruth = yield* d1Effect(
        'tokenUsageEvents.aggregate.usageTruth',
        () =>
          db
            .prepare(
              `SELECT
                  usage_truth AS key,
                  usage_truth AS label,
                  COALESCE(SUM(input_tokens), 0) AS input_tokens,
                  COALESCE(SUM(output_tokens), 0) AS output_tokens,
                  COALESCE(SUM(reasoning_tokens), 0) AS reasoning_tokens,
                  COALESCE(SUM(cache_read_tokens), 0) AS cache_read_tokens,
                  COALESCE(SUM(cache_write_5m_tokens), 0) AS cache_write_5m_tokens,
                  COALESCE(SUM(cache_write_1h_tokens), 0) AS cache_write_1h_tokens,
                  COALESCE(SUM(total_tokens), 0) AS total_tokens,
                  COUNT(*) AS usage_events
                 FROM token_usage_events
                ${where.sql}
                GROUP BY usage_truth
                ORDER BY total_tokens DESC, key ASC`,
            )
            .bind(...where.values)
            .all<TokenUsageGroupRow>(),
      )
      const bySourceRef = yield* aggregateSourceRefRows(db, where)
      const recentRows = yield* d1Effect('tokenUsageEvents.recent', () =>
        db
          .prepare(
            `SELECT *
               FROM token_usage_events
              ${where.sql}
              ORDER BY observed_at DESC, ingested_at DESC
              LIMIT 50`,
          )
          .bind(...where.values)
          .all<TokenUsageEventRow>(),
      )
      const recentEvents = yield* Effect.forEach(
        recentRows.results,
        rowToRecord,
      )

      return yield* decodeAggregateResponse({
        schemaVersion: 'openagents.token_usage_aggregate.v1',
        byActor: byActor.results.map(aggregateActorRow),
        byProviderModel: byProviderModel.results.map(aggregateGroupRow),
        bySourceRoute: bySourceRoute.results.map(aggregateGroupRow),
        bySourceRef: bySourceRef.map(aggregateGroupRow),
        byUsageTruth: byUsageTruth.results.map(aggregateGroupRow),
        filters: where.filters,
        generatedAt: runtime.nowIso(),
        recentEvents,
        totals: countsFromRow(totals),
        usageEvents: totals?.usage_events ?? 0,
      })
    }),

  // OWNER-GATED inference cost / provider-lane analytics (#6232). Aggregate-only
  // token + cost rollups over `token_usage_events` grouped by provider, by
  // model, by source-route/producer-system, and by UTC day, plus window-wide
  // totals. `costUsd` sums the stored `cost_amount`; `costCoverage` reports the
  // fraction of rows that carry a stored cost so a pre-cost-recording gap is
  // explicit rather than silently understated. INTERNAL: provider ids + cost are
  // not public — the route serves this behind the admin/owner gate only.
  readInferenceAnalytics: (filters = {}) =>
    Effect.gen(function* () {
      const window = yield* normalizeLeaderboardWindow(filters.window ?? '7d')
      const nowIso = yield* requireTimestamp(
        'now',
        filters.now ?? runtime.nowIso(),
      )
      const since = leaderboardWindowSince(window, nowIso, runtime)
      const whereSql = since === undefined ? '' : 'WHERE observed_at >= ?'
      const bind: ReadonlyArray<string> = since === undefined ? [] : [since]

      const groupQuery = (
        operation: string,
        keyExpr: string,
        labelExpr: string,
        groupExpr: string,
      ): Effect.Effect<
        ReadonlyArray<{
          key: string | null
          label: string | null
          input_tokens: number | null
          output_tokens: number | null
          total_tokens: number | null
          usage_events: number | null
          cost_usd: number | null
        }>,
        TokenUsageLedgerStorageError
      > =>
        d1Effect(operation, () =>
          db
            .prepare(
              `SELECT
                  ${keyExpr} AS key,
                  ${labelExpr} AS label,
                  COALESCE(SUM(input_tokens), 0) AS input_tokens,
                  COALESCE(SUM(output_tokens), 0) AS output_tokens,
                  COALESCE(SUM(input_tokens), 0) + COALESCE(SUM(output_tokens), 0)
                    AS total_tokens,
                  COUNT(*) AS usage_events,
                  COALESCE(SUM(cost_amount), 0) AS cost_usd
                 FROM token_usage_events
                ${whereSql}
                GROUP BY ${groupExpr}
                ORDER BY total_tokens DESC, key ASC
                LIMIT 100`,
            )
            .bind(...bind)
            .all<{
              key: string | null
              label: string | null
              input_tokens: number | null
              output_tokens: number | null
              total_tokens: number | null
              usage_events: number | null
              cost_usd: number | null
            }>(),
        ).pipe(Effect.map(result => result.results))

      const analyticsRow = (row: {
        key: string | null
        label: string | null
        input_tokens: number | null
        output_tokens: number | null
        total_tokens: number | null
        usage_events: number | null
        cost_usd: number | null
      }) => ({
        costUsd: roundCostUsd(row.cost_usd ?? 0),
        inputTokens: Math.max(0, Math.trunc(row.input_tokens ?? 0)),
        key: row.key ?? 'unknown',
        label: row.label ?? 'unknown',
        outputTokens: Math.max(0, Math.trunc(row.output_tokens ?? 0)),
        totalTokens: Math.max(0, Math.trunc(row.total_tokens ?? 0)),
        usageEvents: Math.max(0, Math.trunc(row.usage_events ?? 0)),
      })

      const byProvider = yield* groupQuery(
        'inferenceAnalytics.byProvider',
        `COALESCE(provider, 'unknown')`,
        `COALESCE(provider, 'unknown')`,
        `COALESCE(provider, 'unknown')`,
      )
      const byModel = yield* groupQuery(
        'inferenceAnalytics.byModel',
        `COALESCE(model, 'unknown')`,
        `COALESCE(model, 'unknown')`,
        `COALESCE(model, 'unknown')`,
      )
      const byRoute = yield* groupQuery(
        'inferenceAnalytics.byRoute',
        `producer_system || ':' || source_route`,
        `producer_system || ' / ' || source_route`,
        `producer_system, source_route`,
      )
      const byDemandKind = yield* groupQuery(
        'inferenceAnalytics.byDemandKind',
        `COALESCE(NULLIF(demand_kind, ''), 'unlabeled')`,
        `COALESCE(NULLIF(demand_kind, ''), 'unlabeled')`,
        `COALESCE(NULLIF(demand_kind, ''), 'unlabeled')`,
      )
      const byDemandSource = yield* groupQuery(
        'inferenceAnalytics.byDemandSource',
        `COALESCE(NULLIF(demand_kind, ''), 'unlabeled') || ':' || COALESCE(NULLIF(demand_source, ''), 'unknown')`,
        `COALESCE(NULLIF(demand_kind, ''), 'unlabeled') || ' / ' || COALESCE(NULLIF(demand_source, ''), 'unknown')`,
        `COALESCE(NULLIF(demand_kind, ''), 'unlabeled'), COALESCE(NULLIF(demand_source, ''), 'unknown')`,
      )
      const byDemandClient = yield* groupQuery(
        'inferenceAnalytics.byDemandClient',
        `COALESCE(NULLIF(demand_kind, ''), 'unlabeled') || ':' || COALESCE(NULLIF(demand_client, ''), 'unknown')`,
        `COALESCE(NULLIF(demand_kind, ''), 'unlabeled') || ' / ' || COALESCE(NULLIF(demand_client, ''), 'unknown')`,
        `COALESCE(NULLIF(demand_kind, ''), 'unlabeled'), COALESCE(NULLIF(demand_client, ''), 'unknown')`,
      )
      const byDayRows = yield* groupQuery(
        'inferenceAnalytics.byDay',
        `date(observed_at)`,
        `date(observed_at)`,
        `date(observed_at)`,
      )
      const byDemandClientDayRows = yield* d1Effect(
        'inferenceAnalytics.byDemandClientDay',
        () =>
          db
            .prepare(
              `SELECT
                  date(observed_at) AS day,
                  COALESCE(NULLIF(demand_kind, ''), 'unlabeled') || ':' ||
                    COALESCE(NULLIF(demand_client, ''), 'unknown') AS key,
                  COALESCE(NULLIF(demand_kind, ''), 'unlabeled') || ' / ' ||
                    COALESCE(NULLIF(demand_client, ''), 'unknown') AS label,
                  COALESCE(SUM(input_tokens), 0) AS input_tokens,
                  COALESCE(SUM(output_tokens), 0) AS output_tokens,
                  COALESCE(SUM(input_tokens), 0) + COALESCE(SUM(output_tokens), 0)
                    AS total_tokens,
                  COUNT(*) AS usage_events,
                  COALESCE(SUM(cost_amount), 0) AS cost_usd
                 FROM token_usage_events
                ${whereSql}
                GROUP BY
                  date(observed_at),
                  COALESCE(NULLIF(demand_kind, ''), 'unlabeled'),
                  COALESCE(NULLIF(demand_client, ''), 'unknown')
                ORDER BY day ASC, total_tokens DESC, key ASC
                LIMIT 500`,
            )
            .bind(...bind)
            .all<{
              day: string | null
              key: string | null
              label: string | null
              input_tokens: number | null
              output_tokens: number | null
              total_tokens: number | null
              usage_events: number | null
              cost_usd: number | null
            }>(),
      ).pipe(Effect.map(result => result.results))
      const totalsRow = yield* d1Effect('inferenceAnalytics.totals', () =>
        db
          .prepare(
            `SELECT
                  COALESCE(SUM(input_tokens), 0) AS input_tokens,
                  COALESCE(SUM(output_tokens), 0) AS output_tokens,
                  COALESCE(SUM(input_tokens), 0) + COALESCE(SUM(output_tokens), 0)
                    AS total_tokens,
                  COUNT(*) AS usage_events,
                  COALESCE(SUM(cost_amount), 0) AS cost_usd,
                  COALESCE(SUM(CASE WHEN cost_amount IS NOT NULL THEN 1 ELSE 0 END), 0)
                    AS cost_rows
                 FROM token_usage_events
                ${whereSql}`,
          )
          .bind(...bind)
          .first<{
            input_tokens: number | null
            output_tokens: number | null
            total_tokens: number | null
            usage_events: number | null
            cost_usd: number | null
            cost_rows: number | null
          }>(),
      )

      const usageEvents = Math.max(0, Math.trunc(totalsRow?.usage_events ?? 0))
      const costRows = Math.max(0, Math.trunc(totalsRow?.cost_rows ?? 0))

      return yield* decodeInferenceAnalyticsResponse({
        schemaVersion: 'openagents.inference_analytics.v1',
        byDay: byDayRows
          .filter(
            (row): row is typeof row & { key: string } =>
              typeof row.key === 'string' && row.key !== '',
          )
          .map(row => ({
            costUsd: roundCostUsd(row.cost_usd ?? 0),
            day: row.key,
            inputTokens: Math.max(0, Math.trunc(row.input_tokens ?? 0)),
            outputTokens: Math.max(0, Math.trunc(row.output_tokens ?? 0)),
            totalTokens: Math.max(0, Math.trunc(row.total_tokens ?? 0)),
            usageEvents: Math.max(0, Math.trunc(row.usage_events ?? 0)),
          }))
          .sort((left, right) => left.day.localeCompare(right.day)),
        byDemandClientDay: byDemandClientDayRows
          .filter(
            (row): row is typeof row & { day: string; key: string } =>
              typeof row.day === 'string' &&
              row.day !== '' &&
              typeof row.key === 'string' &&
              row.key !== '',
          )
          .map(row => ({
            costUsd: roundCostUsd(row.cost_usd ?? 0),
            day: row.day,
            inputTokens: Math.max(0, Math.trunc(row.input_tokens ?? 0)),
            key: row.key,
            label: row.label ?? row.key,
            outputTokens: Math.max(0, Math.trunc(row.output_tokens ?? 0)),
            totalTokens: Math.max(0, Math.trunc(row.total_tokens ?? 0)),
            usageEvents: Math.max(0, Math.trunc(row.usage_events ?? 0)),
          })),
        byModel: byModel.map(analyticsRow),
        byProvider: byProvider.map(analyticsRow),
        byRoute: byRoute.map(analyticsRow),
        byDemandKind: byDemandKind.map(analyticsRow),
        byDemandSource: byDemandSource.map(analyticsRow),
        byDemandClient: byDemandClient.map(analyticsRow),
        generatedAt: runtime.nowIso(),
        totals: {
          costCoverage:
            usageEvents === 0
              ? 1
              : Math.round((costRows / usageEvents) * 1_000_000) / 1_000_000,
          costUsd: roundCostUsd(totalsRow?.cost_usd ?? 0),
          inputTokens: Math.max(0, Math.trunc(totalsRow?.input_tokens ?? 0)),
          outputTokens: Math.max(0, Math.trunc(totalsRow?.output_tokens ?? 0)),
          totalTokens: Math.max(0, Math.trunc(totalsRow?.total_tokens ?? 0)),
          usageEvents,
        },
        window,
      })
    }),

  // Public-safe "Khala Tokens Served" aggregate: the running network-wide SUM
  // of input + output tokens across ALL ledger events. No filters, no grouping,
  // no per-actor or provider material — a single non-negative scalar. The route
  // layer wraps it with generatedAt + the staleness contract before serving.
  readPublicTokensServed: () =>
    Effect.gen(function* () {
      const row = yield* d1Effect('tokenUsageEvents.publicTokensServed', () =>
        db
          .prepare(
            `SELECT
                COALESCE(SUM(input_tokens), 0) + COALESCE(SUM(output_tokens), 0)
                  AS tokens_served
               FROM token_usage_events`,
          )
          .first<{ tokens_served: number | null }>(),
      )

      return yield* decodePublicTokensServedAggregate({
        tokensServed: Math.max(0, Math.trunc(row?.tokens_served ?? 0)),
      })
    }),

  // Public-safe "Khala Tokens Served" history: the per-day SUM of input +
  // output tokens over the requested window, ordered ascending by UTC day. Like
  // the scalar above, it is aggregate only — bare day + sum, no per-user,
  // per-actor, or provider columns. Buckets only on date(observed_at) and uses
  // the observed_at index for the window lower bound. Default window 30d,
  // default bucket day.
  readPublicTokensServedHistory: (filters = {}) =>
    Effect.gen(function* () {
      const window = yield* normalizeLeaderboardWindow(filters.window ?? '30d')
      const bucket = yield* normalizeHistoryBucket(filters.bucket)
      const nowIso = yield* requireTimestamp(
        'now',
        filters.now ?? runtime.nowIso(),
      )
      const since = leaderboardWindowSince(window, nowIso, runtime)
      const rows = yield* d1Effect(
        'tokenUsageEvents.publicTokensServedHistory',
        () =>
          db
            .prepare(
              since === undefined
                ? `SELECT
                      date(observed_at) AS day,
                      COALESCE(SUM(input_tokens), 0) + COALESCE(SUM(output_tokens), 0)
                        AS tokens
                     FROM token_usage_events
                    GROUP BY day
                    ORDER BY day ASC`
                : `SELECT
                      date(observed_at) AS day,
                      COALESCE(SUM(input_tokens), 0) + COALESCE(SUM(output_tokens), 0)
                        AS tokens
                     FROM token_usage_events
                    WHERE observed_at >= ?
                    GROUP BY day
                    ORDER BY day ASC`,
            )
            .bind(...(since === undefined ? [] : [since]))
            .all<{ day: string | null; tokens: number | null }>(),
      )

      const series = rows.results
        .filter(
          (row): row is { day: string; tokens: number | null } =>
            typeof row.day === 'string' && row.day !== '',
        )
        .map(row => ({
          day: row.day,
          tokensServed: Math.max(0, Math.trunc(row.tokens ?? 0)),
        }))

      return yield* decodePublicTokensServedHistory({
        bucket,
        series,
        window,
      })
    }),

  readLeaderboardPreference: input =>
    Effect.gen(function* () {
      const row = yield* d1Effect('tokenUsageLeaderboardPreference.read', () =>
        db
          .prepare(
            `SELECT *
               FROM token_usage_leaderboard_preferences
              WHERE subject_kind = ? AND subject_ref = ?
              LIMIT 1`,
          )
          .bind(input.subjectKind, input.subjectRef)
          .first<TokenUsageLeaderboardPreferenceRow>(),
      )

      return yield* preferenceResponseFromRow(
        row ?? defaultPreferenceRow(input, runtime.nowIso()),
      )
    }),

  updateLeaderboardPreference: (input, body) =>
    Effect.gen(function* () {
      const decoded = yield* decodeLeaderboardPreferenceUpdateBody(body)
      const row: TokenUsageLeaderboardPreferenceRow = {
        leaderboard_participation: decoded.leaderboardParticipation,
        leaderboard_visibility: decoded.leaderboardVisibility,
        subject_kind: input.subjectKind,
        subject_ref: input.subjectRef,
        updated_at: runtime.nowIso(),
        updated_by_user_id: input.actorUserId ?? null,
      }

      yield* d1Effect('tokenUsageLeaderboardPreference.upsert', () =>
        db
          .prepare(
            `INSERT INTO token_usage_leaderboard_preferences (
                subject_kind,
                subject_ref,
                leaderboard_participation,
                leaderboard_visibility,
                updated_at,
                updated_by_user_id
              ) VALUES (?, ?, ?, ?, ?, ?)
              ON CONFLICT(subject_kind, subject_ref) DO UPDATE SET
                leaderboard_participation = excluded.leaderboard_participation,
                leaderboard_visibility = excluded.leaderboard_visibility,
                updated_at = excluded.updated_at,
                updated_by_user_id = excluded.updated_by_user_id`,
          )
          .bind(
            row.subject_kind,
            row.subject_ref,
            row.leaderboard_participation,
            row.leaderboard_visibility,
            row.updated_at,
            row.updated_by_user_id,
          )
          .run(),
      )

      return yield* preferenceResponseFromRow(row)
    }),

  readLeaderboards: (filters = {}) =>
    Effect.gen(function* () {
      const where = yield* leaderboardWhere(filters, runtime)
      const totals = yield* d1Effect(
        'tokenUsageLeaderboards.globalTotals',
        () =>
          db
            .prepare(
              `SELECT
                COALESCE(SUM(input_tokens), 0) AS input_tokens,
                COALESCE(SUM(output_tokens), 0) AS output_tokens,
                COALESCE(SUM(reasoning_tokens), 0) AS reasoning_tokens,
                COALESCE(SUM(cache_read_tokens), 0) AS cache_read_tokens,
                COALESCE(SUM(cache_write_5m_tokens), 0) AS cache_write_5m_tokens,
                COALESCE(SUM(cache_write_1h_tokens), 0) AS cache_write_1h_tokens,
                COALESCE(SUM(total_tokens), 0) AS total_tokens,
                COUNT(*) AS usage_events
               FROM token_usage_events
              ${where.sql}`,
            )
            .bind(...where.values)
            .first<TokenUsageCountRow>(),
      )
      const anonymousTotals = yield* d1Effect(
        'tokenUsageLeaderboards.anonymousTotals',
        () =>
          db
            .prepare(
              `SELECT
                  COALESCE(SUM(input_tokens), 0) AS input_tokens,
                  COALESCE(SUM(output_tokens), 0) AS output_tokens,
                  COALESCE(SUM(reasoning_tokens), 0) AS reasoning_tokens,
                  COALESCE(SUM(cache_read_tokens), 0) AS cache_read_tokens,
                  COALESCE(SUM(cache_write_5m_tokens), 0) AS cache_write_5m_tokens,
                  COALESCE(SUM(cache_write_1h_tokens), 0) AS cache_write_1h_tokens,
                  COALESCE(SUM(total_tokens), 0) AS total_tokens,
                  COUNT(*) AS usage_events
                 FROM token_usage_events
                ${whereSqlWithExtra(
                  where,
                  `((actor_user_id IS NULL AND actor_team_id IS NULL AND account_ref IS NULL)
                   OR leaderboard_eligible = 0
                   OR privacy_opt_out = 1)`,
                )}`,
            )
            .bind(...where.values)
            .first<TokenUsageCountRow>(),
      )
      const topUsers = yield* d1Effect('tokenUsageLeaderboards.users', () =>
        db
          .prepare(
            `SELECT
                actor_user_id,
                NULL AS actor_team_id,
                NULL AS account_ref,
                COALESCE(SUM(input_tokens), 0) AS input_tokens,
                COALESCE(SUM(output_tokens), 0) AS output_tokens,
                COALESCE(SUM(reasoning_tokens), 0) AS reasoning_tokens,
                COALESCE(SUM(cache_read_tokens), 0) AS cache_read_tokens,
                COALESCE(SUM(cache_write_5m_tokens), 0) AS cache_write_5m_tokens,
                COALESCE(SUM(cache_write_1h_tokens), 0) AS cache_write_1h_tokens,
                COALESCE(SUM(total_tokens), 0) AS total_tokens,
                COUNT(*) AS usage_events
               FROM token_usage_events
              ${leaderboardEligibleSql(where, 'actor_user_id IS NOT NULL')}
              GROUP BY actor_user_id
              ORDER BY total_tokens DESC, actor_user_id ASC
              LIMIT 100`,
          )
          .bind(...where.values)
          .all<TokenUsageActorGroupRow>(),
      )
      const topTeams = yield* d1Effect('tokenUsageLeaderboards.teams', () =>
        db
          .prepare(
            `SELECT
                NULL AS actor_user_id,
                actor_team_id,
                NULL AS account_ref,
                COALESCE(SUM(input_tokens), 0) AS input_tokens,
                COALESCE(SUM(output_tokens), 0) AS output_tokens,
                COALESCE(SUM(reasoning_tokens), 0) AS reasoning_tokens,
                COALESCE(SUM(cache_read_tokens), 0) AS cache_read_tokens,
                COALESCE(SUM(cache_write_5m_tokens), 0) AS cache_write_5m_tokens,
                COALESCE(SUM(cache_write_1h_tokens), 0) AS cache_write_1h_tokens,
                COALESCE(SUM(total_tokens), 0) AS total_tokens,
                COUNT(*) AS usage_events
               FROM token_usage_events
              ${leaderboardEligibleSql(where, 'actor_team_id IS NOT NULL')}
              GROUP BY actor_team_id
              ORDER BY total_tokens DESC, actor_team_id ASC
              LIMIT 100`,
          )
          .bind(...where.values)
          .all<TokenUsageActorGroupRow>(),
      )
      const topProviderModels = yield* d1Effect(
        'tokenUsageLeaderboards.providerModels',
        () =>
          db
            .prepare(
              `SELECT
                  COALESCE(provider, 'unknown') || ':' || COALESCE(model, 'unknown') AS key,
                  COALESCE(provider, 'unknown') || ' / ' || COALESCE(model, 'unknown') AS label,
                  COALESCE(SUM(input_tokens), 0) AS input_tokens,
                  COALESCE(SUM(output_tokens), 0) AS output_tokens,
                  COALESCE(SUM(reasoning_tokens), 0) AS reasoning_tokens,
                  COALESCE(SUM(cache_read_tokens), 0) AS cache_read_tokens,
                  COALESCE(SUM(cache_write_5m_tokens), 0) AS cache_write_5m_tokens,
                  COALESCE(SUM(cache_write_1h_tokens), 0) AS cache_write_1h_tokens,
                  COALESCE(SUM(total_tokens), 0) AS total_tokens,
                  COUNT(*) AS usage_events
                 FROM token_usage_events
                ${where.sql}
                GROUP BY COALESCE(provider, 'unknown'), COALESCE(model, 'unknown')
                ORDER BY total_tokens DESC, key ASC
                LIMIT 100`,
            )
            .bind(...where.values)
            .all<TokenUsageGroupRow>(),
      )
      const topRuns = yield* d1Effect('tokenUsageLeaderboards.runs', () =>
        db
          .prepare(
            `SELECT
                'run:' || run_ref AS key,
                'run / ' || run_ref AS label,
                COALESCE(SUM(input_tokens), 0) AS input_tokens,
                COALESCE(SUM(output_tokens), 0) AS output_tokens,
                COALESCE(SUM(reasoning_tokens), 0) AS reasoning_tokens,
                COALESCE(SUM(cache_read_tokens), 0) AS cache_read_tokens,
                COALESCE(SUM(cache_write_5m_tokens), 0) AS cache_write_5m_tokens,
                COALESCE(SUM(cache_write_1h_tokens), 0) AS cache_write_1h_tokens,
                COALESCE(SUM(total_tokens), 0) AS total_tokens,
                COUNT(*) AS usage_events
               FROM token_usage_events
              ${leaderboardEligibleSql(where, 'run_ref IS NOT NULL')}
              GROUP BY run_ref
              ORDER BY total_tokens DESC, key ASC
              LIMIT 100`,
          )
          .bind(...where.values)
          .all<TokenUsageGroupRow>(),
      )
      const topProjects = yield* d1Effect(
        'tokenUsageLeaderboards.projects',
        () =>
          db
            .prepare(
              `SELECT
                'repository:' || repository_ref AS key,
                'repository / ' || repository_ref AS label,
                COALESCE(SUM(input_tokens), 0) AS input_tokens,
                COALESCE(SUM(output_tokens), 0) AS output_tokens,
                COALESCE(SUM(reasoning_tokens), 0) AS reasoning_tokens,
                COALESCE(SUM(cache_read_tokens), 0) AS cache_read_tokens,
                COALESCE(SUM(cache_write_5m_tokens), 0) AS cache_write_5m_tokens,
                COALESCE(SUM(cache_write_1h_tokens), 0) AS cache_write_1h_tokens,
                COALESCE(SUM(total_tokens), 0) AS total_tokens,
                COUNT(*) AS usage_events
               FROM token_usage_events
              ${leaderboardEligibleSql(where, 'repository_ref IS NOT NULL')}
              GROUP BY repository_ref
              ORDER BY total_tokens DESC, key ASC
              LIMIT 100`,
            )
            .bind(...where.values)
            .all<TokenUsageGroupRow>(),
      )

      return yield* decodeLeaderboardsResponse({
        schemaVersion: 'openagents.token_usage_leaderboards.v1',
        anonymousTotals: countsFromRow(anonymousTotals),
        filters: where.filters,
        generatedAt: runtime.nowIso(),
        globalTotals: countsFromRow(totals),
        topProviderModels: topProviderModels.results.map(aggregateGroupRow),
        topProjects: topProjects.results.map(aggregateGroupRow),
        topRuns: topRuns.results.map(aggregateGroupRow),
        topTeams: topTeams.results.map(aggregateActorRow),
        topUsers: topUsers.results.map(aggregateActorRow),
      })
    }),
})

export class TokenUsageLedger extends Context.Service<
  TokenUsageLedger,
  TokenUsageLedgerShape
>()('@openagentsinc/TokenUsageLedger') {
  static live = (
    db: D1Database,
    runtime: TokenUsageLedgerRuntime = systemTokenUsageLedgerRuntime,
  ) => Layer.succeed(TokenUsageLedger, makeD1TokenUsageLedger(db, runtime))

  static layer = (
    env: Readonly<{ OPENAGENTS_DB: D1Database }>,
    runtime: TokenUsageLedgerRuntime = systemTokenUsageLedgerRuntime,
  ) => TokenUsageLedger.live(openAgentsDatabase(env), runtime)

  static effectCfLayer = (
    runtime: TokenUsageLedgerRuntime = systemTokenUsageLedgerRuntime,
  ) =>
    Layer.effect(
      TokenUsageLedger,
      Effect.map(OpenAgentsDatabase, db => makeD1TokenUsageLedger(db, runtime)),
    )
}
