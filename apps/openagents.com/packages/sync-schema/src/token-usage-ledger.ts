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
  'pylon',
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
  'pylon_codex_direct_local',
  'shc_runner_callback',
  'manual',
  'unknown',
])
export type TokenUsageSourceRoute = typeof TokenUsageSourceRoute.Type

export const TokenUsageTruth = S.Literals(['exact', 'estimated', 'unknown'])
export type TokenUsageTruth = typeof TokenUsageTruth.Type

export const TokenUsageDemandKind = S.Literals([
  'external',
  'internal',
  'internal_stress',
  'own_capacity',
  'unlabeled',
])
export type TokenUsageDemandKind = typeof TokenUsageDemandKind.Type

export const TokenUsageDemandChannel = S.Literals([
  'khala_api',
  'direct_local',
])
export type TokenUsageDemandChannel = typeof TokenUsageDemandChannel.Type

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

export class TokenUsageDemandAttribution extends S.Class<TokenUsageDemandAttribution>(
  'TokenUsageDemandAttribution',
)({
  demandChannel: S.optionalKey(TokenUsageDemandChannel),
  demandKind: TokenUsageDemandKind,
  demandSource: S.optionalKey(S.String),
  demandClient: S.optionalKey(S.String),
}) {}

export class TokenUsageEventIngestBody extends S.Class<TokenUsageEventIngestBody>(
  'TokenUsageEventIngestBody',
)({
  schemaVersion: S.Literal('openagents.token_usage_event.v1'),
  actor: S.optionalKey(TokenUsageActorRefs),
  backendProfile: S.optionalKey(S.String),
  cost: S.optionalKey(TokenUsageCost),
  demand: S.optionalKey(TokenUsageDemandAttribution),
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
  demand: TokenUsageDemandAttribution,
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
// (input + output) — the product-wide "Tokens Served" counter. This is an
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

// The windows the public "Tokens Served" history read supports. Reuses the same
// vocabulary as the token-usage leaderboard window helper. Default is 30d.
export const PublicKhalaTokensServedHistoryWindow = S.Literals([
  'today',
  '7d',
  '30d',
  'all',
])
export type PublicKhalaTokensServedHistoryWindow =
  typeof PublicKhalaTokensServedHistoryWindow.Type

// The only supported bucket today is calendar day. Modeled as a literal so a
// future hourly bucket is an additive, typed change rather than a free string.
export const PublicKhalaTokensServedHistoryBucket = S.Literals(['day'])
export type PublicKhalaTokensServedHistoryBucket =
  typeof PublicKhalaTokensServedHistoryBucket.Type

// One point in the public-safe tokens-served history series: a calendar day
// ('YYYY-MM-DD' in the response timezone) and the SUM of input + output tokens
// served on that day. Aggregate only — bare day + sum, no per-user/actor/provider
// material.
export class PublicKhalaTokensServedHistoryPoint extends S.Class<PublicKhalaTokensServedHistoryPoint>(
  'PublicKhalaTokensServedHistoryPoint',
)({
  // Calendar day in the response timezone, ISO 'YYYY-MM-DD'.
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
  timezone: S.String,
  series: S.Array(PublicKhalaTokensServedHistoryPoint),
}) {}

export const PublicKhalaTokensServedModelFamily = S.Literals([
  'glm',
  'fireworks_deepseek',
  'pylon_codex',
  'codex_direct',
  'pylon_claude',
  'gpt_oss',
  'gemini',
  'other',
])
export type PublicKhalaTokensServedModelFamily =
  typeof PublicKhalaTokensServedModelFamily.Type

export class PublicKhalaTokensServedModelMixGroup extends S.Class<PublicKhalaTokensServedModelMixGroup>(
  'PublicKhalaTokensServedModelMixGroup',
)({
  family: PublicKhalaTokensServedModelFamily,
  label: S.String,
  tokens: S.Int,
  reqs: S.Int,
  pct: S.Number,
}) {}

// Public-safe model/provider mix for /stats. The ledger collapses raw provider
// and model ids into the bounded owner-requested groups before returning, so
// the public payload remains aggregate-only: no raw provider ids, raw model ids,
// account refs, actor refs, or per-event rows.
export class PublicKhalaTokensServedModelMix extends S.Class<PublicKhalaTokensServedModelMix>(
  'PublicKhalaTokensServedModelMix',
)({
  window: PublicKhalaTokensServedHistoryWindow,
  totalTokens: S.Int,
  groups: S.Array(PublicKhalaTokensServedModelMixGroup),
}) {}

export class PublicKhalaTokensServedDemandMixGroup extends S.Class<PublicKhalaTokensServedDemandMixGroup>(
  'PublicKhalaTokensServedDemandMixGroup',
)({
  kind: TokenUsageDemandKind,
  source: S.String,
  client: S.String,
  tokens: S.Int,
  reqs: S.Int,
  pct: S.Number,
}) {}

// Public-safe demand/adoption mix for /stats and GTM closeability checks. This
// intentionally exposes only aggregate rows over the bounded demand taxonomy
// plus sanitized source/client labels. It never includes account refs, actor
// refs, provider ids, model ids, prompts, completions, traces, or event ids.
export class PublicKhalaTokensServedDemandMix extends S.Class<PublicKhalaTokensServedDemandMix>(
  'PublicKhalaTokensServedDemandMix',
)({
  window: PublicKhalaTokensServedHistoryWindow,
  totalTokens: S.Int,
  groups: S.Array(PublicKhalaTokensServedDemandMixGroup),
}) {}

export class PublicKhalaTokensServedChannelMixGroup extends S.Class<PublicKhalaTokensServedChannelMixGroup>(
  'PublicKhalaTokensServedChannelMixGroup',
)({
  channel: TokenUsageDemandChannel,
  label: S.String,
  tokens: S.Int,
  reqs: S.Int,
  pct: S.Number,
}) {}

// Public-safe channel mix for /stats. This splits the product-wide total into
// Khala API/delegation traffic and explicit opt-in direct-local usage. It is
// aggregate only and never carries account refs, actor refs, prompts, paths, or
// provider payloads.
export class PublicKhalaTokensServedChannelMix extends S.Class<PublicKhalaTokensServedChannelMix>(
  'PublicKhalaTokensServedChannelMix',
)({
  window: PublicKhalaTokensServedHistoryWindow,
  totalTokens: S.Int,
  groups: S.Array(PublicKhalaTokensServedChannelMixGroup),
}) {}

// ----------------------------------------------------------------------------
// Inference cost / provider-lane analytics (issue #6232) — OWNER-GATED
// ----------------------------------------------------------------------------
//
// An aggregate-only read over `token_usage_events` for the owner to answer
// "which providers/models/lanes has Khala inference gone to, how many tokens,
// and what did it cost us." It is INTERNAL cost data (provider ids + cost), NOT
// public — served behind the admin/owner gate, never on a public route. Every
// row is aggregate (SUM/COUNT) with no per-user / per-account / prompt material.
// `costUsd` is the SUM of our marginal cost (token_usage_events.cost_amount);
// rows written before cost was recorded carry NULL cost and are reported as 0
// stored cost with a `costCoverage` ratio so the gap is explicit.

export const InferenceAnalyticsWindow = S.Literals([
  'today',
  '7d',
  '30d',
  'all',
])
export type InferenceAnalyticsWindow = typeof InferenceAnalyticsWindow.Type

// One aggregate analytics row: a grouping key + label, the summed token counts,
// the request count, and our summed marginal cost in USD for the group.
export class InferenceAnalyticsRow extends S.Class<InferenceAnalyticsRow>(
  'InferenceAnalyticsRow',
)({
  key: S.String,
  label: S.String,
  inputTokens: S.Int,
  outputTokens: S.Int,
  totalTokens: S.Int,
  usageEvents: S.Int,
  // SUM of our marginal cost (USD) over the group. 0 when every row in the group
  // predates cost recording (see costCoverage on the response).
  costUsd: S.Number,
  // Fraction (0..1) of rows in this group that carry a stored cost_amount.
  // Missing costs are not silently treated as free.
  costCoverage: S.Number,
}) {}

export const InferenceAnalyticsMeasuredNumber = S.Union([
  S.Number,
  S.Literal('not_measured'),
])
export type InferenceAnalyticsMeasuredNumber =
  typeof InferenceAnalyticsMeasuredNumber.Type

export class InferenceAnalyticsLatencySummary extends S.Class<InferenceAnalyticsLatencySummary>(
  'InferenceAnalyticsLatencySummary',
)({
  sampleCount: S.Int,
  averageMs: InferenceAnalyticsMeasuredNumber,
  p50Ms: InferenceAnalyticsMeasuredNumber,
  p90Ms: InferenceAnalyticsMeasuredNumber,
  p99Ms: InferenceAnalyticsMeasuredNumber,
}) {}

export class InferenceAnalyticsThroughputSummary extends S.Class<InferenceAnalyticsThroughputSummary>(
  'InferenceAnalyticsThroughputSummary',
)({
  sampleCount: S.Int,
  averageTokensPerSecond: InferenceAnalyticsMeasuredNumber,
  p50TokensPerSecond: InferenceAnalyticsMeasuredNumber,
  p90TokensPerSecond: InferenceAnalyticsMeasuredNumber,
  p99TokensPerSecond: InferenceAnalyticsMeasuredNumber,
}) {}

export class InferenceAnalyticsOperationalSummary extends S.Class<InferenceAnalyticsOperationalSummary>(
  'InferenceAnalyticsOperationalSummary',
)({
  busyEvents: S.Int,
  fallbackEvents: S.Int,
  fallbackRate: S.Number,
  saturationEvents: S.Int,
  queueWaitMs: InferenceAnalyticsLatencySummary,
  batchWaitMs: InferenceAnalyticsLatencySummary,
  ttftMs: InferenceAnalyticsLatencySummary,
  totalWallClockMs: InferenceAnalyticsLatencySummary,
  perceivedTokensPerSecond: InferenceAnalyticsThroughputSummary,
}) {}

export class InferenceAnalyticsGlmReplicaSummary extends S.Class<InferenceAnalyticsGlmReplicaSummary>(
  'InferenceAnalyticsGlmReplicaSummary',
)({
  key: S.String,
  label: S.String,
  totalTokens: S.Int,
  usageEvents: S.Int,
  costUsd: S.Number,
  costCoverage: S.Number,
  capacityClass: S.String,
  warmState: S.String,
  latestInflight: InferenceAnalyticsMeasuredNumber,
  maxInflight: InferenceAnalyticsMeasuredNumber,
  latestQueueDepth: InferenceAnalyticsMeasuredNumber,
  busyEvents: S.Int,
  fallbackEvents: S.Int,
  saturationEvents: S.Int,
  queueWaitMs: InferenceAnalyticsLatencySummary,
  ttftMs: InferenceAnalyticsLatencySummary,
  totalWallClockMs: InferenceAnalyticsLatencySummary,
  perceivedTokensPerSecond: InferenceAnalyticsThroughputSummary,
  keepWarmStatus: S.String,
  watchdogStatus: S.String,
  uptimeHours: InferenceAnalyticsMeasuredNumber,
  idleHours: InferenceAnalyticsMeasuredNumber,
  effectiveCostPerServedTokenUsd: InferenceAnalyticsMeasuredNumber,
}) {}

export const InferenceAnalyticsHourlyCostCoverage = S.Literals([
  'measured',
  'not_measured',
  'partial',
])
export type InferenceAnalyticsHourlyCostCoverage =
  typeof InferenceAnalyticsHourlyCostCoverage.Type

export const OwnedInferenceProvisioningModel = S.Literals([
  'spot',
  'on_demand',
  'dws_flex',
])
export type OwnedInferenceProvisioningModel =
  typeof OwnedInferenceProvisioningModel.Type

export class OwnedInferenceCostProfile extends S.Class<OwnedInferenceCostProfile>(
  'OwnedInferenceCostProfile',
)({
  profileRef: S.String,
  supplyLane: S.String,
  modelRef: S.String,
  machineShape: S.String,
  gpuCount: S.Int,
  provisioningModel: OwnedInferenceProvisioningModel,
  monthlyComputeUsd: S.Number,
  hourlyComputeUsd: S.Number,
  monthlyStorageOverheadUsd: InferenceAnalyticsMeasuredNumber,
  hourlyStorageOverheadUsd: InferenceAnalyticsMeasuredNumber,
  sourceRef: S.String,
  evidenceRefs: S.Array(S.String),
}) {}

export class InferenceAnalyticsOwnedCostScenario extends S.Class<InferenceAnalyticsOwnedCostScenario>(
  'InferenceAnalyticsOwnedCostScenario',
)({
  profileRef: S.String,
  machineShape: S.String,
  gpuCount: S.Int,
  provisioningModel: OwnedInferenceProvisioningModel,
  replicaCount: S.Int,
  uptimeHours: S.Number,
  activeServingHours: S.Number,
  idleHours: S.Number,
  hourlyBurnUsd: S.Number,
  monthlyComputeUsd: S.Number,
  windowBurnUsd: S.Number,
  idleBurnUsd: S.Number,
  activeDemandBurnUsd: S.Number,
  internalDemandBurnUsd: S.Number,
  externalDemandBurnUsd: S.Number,
  unlabeledDemandBurnUsd: S.Number,
  benchmarkReservedBurnUsd: InferenceAnalyticsMeasuredNumber,
  keepWarmBurnUsd: InferenceAnalyticsMeasuredNumber,
  storageOverheadUsd: InferenceAnalyticsMeasuredNumber,
  acceptedOutcomes: InferenceAnalyticsMeasuredNumber,
  costPerAcceptedOutcomeUsd: InferenceAnalyticsMeasuredNumber,
  effectiveCostPerServedTokenUsd: InferenceAnalyticsMeasuredNumber,
  sourceRef: S.String,
}) {}

export class InferenceAnalyticsOwnedDemandCostRow extends S.Class<InferenceAnalyticsOwnedDemandCostRow>(
  'InferenceAnalyticsOwnedDemandCostRow',
)({
  key: S.String,
  label: S.String,
  demandKind: TokenUsageDemandKind,
  demandSource: S.String,
  demandClient: S.String,
  totalTokens: S.Int,
  usageEvents: S.Int,
  activeServingHours: S.Number,
  activeDemandBurnUsd: S.Number,
  costPerServedTokenUsd: InferenceAnalyticsMeasuredNumber,
}) {}

export class InferenceAnalyticsOwnedHourlySummary extends S.Class<InferenceAnalyticsOwnedHourlySummary>(
  'InferenceAnalyticsOwnedHourlySummary',
)({
  costCoverage: InferenceAnalyticsHourlyCostCoverage,
  hourlyBurnUsd: InferenceAnalyticsMeasuredNumber,
  monthlyBurnUsd: InferenceAnalyticsMeasuredNumber,
  windowBurnUsd: InferenceAnalyticsMeasuredNumber,
  activeDemandBurnUsd: InferenceAnalyticsMeasuredNumber,
  idleBurnUsd: InferenceAnalyticsMeasuredNumber,
  uptimeHours: InferenceAnalyticsMeasuredNumber,
  activeServingHours: InferenceAnalyticsMeasuredNumber,
  idleHours: InferenceAnalyticsMeasuredNumber,
  internalDemandBurnUsd: InferenceAnalyticsMeasuredNumber,
  externalDemandBurnUsd: InferenceAnalyticsMeasuredNumber,
  unlabeledDemandBurnUsd: InferenceAnalyticsMeasuredNumber,
  benchmarkReservedBurnUsd: InferenceAnalyticsMeasuredNumber,
  keepWarmBurnUsd: InferenceAnalyticsMeasuredNumber,
  storageOverheadUsd: InferenceAnalyticsMeasuredNumber,
  acceptedOutcomes: InferenceAnalyticsMeasuredNumber,
  costPerAcceptedOutcomeUsd: InferenceAnalyticsMeasuredNumber,
  effectiveCostPerServedTokenUsd: InferenceAnalyticsMeasuredNumber,
  profiles: S.Array(OwnedInferenceCostProfile),
  scenarios: S.Array(InferenceAnalyticsOwnedCostScenario),
  demand: S.Array(InferenceAnalyticsOwnedDemandCostRow),
  blockerRefs: S.Array(S.String),
}) {}

// One per-day analytics point (UTC calendar day) with summed tokens, request
// count, and summed marginal cost.
export class InferenceAnalyticsDayPoint extends S.Class<InferenceAnalyticsDayPoint>(
  'InferenceAnalyticsDayPoint',
)({
  day: S.String,
  inputTokens: S.Int,
  outputTokens: S.Int,
  totalTokens: S.Int,
  usageEvents: S.Int,
  costUsd: S.Number,
}) {}

// One per-tool/per-day analytics point. The key/label mirrors byDemandClient so
// owner views can render tool adoption over time without joining separate
// response arrays client-side.
export class InferenceAnalyticsDemandClientDayPoint extends S.Class<InferenceAnalyticsDemandClientDayPoint>(
  'InferenceAnalyticsDemandClientDayPoint',
)({
  day: S.String,
  key: S.String,
  label: S.String,
  inputTokens: S.Int,
  outputTokens: S.Int,
  totalTokens: S.Int,
  usageEvents: S.Int,
  costUsd: S.Number,
}) {}

// Window-wide totals across the whole result.
export class InferenceAnalyticsTotals extends S.Class<InferenceAnalyticsTotals>(
  'InferenceAnalyticsTotals',
)({
  inputTokens: S.Int,
  outputTokens: S.Int,
  totalTokens: S.Int,
  usageEvents: S.Int,
  // SUM of stored marginal cost (USD) across the window.
  costUsd: S.Number,
  // Fraction (0..1) of rows in the window that carry a stored cost_amount. < 1
  // means some rows predate cost recording, so costUsd understates true cost.
  costCoverage: S.Number,
}) {}

// The owner-gated inference analytics response: token + cost aggregates grouped
// by provider, by model, by source-route/producer-system, by demand attribution,
// and by day, plus window-wide totals. Demand attribution is owner-gated so
// internal dogfood tokens remain real served tokens without being presented as
// external market demand on public surfaces.
export class InferenceAnalyticsResponse extends S.Class<InferenceAnalyticsResponse>(
  'InferenceAnalyticsResponse',
)({
  schemaVersion: S.Literal('openagents.inference_analytics.v1'),
  window: InferenceAnalyticsWindow,
  generatedAt: S.String,
  byProvider: S.Array(InferenceAnalyticsRow),
  bySupplyLane: S.Array(InferenceAnalyticsRow),
  byAdapter: S.Array(InferenceAnalyticsRow),
  byModel: S.Array(InferenceAnalyticsRow),
  byRoute: S.Array(InferenceAnalyticsRow),
  byGlmReplica: S.Array(InferenceAnalyticsRow),
  byRequestClass: S.Array(InferenceAnalyticsRow),
  byDemandKind: S.Array(InferenceAnalyticsRow),
  byDemandSource: S.Array(InferenceAnalyticsRow),
  byDemandClient: S.Array(InferenceAnalyticsRow),
  byDay: S.Array(InferenceAnalyticsDayPoint),
  byDemandClientDay: S.Array(InferenceAnalyticsDemandClientDayPoint),
  operational: InferenceAnalyticsOperationalSummary,
  glmReplicas: S.Array(InferenceAnalyticsGlmReplicaSummary),
  ownedHourly: InferenceAnalyticsOwnedHourlySummary,
  totals: InferenceAnalyticsTotals,
}) {}
