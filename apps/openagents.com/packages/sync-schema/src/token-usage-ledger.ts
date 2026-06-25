import { Schema as S } from 'effect'

export const TokenUsageLedgerEventId = S.String.pipe(
  S.brand('TokenUsageLedgerEventId'),
)
export type TokenUsageLedgerEventId = typeof TokenUsageLedgerEventId.Type

export const TokenUsageIdempotencyKey = S.String.pipe(
  S.brand('TokenUsageIdempotencyKey'),
)
export type TokenUsageIdempotencyKey = typeof TokenUsageIdempotencyKey.Type

export const TokenUsageProducerSystem = S.Literals([
  'probe',
  'omega',
  'provider_broker',
  'shc_runner',
  'manual',
  'unknown',
])
export type TokenUsageProducerSystem = typeof TokenUsageProducerSystem.Type

export const TokenUsageSourceRoute = S.Literals([
  'probe_direct_provider',
  'probe_local_model',
  'omega_provider_broker',
  'omega_hosted_gemini',
  'shc_runner_callback',
  'manual',
  'unknown',
])
export type TokenUsageSourceRoute = typeof TokenUsageSourceRoute.Type

export const TokenUsageTruth = S.Literals(['exact', 'estimated', 'unknown'])
export type TokenUsageTruth = typeof TokenUsageTruth.Type

export const TokenUsageLeaderboardWindow = S.Literals([
  'today',
  '7d',
  '30d',
  'all',
])
export type TokenUsageLeaderboardWindow =
  typeof TokenUsageLeaderboardWindow.Type

export const TokenUsageLeaderboardParticipation = S.Literals([
  'eligible',
  'opted_out',
])
export type TokenUsageLeaderboardParticipation =
  typeof TokenUsageLeaderboardParticipation.Type

export const TokenUsageLeaderboardVisibility = S.Literals([
  'internal',
  'private',
])
export type TokenUsageLeaderboardVisibility =
  typeof TokenUsageLeaderboardVisibility.Type

export class TokenUsageCounts extends S.Class<TokenUsageCounts>(
  'TokenUsageCounts',
)({
  inputTokens: S.Int,
  outputTokens: S.Int,
  reasoningTokens: S.Int,
  cacheReadTokens: S.Int,
  cacheWrite5mTokens: S.Int,
  cacheWrite1hTokens: S.Int,
  totalTokens: S.Int,
}) {}

export class TokenUsageActorRefs extends S.Class<TokenUsageActorRefs>(
  'TokenUsageActorRefs',
)({
  accountRef: S.optionalKey(S.String),
  teamId: S.optionalKey(S.String),
  userId: S.optionalKey(S.String),
}) {}

export class TokenUsageSourceRefs extends S.Class<TokenUsageSourceRefs>(
  'TokenUsageSourceRefs',
)({
  anonymizedSourceRef: S.optionalKey(S.String),
  repositoryRef: S.optionalKey(S.String),
  runRef: S.optionalKey(S.String),
  sessionRef: S.optionalKey(S.String),
  taskRef: S.optionalKey(S.String),
}) {}

export class TokenUsageCost extends S.Class<TokenUsageCost>('TokenUsageCost')({
  amount: S.Number,
  currency: S.String,
}) {}

export class TokenUsagePrivacyFlags extends S.Class<TokenUsagePrivacyFlags>(
  'TokenUsagePrivacyFlags',
)({
  leaderboardEligible: S.Boolean,
  privacyOptOut: S.Boolean,
}) {}

export class TokenUsageEventIngestBody extends S.Class<TokenUsageEventIngestBody>(
  'TokenUsageEventIngestBody',
)({
  schemaVersion: S.Literal('openagents.token_usage_event.v1'),
  actor: S.optionalKey(TokenUsageActorRefs),
  backendProfile: S.optionalKey(S.String),
  cost: S.optionalKey(TokenUsageCost),
  eventId: TokenUsageLedgerEventId,
  idempotencyKey: TokenUsageIdempotencyKey,
  model: S.optionalKey(S.String),
  observedAt: S.String,
  privacy: S.optionalKey(TokenUsagePrivacyFlags),
  producerSystem: TokenUsageProducerSystem,
  provider: S.optionalKey(S.String),
  safeMetadata: S.optionalKey(S.Record(S.String, S.Unknown)),
  sourceRefs: S.optionalKey(TokenUsageSourceRefs),
  sourceRoute: TokenUsageSourceRoute,
  tokenCounts: TokenUsageCounts,
  usageTruth: TokenUsageTruth,
}) {}

export class TokenUsageEventRecord extends S.Class<TokenUsageEventRecord>(
  'TokenUsageEventRecord',
)({
  schemaVersion: S.Literal('openagents.token_usage_event.record.v1'),
  actor: TokenUsageActorRefs,
  backendProfile: S.NullOr(S.String),
  cost: S.NullOr(TokenUsageCost),
  eventId: TokenUsageLedgerEventId,
  idempotencyKey: TokenUsageIdempotencyKey,
  ingestedAt: S.String,
  model: S.NullOr(S.String),
  observedAt: S.String,
  privacy: TokenUsagePrivacyFlags,
  producerSystem: TokenUsageProducerSystem,
  provider: S.NullOr(S.String),
  safeMetadata: S.Record(S.String, S.Unknown),
  sourceRefs: TokenUsageSourceRefs,
  sourceRoute: TokenUsageSourceRoute,
  tokenCounts: TokenUsageCounts,
  usageTruth: TokenUsageTruth,
}) {}

export class TokenUsageAggregateFilters extends S.Class<TokenUsageAggregateFilters>(
  'TokenUsageAggregateFilters',
)({
  accountRef: S.optionalKey(S.String),
  actorTeamId: S.optionalKey(S.String),
  actorUserId: S.optionalKey(S.String),
  leaderboardEligible: S.optionalKey(S.Boolean),
  model: S.optionalKey(S.String),
  privacyOptOut: S.optionalKey(S.Boolean),
  producerSystem: S.optionalKey(TokenUsageProducerSystem),
  provider: S.optionalKey(S.String),
  since: S.optionalKey(S.String),
  sourceRoute: S.optionalKey(TokenUsageSourceRoute),
  until: S.optionalKey(S.String),
  usageTruth: S.optionalKey(TokenUsageTruth),
}) {}

export class TokenUsageAggregateRow extends S.Class<TokenUsageAggregateRow>(
  'TokenUsageAggregateRow',
)({
  key: S.String,
  label: S.String,
  tokenCounts: TokenUsageCounts,
  usageEvents: S.Int,
}) {}

export class TokenUsageActorAggregateRow extends S.Class<TokenUsageActorAggregateRow>(
  'TokenUsageActorAggregateRow',
)({
  accountRef: S.NullOr(S.String),
  anonymous: S.Boolean,
  teamId: S.NullOr(S.String),
  tokenCounts: TokenUsageCounts,
  usageEvents: S.Int,
  userId: S.NullOr(S.String),
}) {}

export class TokenUsageAggregateResponse extends S.Class<TokenUsageAggregateResponse>(
  'TokenUsageAggregateResponse',
)({
  schemaVersion: S.Literal('openagents.token_usage_aggregate.v1'),
  byActor: S.Array(TokenUsageActorAggregateRow),
  byProviderModel: S.Array(TokenUsageAggregateRow),
  bySourceRoute: S.Array(TokenUsageAggregateRow),
  bySourceRef: S.Array(TokenUsageAggregateRow),
  byUsageTruth: S.Array(TokenUsageAggregateRow),
  filters: TokenUsageAggregateFilters,
  generatedAt: S.String,
  recentEvents: S.Array(TokenUsageEventRecord),
  totals: TokenUsageCounts,
  usageEvents: S.Int,
}) {}

export class TokenUsageLeaderboardFilters extends S.Class<TokenUsageLeaderboardFilters>(
  'TokenUsageLeaderboardFilters',
)({
  since: S.optionalKey(S.String),
  until: S.optionalKey(S.String),
  window: TokenUsageLeaderboardWindow,
}) {}

export class TokenUsageLeaderboardsResponse extends S.Class<TokenUsageLeaderboardsResponse>(
  'TokenUsageLeaderboardsResponse',
)({
  schemaVersion: S.Literal('openagents.token_usage_leaderboards.v1'),
  anonymousTotals: TokenUsageCounts,
  filters: TokenUsageLeaderboardFilters,
  generatedAt: S.String,
  globalTotals: TokenUsageCounts,
  topProviderModels: S.Array(TokenUsageAggregateRow),
  topProjects: S.Array(TokenUsageAggregateRow),
  topRuns: S.Array(TokenUsageAggregateRow),
  topTeams: S.Array(TokenUsageActorAggregateRow),
  topUsers: S.Array(TokenUsageActorAggregateRow),
}) {}

export class TokenUsageLeaderboardPreference extends S.Class<TokenUsageLeaderboardPreference>(
  'TokenUsageLeaderboardPreference',
)({
  leaderboardParticipation: TokenUsageLeaderboardParticipation,
  leaderboardVisibility: TokenUsageLeaderboardVisibility,
  subjectKind: S.Literals(['account', 'team', 'user']),
  subjectRef: S.String,
  updatedAt: S.String,
  updatedByUserId: S.NullOr(S.String),
}) {}

export class TokenUsageLeaderboardPreferenceResponse extends S.Class<TokenUsageLeaderboardPreferenceResponse>(
  'TokenUsageLeaderboardPreferenceResponse',
)({
  schemaVersion: S.Literal('openagents.token_usage_leaderboard_preference.v1'),
  preference: TokenUsageLeaderboardPreference,
}) {}

export class TokenUsageLeaderboardPreferenceUpdateBody extends S.Class<TokenUsageLeaderboardPreferenceUpdateBody>(
  'TokenUsageLeaderboardPreferenceUpdateBody',
)({
  leaderboardParticipation: TokenUsageLeaderboardParticipation,
  leaderboardVisibility: TokenUsageLeaderboardVisibility,
}) {}

// Public-safe aggregate: the running network-wide total of tokens SERVED
// (input + output) — "Khala Tokens Served" on the homepage. This is an
// aggregate-only projection of the canonical token usage ledger; it carries NO
// per-user, per-team, per-account, provider-payload, or any other private
// material. `tokensServed` is the SUM of input + output tokens across ALL
// ledger events (privacy opt-out events still count toward the global aggregate
// per the ledger invariant — only leaderboard projections exclude them). The
// route layer wraps this scalar with the shared public-projection staleness
// contract (generatedAt + staleness) before serving.
export class PublicKhalaTokensServedAggregate extends S.Class<PublicKhalaTokensServedAggregate>(
  'PublicKhalaTokensServedAggregate',
)({
  // Always a non-negative integer; the producer clamps with Math.max(0, ...).
  tokensServed: S.Int,
}) {}

// The windows the "Khala Tokens Served" history read supports. Reuses the same
// vocabulary as the token-usage leaderboard window helper. Default is 30d.
export const PublicKhalaTokensServedHistoryWindow = S.Literals([
  'today',
  '7d',
  '30d',
  'all',
])
export type PublicKhalaTokensServedHistoryWindow =
  typeof PublicKhalaTokensServedHistoryWindow.Type

// The only supported bucket today is calendar day (UTC). Modeled as a literal
// so a future hourly bucket is an additive, typed change rather than a free
// string.
export const PublicKhalaTokensServedHistoryBucket = S.Literals(['day'])
export type PublicKhalaTokensServedHistoryBucket =
  typeof PublicKhalaTokensServedHistoryBucket.Type

// One point in the public-safe tokens-served history series: a calendar day
// (UTC, 'YYYY-MM-DD') and the SUM of input + output tokens served on that day.
// Aggregate only — bare day + sum, no per-user/actor/provider material.
export class PublicKhalaTokensServedHistoryPoint extends S.Class<PublicKhalaTokensServedHistoryPoint>(
  'PublicKhalaTokensServedHistoryPoint',
)({
  // Calendar day in UTC, ISO 'YYYY-MM-DD'.
  day: S.String,
  // Non-negative integer SUM of input + output tokens served that day.
  tokensServed: S.Int,
}) {}

// The public-safe tokens-served history projection: the requested window and
// bucket plus the per-day series, ordered ascending by day. The route layer
// wraps this with generatedAt + the shared public-projection staleness
// contract before serving.
export class PublicKhalaTokensServedHistory extends S.Class<PublicKhalaTokensServedHistory>(
  'PublicKhalaTokensServedHistory',
)({
  window: PublicKhalaTokensServedHistoryWindow,
  bucket: PublicKhalaTokensServedHistoryBucket,
  series: S.Array(PublicKhalaTokensServedHistoryPoint),
}) {}
