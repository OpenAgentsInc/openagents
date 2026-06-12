import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import {
  exaEnrichmentOperationsPolicyFromConfig,
  retryExaEffect,
} from './adjutant-enrichment-operations'
import { sha256Hex } from './agent-registration'
import { type ExaConfig, type OpenAgentsWorkerConfigShape } from './config'
import {
  type ExaClientShape,
  ExaConfigurationDisabled,
  type ExaError,
  ExaProviderFetchError,
  ExaProviderHttpError,
  ExaProviderInvalidJson,
  ExaProviderSchemaError,
  ExaProviderTimeout,
  type ExaSearchCategory,
  type ExaSearchResult,
} from './exa'
import { parseJsonRecord, parseJsonWithSchema } from './json-boundary'
import {
  compactRandomId,
  currentEpochMillis,
  currentIsoTimestamp,
  epochMillisToIsoTimestamp,
  isoTimestampAfterIso,
} from './runtime-primitives'

export type AgentSearchMode = 'basic'
export type AgentSearchCacheStatus = 'hit' | 'miss'
export type AgentSearchChargeState = 'free_allowance' | 'paid_entitlement'
export type AgentSearchStatus = 'failed' | 'succeeded'
export type AgentSearchQuotaEventKind = 'provider_request' | 'search_request'

export type AgentSearchPolicy = Readonly<{
  cacheTtlHours: number
  freeDailyLimit: number
  freeHourlyLimit: number
  globalDailyProviderLimit: number
  maxHighlightCharacters: number
  maxQueryLength: number
  maxResults: number
}>

export const AGENT_SEARCH_BASIC_MODE: AgentSearchMode = 'basic'
export const AGENT_SEARCH_ENDPOINT = '/api/agents/search'
export const AGENT_SEARCH_ENTITLEMENT_HEADER =
  'x-openagents-agent-search-entitlement'
export const AGENT_SEARCH_FREE_DAILY_LIMIT = 20
export const AGENT_SEARCH_FREE_HOURLY_LIMIT = 5
export const AGENT_SEARCH_MAX_QUERY_LENGTH = 500
export const AGENT_SEARCH_MAX_RESULTS = 5
export const AGENT_SEARCH_PAYMENT_PREVIEW_ENDPOINT =
  '/api/agents/search/payments/preview'
export const AGENT_SEARCH_PAYMENT_REDEEM_ENDPOINT =
  '/api/agents/search/payments/redeem'
export const AGENT_SEARCH_BASIC_RECOVERY_PRODUCT_ID =
  'product.agent_api.search.basic.single'
export const AGENT_SEARCH_BASIC_RECOVERY_SCOPE_REF =
  'entitlement.agent_api.search.basic.single'
export const AGENT_SEARCH_BASIC_RECOVERY_PRICE = {
  amountMinorUnits: 1,
  asset: 'credits',
  denomination: 'credit',
} as const
const AGENT_SEARCH_CACHE_RESULTS_JSON_LIMIT = 12_000

const unsupportedQueryMaterialPattern =
  /(bearer\s+|cookie:|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github_pat_[a-z0-9_]+|lnbc|lntb|lnbcrt|mnemonic|oauth[_ -]?token|payment[_ -]?preimage|private[_ -]?key|provider[_ -]?token|raw[_ -]?invoice|secret[_ -]?key|sk-[a-z0-9])/i

const hostnamePattern =
  /^(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z0-9][a-z0-9-]{1,62}$/i

export type AgentSearchRuntime = Readonly<{
  makeCacheEntryId: () => string
  makeMetricEventId: () => string
  makeQuotaEventId: () => string
  makeRequestId: () => string
  makeSourceId: () => string
  nowIso: () => string
  nowMillis: () => number
}>

export const systemAgentSearchRuntime: AgentSearchRuntime = {
  makeCacheEntryId: () => compactRandomId('agent_search_cache'),
  makeMetricEventId: () => compactRandomId('agent_search_metric'),
  makeQuotaEventId: () => compactRandomId('agent_search_quota'),
  makeRequestId: () => compactRandomId('agent_search'),
  makeSourceId: () => compactRandomId('agent_search_source'),
  nowIso: currentIsoTimestamp,
  nowMillis: currentEpochMillis,
}

export type AgentSearchRequestRecord = Readonly<{
  actorRef: string
  agentUserId: string
  archivedAt: string | null
  cacheStatus: AgentSearchCacheStatus
  chargeState: AgentSearchChargeState
  completedAt: string | null
  createdAt: string
  credentialId: string
  entitlementRef: string | null
  id: string
  idempotencyKeyHash: string
  mode: AgentSearchMode
  productId: string | null
  provider: 'exa'
  providerCostDollars: number | null
  providerRequestId: string | null
  publicProjectionJson: string
  queryHash: string
  queryText: string | null
  receiptRef: string
  requestBodyDigest: string
  status: AgentSearchStatus
  tokenPrefix: string
}>

export type AgentSearchSourceCard = Readonly<{
  createdAt: string
  domain: string
  highlightText: string | null
  id: string
  publicSafe: boolean
  publishedDate: string | null
  score: number | null
  searchRequestId: string
  selectedTextHash: string | null
  sourceRef: string
  title: string
  url: string
}>

export type AgentSearchQuotaEvent = Readonly<{
  actorRef: string
  createdAt: string
  credentialId: string
  entitlementRef: string | null
  eventKind: AgentSearchQuotaEventKind
  id: string
  mode: AgentSearchMode
  productId: string | null
  units: number
}>

export type AgentSearchMetricEvent = Readonly<{
  actorRef: string
  cacheStatus: AgentSearchCacheStatus | null
  createdAt: string
  credentialId: string | null
  durationMs: number | null
  eventName: string
  id: string
  mode: AgentSearchMode
  providerCostDollars: number | null
  providerStatus: string | null
  resultCount: number | null
}>

export type AgentSearchCacheEntry = Readonly<{
  cacheKey: string
  costDollars: number | null
  createdAt: string
  expiresAt: string
  id: string
  mode: AgentSearchMode
  provider: 'exa'
  results: ReadonlyArray<AgentSearchCachedResult>
}>

export type AgentSearchConsumedEntitlement = Readonly<{
  entitlementRef: string
  productId: string
  receiptRef: string
  scopeRef: string
}>

export type AgentSearchStore = Readonly<{
  consumeEntitlement: (input: {
    actorRef: string
    credentialId: string
    entitlementRef: string
    nowIso: string
    requestBodyDigest: string
  }) => Promise<AgentSearchConsumedEntitlement | undefined>
  countProviderRequestsSince: (sinceIso: string) => Promise<number>
  countQuotaEventsSince: (input: {
    actorRef: string
    credentialId: string
    eventKind: AgentSearchQuotaEventKind
    sinceIso: string
  }) => Promise<number>
  readFreshCache: (
    cacheKey: string,
    nowIso: string,
  ) => Promise<AgentSearchCacheEntry | null>
  readRequestByIdempotencyKeyHash: (
    idempotencyKeyHash: string,
  ) => Promise<AgentSearchRequestRecord | undefined>
  recordMetric: (event: AgentSearchMetricEvent) => Promise<void>
  recordSearch: (input: {
    quotaEvents: ReadonlyArray<AgentSearchQuotaEvent>
    request: AgentSearchRequestRecord
    sources: ReadonlyArray<AgentSearchSourceCard>
  }) => Promise<void>
  storeCache: (entry: AgentSearchCacheEntry) => Promise<void>
}>

export type AgentSearchSession = Readonly<{
  credential: Readonly<{
    id: string
    tokenPrefix: string
  }>
  user: Readonly<{
    id: string
  }>
}>

export const AgentSearchResultProjection = S.Struct({
  domain: S.String,
  highlights: S.Array(S.String),
  id: S.String,
  publishedDate: S.NullOr(S.String),
  score: S.NullOr(S.Number),
  sourceRef: S.String,
  title: S.String,
  url: S.String,
})
export type AgentSearchResultProjection =
  typeof AgentSearchResultProjection.Type

export const AgentSearchResponseProjection = S.Struct({
  search: S.Struct({
    cache: S.Literals(['hit', 'miss']),
    charged: S.Boolean,
    freeAllowance: S.Struct({
      remaining: S.Number,
      resetsAt: S.String,
    }),
    id: S.String,
    mode: S.Literal('basic'),
    payment: S.Struct({
      requiredProductRefs: S.Array(S.String),
      state: S.Literals(['free_allowance', 'paid_entitlement']),
    }),
    receiptRef: S.String,
    results: S.Array(AgentSearchResultProjection),
    status: S.Literal('succeeded'),
  }),
})
export type AgentSearchResponseProjection =
  typeof AgentSearchResponseProjection.Type

export const AgentSearchCachedResult = AgentSearchResultProjection
export type AgentSearchCachedResult = typeof AgentSearchCachedResult.Type

type ValidatedAgentSearchRequest = Readonly<{
  category: ExaSearchCategory | undefined
  excludeDomains: ReadonlyArray<string>
  freshnessMaxAgeHours: number
  includeDomains: ReadonlyArray<string>
  mode: AgentSearchMode
  numResults: number
  query: string
}>

export class AgentSearchValidationError extends Error {
  override readonly name = 'AgentSearchValidationError'
}

export class AgentSearchQuotaExceeded extends Error {
  override readonly name = 'AgentSearchQuotaExceeded'

  constructor(
    message: string,
    readonly resetAt: string,
  ) {
    super(message)
  }
}

export class AgentSearchPaymentRequired extends Error {
  override readonly name = 'AgentSearchPaymentRequired'

  constructor(
    message: string,
    readonly previewHref: string,
    readonly requiredProductRefs: ReadonlyArray<string>,
  ) {
    super(message)
  }
}

export class AgentSearchProviderBudgetExceeded extends Error {
  override readonly name = 'AgentSearchProviderBudgetExceeded'
}

export class AgentSearchStorageError extends Error {
  override readonly name = 'AgentSearchStorageError'

  constructor(
    readonly operation: string,
    readonly rootCause: unknown,
  ) {
    super(`Agent search storage failed during ${operation}.`)
  }
}

export type AgentSearchFailure =
  | AgentSearchPaymentRequired
  | AgentSearchProviderBudgetExceeded
  | AgentSearchQuotaExceeded
  | AgentSearchStorageError
  | AgentSearchValidationError
  | ExaError

export type ExecuteAgentSearchInput = Readonly<{
  body: Record<string, unknown>
  config: OpenAgentsWorkerConfigShape
  exaClient: ExaClientShape
  idempotencyKey: string
  paidEntitlementRef?: string | undefined
  runtime?: AgentSearchRuntime | undefined
  session: AgentSearchSession
  store: AgentSearchStore
}>

export const agentSearchPolicyFromConfig = (
  config: ExaConfig,
): AgentSearchPolicy => ({
  cacheTtlHours: config.cacheTtlHours,
  freeDailyLimit: AGENT_SEARCH_FREE_DAILY_LIMIT,
  freeHourlyLimit: AGENT_SEARCH_FREE_HOURLY_LIMIT,
  globalDailyProviderLimit: config.dailyRequestBudget,
  maxHighlightCharacters: config.maxHighlightCharacters,
  maxQueryLength: AGENT_SEARCH_MAX_QUERY_LENGTH,
  maxResults: Math.min(config.defaultNumResults, AGENT_SEARCH_MAX_RESULTS),
})

const cleanText = (value: string): string => value.trim().replace(/\s+/g, ' ')

const boundedText = (value: string, maxLength: number): string =>
  cleanText(value).slice(0, Math.max(0, maxLength))

const optionalString = (value: unknown): string | undefined =>
  typeof value === 'string' && value.trim() !== '' ? value.trim() : undefined

const optionalInteger = (value: unknown): number | undefined => {
  if (typeof value === 'number' && Number.isInteger(value)) {
    return value
  }

  if (typeof value === 'string' && value.trim() !== '') {
    const parsed = Number(value)

    return Number.isInteger(parsed) ? parsed : undefined
  }

  return undefined
}

const stringArray = (value: unknown, field: string): ReadonlyArray<string> => {
  if (value === undefined) {
    return []
  }

  if (!Array.isArray(value)) {
    throw new AgentSearchValidationError(`${field} must be an array.`)
  }

  if (value.length > 10) {
    throw new AgentSearchValidationError(
      `${field} can include at most 10 domains.`,
    )
  }

  return value.map((item, index) => {
    if (typeof item !== 'string' || item.trim() === '') {
      throw new AgentSearchValidationError(
        `${field}.${index} must be a hostname.`,
      )
    }

    const hostname = item
      .trim()
      .toLowerCase()
      .replace(/^https?:\/\//, '')
      .split('/')[0]
      ?.replace(/\.$/, '')

    if (hostname === undefined || !hostnamePattern.test(hostname)) {
      throw new AgentSearchValidationError(
        `${field}.${index} must be a public hostname.`,
      )
    }

    return hostname
  })
}

const allowedCategories = new Set<ExaSearchCategory>([
  'company',
  'github',
  'linkedin profile',
  'news',
  'pdf',
  'personal site',
  'research paper',
  'tweet',
])

const categoryFromUnknown = (value: unknown): ExaSearchCategory | undefined => {
  if (value === undefined) {
    return undefined
  }

  const category = optionalString(value)

  if (category === undefined) {
    throw new AgentSearchValidationError('category must be a string.')
  }

  if (category === 'people') {
    throw new AgentSearchValidationError(
      'people category search is not enabled for hosted agent search.',
    )
  }

  if (!allowedCategories.has(category as ExaSearchCategory)) {
    throw new AgentSearchValidationError('category is not supported.')
  }

  return category as ExaSearchCategory
}

const assertPublicSafeRequest = (value: unknown): void => {
  const json = JSON.stringify(value)

  if (
    containsProviderSecretMaterial(json) ||
    unsupportedQueryMaterialPattern.test(json)
  ) {
    throw new AgentSearchValidationError(
      'Hosted search requests must not include credentials, payment material, private keys, source archives, or private data.',
    )
  }
}

export const validateAgentSearchRequest = (
  body: Record<string, unknown>,
  policy: AgentSearchPolicy,
): ValidatedAgentSearchRequest => {
  const mode = optionalString(body.mode) ?? AGENT_SEARCH_BASIC_MODE

  if (mode !== AGENT_SEARCH_BASIC_MODE) {
    throw new AgentSearchValidationError(
      'Only basic hosted search is available in this release.',
    )
  }

  const query = optionalString(body.query)

  if (query === undefined || cleanText(query).length < 3) {
    throw new AgentSearchValidationError('query must be at least 3 characters.')
  }

  if (cleanText(query).length > policy.maxQueryLength) {
    throw new AgentSearchValidationError(
      `query must be at most ${policy.maxQueryLength} characters.`,
    )
  }

  const contents = body.contents

  if (contents !== undefined) {
    if (
      typeof contents !== 'object' ||
      contents === null ||
      Array.isArray(contents)
    ) {
      throw new AgentSearchValidationError('contents must be an object.')
    }

    const record = contents as Record<string, unknown>

    if (record.summary !== undefined && record.summary !== false) {
      throw new AgentSearchValidationError(
        'summary content is not enabled for free basic search.',
      )
    }

    if (record.text !== undefined && record.text !== false) {
      throw new AgentSearchValidationError(
        'text content is not enabled for free basic search.',
      )
    }
  }

  const numResults = optionalInteger(body.numResults) ?? policy.maxResults

  if (numResults < 1 || numResults > policy.maxResults) {
    throw new AgentSearchValidationError(
      `numResults must be between 1 and ${policy.maxResults}.`,
    )
  }

  const request = {
    category: categoryFromUnknown(body.category),
    excludeDomains: stringArray(body.excludeDomains, 'excludeDomains'),
    freshnessMaxAgeHours: 24,
    includeDomains: stringArray(body.includeDomains, 'includeDomains'),
    mode,
    numResults,
    query: cleanText(query),
  } satisfies ValidatedAgentSearchRequest

  assertPublicSafeRequest(request)

  return request
}

const projectionFromJson = (
  value: string,
): AgentSearchResponseProjection | undefined => {
  try {
    return parseJsonWithSchema(AgentSearchResponseProjection, value)
  } catch {
    return undefined
  }
}

const domainFromUrl = (url: string, fallback: string | undefined): string => {
  try {
    return new URL(url).hostname.toLowerCase()
  } catch {
    return fallback?.trim().toLowerCase() || 'unknown.invalid'
  }
}

const validPublicUrl = (url: string): boolean => {
  try {
    const parsed = new URL(url)

    return parsed.protocol === 'http:' || parsed.protocol === 'https:'
  } catch {
    return false
  }
}

const sourceHighlights = (
  result: ExaSearchResult,
  maxCharacters: number,
): ReadonlyArray<string> => {
  const highlights = result.highlights ?? result.contents?.highlights ?? []

  return highlights
    .filter((highlight): highlight is string => typeof highlight === 'string')
    .map(highlight => boundedText(highlight, maxCharacters))
    .filter(highlight => highlight !== '')
    .slice(0, 3)
}

const sourceTitle = (result: ExaSearchResult, domain: string): string =>
  boundedText(result.title ?? domain, 200) || domain

const projectExaResults = async (
  input: Readonly<{
    maxHighlightCharacters: number
    results: ReadonlyArray<ExaSearchResult>
    runtime: AgentSearchRuntime
    searchRequestId: string
  }>,
): Promise<ReadonlyArray<AgentSearchSourceCard>> => {
  const cards: Array<AgentSearchSourceCard> = []

  for (const result of input.results) {
    if (!validPublicUrl(result.url)) {
      continue
    }

    const id = input.runtime.makeSourceId()
    const domain = domainFromUrl(result.url, result.domain)
    const highlights = sourceHighlights(result, input.maxHighlightCharacters)
    const highlightText = highlights.join('\n\n') || null
    const title = sourceTitle(result, domain)
    const sourceRef = `agent_search_source:${id}`
    const selectedTextHash =
      highlightText === null ? null : await sha256Hex(highlightText)

    cards.push({
      createdAt: input.runtime.nowIso(),
      domain,
      highlightText,
      id,
      publicSafe: true,
      publishedDate: result.publishedDate ?? null,
      score: typeof result.score === 'number' ? result.score : null,
      searchRequestId: input.searchRequestId,
      selectedTextHash,
      sourceRef,
      title,
      url: result.url,
    })
  }

  return cards.slice(0, AGENT_SEARCH_MAX_RESULTS)
}

const resultProjectionFromSource = (
  source: AgentSearchSourceCard,
): AgentSearchResultProjection => ({
  domain: source.domain,
  highlights:
    source.highlightText === null
      ? []
      : source.highlightText.split('\n\n').filter(text => text !== ''),
  id: source.id,
  publishedDate: source.publishedDate,
  score: source.score,
  sourceRef: source.sourceRef,
  title: source.title,
  url: source.url,
})

const sourceFromCachedResult = (
  input: Readonly<{
    cached: AgentSearchCachedResult
    runtime: AgentSearchRuntime
    searchRequestId: string
  }>,
): AgentSearchSourceCard => {
  const id = input.runtime.makeSourceId()
  const highlightText =
    input.cached.highlights.length === 0
      ? null
      : input.cached.highlights.join('\n\n')

  return {
    createdAt: input.runtime.nowIso(),
    domain: input.cached.domain,
    highlightText,
    id,
    publicSafe: true,
    publishedDate: input.cached.publishedDate,
    score: input.cached.score,
    searchRequestId: input.searchRequestId,
    selectedTextHash: null,
    sourceRef: `agent_search_source:${id}`,
    title: input.cached.title,
    url: input.cached.url,
  }
}

const searchCacheKey = async (
  request: ValidatedAgentSearchRequest,
): Promise<string> =>
  sha256Hex(
    JSON.stringify({
      category: request.category ?? null,
      excludeDomains: [...request.excludeDomains].sort(),
      freshnessMaxAgeHours: request.freshnessMaxAgeHours,
      includeDomains: [...request.includeDomains].sort(),
      mode: request.mode,
      numResults: request.numResults,
      provider: 'exa',
      query: request.query,
      version: 1,
    }),
  )

const startOfHourIso = (nowMillis: number): string => {
  const hour = 60 * 60 * 1000

  return epochMillisToIsoTimestamp(Math.floor(nowMillis / hour) * hour)
}

const startOfDayIso = (nowIso: string): string =>
  `${nowIso.slice(0, 10)}T00:00:00.000Z`

const nextHourIso = (nowMillis: number): string => {
  const hour = 60 * 60 * 1000

  return epochMillisToIsoTimestamp((Math.floor(nowMillis / hour) + 1) * hour)
}

const quotaRemaining = (policy: AgentSearchPolicy, usedDaily: number): number =>
  Math.max(0, policy.freeDailyLimit - usedDaily - 1)

const paidQuotaRemaining = (
  policy: AgentSearchPolicy,
  usedDaily: number,
): number => Math.max(0, policy.freeDailyLimit - usedDaily)

const agentSearchPaymentRequired = (
  reason: string,
): AgentSearchPaymentRequired =>
  new AgentSearchPaymentRequired(
    reason,
    AGENT_SEARCH_PAYMENT_PREVIEW_ENDPOINT,
    [AGENT_SEARCH_BASIC_RECOVERY_PRODUCT_ID],
  )

const assertWithinFreeQuota = async (
  input: Readonly<{
    actorRef: string
    credentialId: string
    nowIso: string
    nowMillis: number
    policy: AgentSearchPolicy
    store: AgentSearchStore
  }>,
): Promise<
  Readonly<{ dailyUsed: number; hourlyUsed: number; resetAt: string }>
> => {
  const hourlyUsed = await input.store.countQuotaEventsSince({
    actorRef: input.actorRef,
    credentialId: input.credentialId,
    eventKind: 'search_request',
    sinceIso: startOfHourIso(input.nowMillis),
  })
  const dailyUsed = await input.store.countQuotaEventsSince({
    actorRef: input.actorRef,
    credentialId: input.credentialId,
    eventKind: 'search_request',
    sinceIso: startOfDayIso(input.nowIso),
  })

  if (hourlyUsed >= input.policy.freeHourlyLimit) {
    throw new AgentSearchQuotaExceeded(
      'Hosted search free hourly limit exceeded.',
      nextHourIso(input.nowMillis),
    )
  }

  if (dailyUsed >= input.policy.freeDailyLimit) {
    throw new AgentSearchQuotaExceeded(
      'Hosted search free daily limit exceeded.',
      isoTimestampAfterIso(startOfDayIso(input.nowIso), 24 * 60 * 60 * 1000),
    )
  }

  return { dailyUsed, hourlyUsed, resetAt: nextHourIso(input.nowMillis) }
}

const readFreeQuotaUsage = async (
  input: Readonly<{
    actorRef: string
    credentialId: string
    nowIso: string
    nowMillis: number
    store: AgentSearchStore
  }>,
): Promise<
  Readonly<{ dailyUsed: number; hourlyUsed: number; resetAt: string }>
> => {
  const hourlyUsed = await input.store.countQuotaEventsSince({
    actorRef: input.actorRef,
    credentialId: input.credentialId,
    eventKind: 'search_request',
    sinceIso: startOfHourIso(input.nowMillis),
  })
  const dailyUsed = await input.store.countQuotaEventsSince({
    actorRef: input.actorRef,
    credentialId: input.credentialId,
    eventKind: 'search_request',
    sinceIso: startOfDayIso(input.nowIso),
  })

  return { dailyUsed, hourlyUsed, resetAt: nextHourIso(input.nowMillis) }
}

const assertProviderBudget = async (
  input: Readonly<{
    nowIso: string
    policy: AgentSearchPolicy
    store: AgentSearchStore
  }>,
): Promise<void> => {
  const used = await input.store.countProviderRequestsSince(
    startOfDayIso(input.nowIso),
  )

  if (used >= input.policy.globalDailyProviderLimit) {
    throw new AgentSearchProviderBudgetExceeded(
      'Hosted search provider budget is temporarily exhausted.',
    )
  }
}

const requestProjection = (
  input: Readonly<{
    cacheStatus: AgentSearchCacheStatus
    charged: boolean
    dailyUsed: number
    paymentState: 'free_allowance' | 'paid_entitlement'
    policy: AgentSearchPolicy
    receiptRef: string
    requestId: string
    resetAt: string
    results: ReadonlyArray<AgentSearchResultProjection>
  }>,
): AgentSearchResponseProjection => ({
  search: {
    cache: input.cacheStatus,
    charged: input.charged,
    freeAllowance: {
      remaining:
        input.paymentState === 'paid_entitlement'
          ? paidQuotaRemaining(input.policy, input.dailyUsed)
          : quotaRemaining(input.policy, input.dailyUsed),
      resetsAt: input.resetAt,
    },
    id: input.requestId,
    mode: AGENT_SEARCH_BASIC_MODE,
    payment: {
      requiredProductRefs: [],
      state: input.paymentState,
    },
    receiptRef: input.receiptRef,
    results: [...input.results],
    status: 'succeeded',
  },
})

const makeQuotaEvent = (
  input: Readonly<{
    actorRef: string
    credentialId: string
    eventKind: AgentSearchQuotaEventKind
    entitlementRef?: string | null | undefined
    nowIso: string
    productId?: string | null | undefined
    runtime: AgentSearchRuntime
  }>,
): AgentSearchQuotaEvent => ({
  actorRef: input.actorRef,
  createdAt: input.nowIso,
  credentialId: input.credentialId,
  entitlementRef: input.entitlementRef ?? null,
  eventKind: input.eventKind,
  id: input.runtime.makeQuotaEventId(),
  mode: AGENT_SEARCH_BASIC_MODE,
  productId: input.productId ?? null,
  units: 1,
})

const makeMetric = (
  input: Readonly<{
    actorRef: string
    cacheStatus: AgentSearchCacheStatus | null
    credentialId: string
    durationMs: number | null
    eventName: string
    providerCostDollars?: number | null | undefined
    providerStatus?: string | null | undefined
    resultCount?: number | null | undefined
    runtime: AgentSearchRuntime
  }>,
): AgentSearchMetricEvent => ({
  actorRef: input.actorRef,
  cacheStatus: input.cacheStatus,
  createdAt: input.runtime.nowIso(),
  credentialId: input.credentialId,
  durationMs: input.durationMs,
  eventName: input.eventName,
  id: input.runtime.makeMetricEventId(),
  mode: AGENT_SEARCH_BASIC_MODE,
  providerCostDollars: input.providerCostDollars ?? null,
  providerStatus: input.providerStatus ?? null,
  resultCount: input.resultCount ?? null,
})

export const executeAgentSearch = (
  input: ExecuteAgentSearchInput,
): Effect.Effect<AgentSearchResponseProjection, AgentSearchFailure> =>
  Effect.gen(function* () {
    const runtime = input.runtime ?? systemAgentSearchRuntime
    const policy = agentSearchPolicyFromConfig(input.config.exa)
    const nowIso = runtime.nowIso()
    const nowMillis = runtime.nowMillis()
    const actorRef = `agent:${input.session.user.id}`
    const request = yield* Effect.try({
      catch: error =>
        error instanceof AgentSearchValidationError
          ? error
          : new AgentSearchValidationError('Hosted search request is invalid.'),
      try: () => validateAgentSearchRequest(input.body, policy),
    })
    const requestBodyDigest = yield* Effect.tryPromise({
      catch: error => new AgentSearchStorageError('search.digest', error),
      try: () => sha256Hex(JSON.stringify(request)),
    })
    const idempotencyKeyHash = yield* Effect.tryPromise({
      catch: error => new AgentSearchStorageError('search.idempotency', error),
      try: () => sha256Hex(`${actorRef}:${input.idempotencyKey}`),
    })
    const existing = yield* Effect.tryPromise({
      catch: error =>
        new AgentSearchStorageError('search.idempotency.read', error),
      try: () =>
        input.store.readRequestByIdempotencyKeyHash(idempotencyKeyHash),
    })

    if (existing !== undefined) {
      const projection = projectionFromJson(existing.publicProjectionJson)

      if (projection !== undefined) {
        return projection
      }
    }

    const paidEntitlement =
      input.paidEntitlementRef === undefined
        ? null
        : yield* Effect.tryPromise({
            catch: error =>
              error instanceof AgentSearchPaymentRequired
                ? error
                : new AgentSearchStorageError(
                    'search.entitlement.consume',
                    error,
                  ),
            try: async () => {
              const consumed = await input.store.consumeEntitlement({
                actorRef,
                credentialId: input.session.credential.id,
                entitlementRef: input.paidEntitlementRef ?? '',
                nowIso,
                requestBodyDigest,
              })

              if (consumed === undefined) {
                throw agentSearchPaymentRequired(
                  'A valid paid hosted-search entitlement is required for this request.',
                )
              }

              return consumed
            },
          })
    const quota =
      paidEntitlement === null
        ? yield* Effect.tryPromise({
            catch: error =>
              error instanceof AgentSearchQuotaExceeded
                ? agentSearchPaymentRequired(error.message)
                : new AgentSearchStorageError('search.quota.read', error),
            try: () =>
              assertWithinFreeQuota({
                actorRef,
                credentialId: input.session.credential.id,
                nowIso,
                nowMillis,
                policy,
                store: input.store,
              }),
          })
        : yield* Effect.tryPromise({
            catch: error =>
              new AgentSearchStorageError('search.quota.read_paid', error),
            try: () =>
              readFreeQuotaUsage({
                actorRef,
                credentialId: input.session.credential.id,
                nowIso,
                nowMillis,
                store: input.store,
              }),
          })
    const cacheKey = yield* Effect.tryPromise({
      catch: error => new AgentSearchStorageError('search.cache.key', error),
      try: () => searchCacheKey(request),
    })
    const cached = yield* Effect.tryPromise({
      catch: error => new AgentSearchStorageError('search.cache.read', error),
      try: () => input.store.readFreshCache(cacheKey, nowIso),
    })
    const startedAt = runtime.nowMillis()
    const requestId = runtime.makeRequestId()
    const receiptRef = `receipt.agent_search.${requestId}`
    const chargeState: AgentSearchChargeState =
      paidEntitlement === null ? 'free_allowance' : 'paid_entitlement'

    if (cached !== null) {
      const sources = cached.results.map(cachedResult =>
        sourceFromCachedResult({
          cached: cachedResult,
          runtime,
          searchRequestId: requestId,
        }),
      )
      const results = sources.map(resultProjectionFromSource)
      const projection = requestProjection({
        cacheStatus: 'hit',
        charged: paidEntitlement !== null,
        dailyUsed: quota.dailyUsed,
        paymentState:
          paidEntitlement === null ? 'free_allowance' : 'paid_entitlement',
        policy,
        receiptRef,
        requestId,
        resetAt: quota.resetAt,
        results,
      })
      const projectionJson = JSON.stringify(projection)

      assertPublicSafeRequest(projection)

      yield* Effect.tryPromise({
        catch: error => new AgentSearchStorageError('search.record.hit', error),
        try: () =>
          input.store.recordSearch({
            quotaEvents: [
              makeQuotaEvent({
                actorRef,
                credentialId: input.session.credential.id,
                eventKind: 'search_request',
                entitlementRef: paidEntitlement?.entitlementRef ?? null,
                nowIso,
                productId: paidEntitlement?.productId ?? null,
                runtime,
              }),
            ],
            request: {
              actorRef,
              agentUserId: input.session.user.id,
              archivedAt: null,
              cacheStatus: 'hit',
              chargeState,
              completedAt: runtime.nowIso(),
              createdAt: nowIso,
              credentialId: input.session.credential.id,
              entitlementRef: paidEntitlement?.entitlementRef ?? null,
              id: requestId,
              idempotencyKeyHash,
              mode: AGENT_SEARCH_BASIC_MODE,
              productId: paidEntitlement?.productId ?? null,
              provider: 'exa',
              providerCostDollars: cached.costDollars,
              providerRequestId: null,
              publicProjectionJson: projectionJson,
              queryHash: requestBodyDigest,
              queryText: request.query,
              receiptRef,
              requestBodyDigest,
              status: 'succeeded',
              tokenPrefix: input.session.credential.tokenPrefix,
            },
            sources,
          }),
      })

      yield* Effect.tryPromise({
        catch: error => new AgentSearchStorageError('search.metric.hit', error),
        try: () =>
          input.store.recordMetric(
            makeMetric({
              actorRef,
              cacheStatus: 'hit',
              credentialId: input.session.credential.id,
              durationMs: runtime.nowMillis() - startedAt,
              eventName: 'agent_search.succeeded',
              providerCostDollars: cached.costDollars,
              providerStatus: 'cache_hit',
              resultCount: results.length,
              runtime,
            }),
          ),
      })

      return projection
    }

    yield* Effect.tryPromise({
      catch: error =>
        error instanceof AgentSearchProviderBudgetExceeded
          ? error
          : new AgentSearchStorageError('search.provider_budget.read', error),
      try: () =>
        assertProviderBudget({
          nowIso,
          policy,
          store: input.store,
        }),
    })

    const exaSearchInput = {
      ...(request.category === undefined ? {} : { category: request.category }),
      ...(request.excludeDomains.length === 0
        ? {}
        : { excludeDomains: [...request.excludeDomains] }),
      ...(request.includeDomains.length === 0
        ? {}
        : { includeDomains: [...request.includeDomains] }),
      contents: {
        highlights: {
          maxCharacters: policy.maxHighlightCharacters,
        },
        maxAgeHours: request.freshnessMaxAgeHours,
      },
      numResults: request.numResults,
      query: request.query,
      type: 'auto' as const,
    }
    const response = yield* retryExaEffect(
      exaEnrichmentOperationsPolicyFromConfig(input.config.exa),
      input.exaClient.search(exaSearchInput),
    )
    const sources = yield* Effect.tryPromise({
      catch: error => new AgentSearchStorageError('search.project', error),
      try: () =>
        projectExaResults({
          maxHighlightCharacters: policy.maxHighlightCharacters,
          results: response.results,
          runtime,
          searchRequestId: requestId,
        }),
    })
    const results = sources.map(resultProjectionFromSource)
    const projection = requestProjection({
      cacheStatus: 'miss',
      charged: paidEntitlement !== null,
      dailyUsed: quota.dailyUsed,
      paymentState:
        paidEntitlement === null ? 'free_allowance' : 'paid_entitlement',
      policy,
      receiptRef,
      requestId,
      resetAt: quota.resetAt,
      results,
    })
    const projectionJson = JSON.stringify(projection)

    assertPublicSafeRequest(projection)

    yield* Effect.tryPromise({
      catch: error => new AgentSearchStorageError('search.cache.store', error),
      try: () =>
        input.store.storeCache({
          cacheKey,
          costDollars: response.costDollars ?? null,
          createdAt: nowIso,
          expiresAt: isoTimestampAfterIso(
            nowIso,
            policy.cacheTtlHours * 3_600_000,
          ),
          id: runtime.makeCacheEntryId(),
          mode: AGENT_SEARCH_BASIC_MODE,
          provider: 'exa',
          results,
        }),
    })
    yield* Effect.tryPromise({
      catch: error => new AgentSearchStorageError('search.record.miss', error),
      try: () =>
        input.store.recordSearch({
          quotaEvents: [
            makeQuotaEvent({
              actorRef,
              credentialId: input.session.credential.id,
              eventKind: 'search_request',
              entitlementRef: paidEntitlement?.entitlementRef ?? null,
              nowIso,
              productId: paidEntitlement?.productId ?? null,
              runtime,
            }),
            makeQuotaEvent({
              actorRef,
              credentialId: input.session.credential.id,
              eventKind: 'provider_request',
              entitlementRef: paidEntitlement?.entitlementRef ?? null,
              nowIso,
              productId: paidEntitlement?.productId ?? null,
              runtime,
            }),
          ],
          request: {
            actorRef,
            agentUserId: input.session.user.id,
            archivedAt: null,
            cacheStatus: 'miss',
            chargeState,
            completedAt: runtime.nowIso(),
            createdAt: nowIso,
            credentialId: input.session.credential.id,
            entitlementRef: paidEntitlement?.entitlementRef ?? null,
            id: requestId,
            idempotencyKeyHash,
            mode: AGENT_SEARCH_BASIC_MODE,
            productId: paidEntitlement?.productId ?? null,
            provider: 'exa',
            providerCostDollars: response.costDollars ?? null,
            providerRequestId: response.requestId ?? null,
            publicProjectionJson: projectionJson,
            queryHash: requestBodyDigest,
            queryText: request.query,
            receiptRef,
            requestBodyDigest,
            status: 'succeeded',
            tokenPrefix: input.session.credential.tokenPrefix,
          },
          sources,
        }),
    })
    yield* Effect.tryPromise({
      catch: error => new AgentSearchStorageError('search.metric.miss', error),
      try: () =>
        input.store.recordMetric(
          makeMetric({
            actorRef,
            cacheStatus: 'miss',
            credentialId: input.session.credential.id,
            durationMs: runtime.nowMillis() - startedAt,
            eventName: 'agent_search.succeeded',
            providerCostDollars: response.costDollars ?? null,
            providerStatus: 'provider_success',
            resultCount: results.length,
            runtime,
          }),
        ),
    })

    return projection
  })

type AgentSearchRequestRow = Readonly<{
  actor_ref: string
  agent_user_id: string
  archived_at: string | null
  cache_status: AgentSearchCacheStatus
  charge_state: AgentSearchChargeState
  completed_at: string | null
  created_at: string
  credential_id: string
  entitlement_ref: string | null
  id: string
  idempotency_key_hash: string
  mode: AgentSearchMode
  product_id: string | null
  provider: 'exa'
  provider_cost_dollars: number | null
  provider_request_id: string | null
  public_projection_json: string
  query_hash: string
  query_text: string | null
  receipt_ref: string
  request_body_digest: string
  status: AgentSearchStatus
  token_prefix: string
}>

type AgentSearchCacheRow = Readonly<{
  cache_key: string
  cost_dollars: number | null
  created_at: string
  expires_at: string
  id: string
  mode: AgentSearchMode
  provider: 'exa'
  results_json: string
}>

type AgentSearchEntitlementRow = Readonly<{
  entitlement_ref: string
  product_id: string
  receipt_ref: string
  scope_ref: string
}>

type CountRow = Readonly<{ count: number | null }>

const rowToRequest = (
  row: AgentSearchRequestRow,
): AgentSearchRequestRecord => ({
  actorRef: row.actor_ref,
  agentUserId: row.agent_user_id,
  archivedAt: row.archived_at,
  cacheStatus: row.cache_status,
  chargeState: row.charge_state,
  completedAt: row.completed_at,
  createdAt: row.created_at,
  credentialId: row.credential_id,
  entitlementRef: row.entitlement_ref,
  id: row.id,
  idempotencyKeyHash: row.idempotency_key_hash,
  mode: row.mode,
  productId: row.product_id,
  provider: row.provider,
  providerCostDollars: row.provider_cost_dollars,
  providerRequestId: row.provider_request_id,
  publicProjectionJson: row.public_projection_json,
  queryHash: row.query_hash,
  queryText: row.query_text,
  receiptRef: row.receipt_ref,
  requestBodyDigest: row.request_body_digest,
  status: row.status,
  tokenPrefix: row.token_prefix,
})

const rowToCache = (row: AgentSearchCacheRow): AgentSearchCacheEntry => ({
  cacheKey: row.cache_key,
  costDollars: row.cost_dollars,
  createdAt: row.created_at,
  expiresAt: row.expires_at,
  id: row.id,
  mode: row.mode,
  provider: row.provider,
  results: parseJsonWithSchema(
    S.Array(AgentSearchCachedResult),
    row.results_json,
  ),
})

const safePublicProjectionJson = (value: string): string =>
  containsProviderSecretMaterial(value) ? '{}' : value

export const makeD1AgentSearchStore = (db: D1Database): AgentSearchStore => ({
  consumeEntitlement: async input => {
    const row = await db
      .prepare(
        `SELECT entitlement_ref,
                product_id,
                receipt_ref,
                scope_ref
           FROM agent_search_entitlements
          WHERE entitlement_ref = ?
            AND actor_ref = ?
            AND credential_id = ?
            AND request_body_digest = ?
            AND status = 'active'
            AND expires_at > ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(
        input.entitlementRef,
        input.actorRef,
        input.credentialId,
        input.requestBodyDigest,
        input.nowIso,
      )
      .first<AgentSearchEntitlementRow>()

    if (row === null) {
      return undefined
    }

    const result = await db
      .prepare(
        `UPDATE agent_search_entitlements
            SET status = 'consumed',
                consumed_at = ?
          WHERE entitlement_ref = ?
            AND actor_ref = ?
            AND credential_id = ?
            AND request_body_digest = ?
            AND status = 'active'
            AND expires_at > ?
            AND archived_at IS NULL`,
      )
      .bind(
        input.nowIso,
        input.entitlementRef,
        input.actorRef,
        input.credentialId,
        input.requestBodyDigest,
        input.nowIso,
      )
      .run()

    if (!result.success || result.meta.changes < 1) {
      return undefined
    }

    return {
      entitlementRef: row.entitlement_ref,
      productId: row.product_id,
      receiptRef: row.receipt_ref,
      scopeRef: row.scope_ref,
    }
  },
  countProviderRequestsSince: async sinceIso => {
    const row = await db
      .prepare(
        `SELECT COUNT(*) AS count
           FROM agent_search_quota_events
          WHERE event_kind = 'provider_request'
            AND created_at >= ?`,
      )
      .bind(sinceIso)
      .first<CountRow>()

    return row?.count ?? 0
  },
  countQuotaEventsSince: async input => {
    const row = await db
      .prepare(
        `SELECT COUNT(*) AS count
           FROM agent_search_quota_events
          WHERE actor_ref = ?
            AND credential_id = ?
            AND event_kind = ?
            AND created_at >= ?`,
      )
      .bind(input.actorRef, input.credentialId, input.eventKind, input.sinceIso)
      .first<CountRow>()

    return row?.count ?? 0
  },
  readFreshCache: async (cacheKey, nowIso) => {
    const row = await db
      .prepare(
        `SELECT id,
                cache_key,
                mode,
                provider,
                results_json,
                cost_dollars,
                created_at,
                expires_at
           FROM agent_search_cache_entries
          WHERE cache_key = ?
            AND expires_at > ?
            AND archived_at IS NULL
          ORDER BY expires_at DESC
          LIMIT 1`,
      )
      .bind(cacheKey, nowIso)
      .first<AgentSearchCacheRow>()

    return row === null ? null : rowToCache(row)
  },
  readRequestByIdempotencyKeyHash: async idempotencyKeyHash => {
    const row = await db
      .prepare(
        `SELECT id,
                receipt_ref,
                actor_ref,
                agent_user_id,
                credential_id,
                token_prefix,
                idempotency_key_hash,
                request_body_digest,
                query_hash,
                query_text,
                mode,
                provider,
                provider_request_id,
                status,
                cache_status,
                charge_state,
                product_id,
                entitlement_ref,
                provider_cost_dollars,
                public_projection_json,
                created_at,
                completed_at,
                archived_at
           FROM agent_search_requests
          WHERE idempotency_key_hash = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(idempotencyKeyHash)
      .first<AgentSearchRequestRow>()

    return row === null ? undefined : rowToRequest(row)
  },
  recordMetric: async event => {
    await db
      .prepare(
        `INSERT INTO agent_search_metric_events
           (id,
            actor_ref,
            credential_id,
            event_name,
            mode,
            cache_status,
            provider_status,
            provider_cost_dollars,
            duration_ms,
            result_count,
            created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        event.id,
        event.actorRef,
        event.credentialId,
        event.eventName,
        event.mode,
        event.cacheStatus,
        event.providerStatus,
        event.providerCostDollars,
        event.durationMs,
        event.resultCount,
        event.createdAt,
      )
      .run()
  },
  recordSearch: async input => {
    const statements = [
      db
        .prepare(
          `INSERT OR IGNORE INTO agent_search_requests
             (id,
              receipt_ref,
              actor_ref,
              agent_user_id,
              credential_id,
              token_prefix,
              idempotency_key_hash,
              request_body_digest,
              query_hash,
              query_text,
              mode,
              provider,
              provider_request_id,
              status,
              cache_status,
              charge_state,
              product_id,
              entitlement_ref,
              provider_cost_dollars,
              public_projection_json,
              created_at,
              completed_at,
              archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          input.request.id,
          input.request.receiptRef,
          input.request.actorRef,
          input.request.agentUserId,
          input.request.credentialId,
          input.request.tokenPrefix,
          input.request.idempotencyKeyHash,
          input.request.requestBodyDigest,
          input.request.queryHash,
          input.request.queryText,
          input.request.mode,
          input.request.provider,
          input.request.providerRequestId,
          input.request.status,
          input.request.cacheStatus,
          input.request.chargeState,
          input.request.productId,
          input.request.entitlementRef,
          input.request.providerCostDollars,
          safePublicProjectionJson(input.request.publicProjectionJson),
          input.request.createdAt,
          input.request.completedAt,
          input.request.archivedAt,
        ),
      ...input.sources.map(source =>
        db
          .prepare(
            `INSERT OR IGNORE INTO agent_search_sources
               (id,
                search_request_id,
                source_ref,
                title,
                url,
                domain,
                published_date,
                score,
                highlight_text,
                selected_text_hash,
                public_safe,
                created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
          )
          .bind(
            source.id,
            source.searchRequestId,
            source.sourceRef,
            source.title,
            source.url,
            source.domain,
            source.publishedDate,
            source.score,
            source.highlightText,
            source.selectedTextHash,
            source.publicSafe ? 1 : 0,
            source.createdAt,
          ),
      ),
      ...input.quotaEvents.map(event =>
        db
          .prepare(
            `INSERT OR IGNORE INTO agent_search_quota_events
               (id,
                actor_ref,
                credential_id,
                event_kind,
                mode,
                units,
                product_id,
                entitlement_ref,
                created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
          )
          .bind(
            event.id,
            event.actorRef,
            event.credentialId,
            event.eventKind,
            event.mode,
            event.units,
            event.productId,
            event.entitlementRef,
            event.createdAt,
          ),
      ),
    ]

    await db.batch(statements)
  },
  storeCache: async entry => {
    const resultsJson = JSON.stringify(entry.results)

    if (
      resultsJson.length > AGENT_SEARCH_CACHE_RESULTS_JSON_LIMIT ||
      containsProviderSecretMaterial(resultsJson)
    ) {
      return
    }

    await db.batch([
      db
        .prepare(
          `UPDATE agent_search_cache_entries
              SET archived_at = ?
            WHERE cache_key = ?
              AND archived_at IS NULL`,
        )
        .bind(entry.createdAt, entry.cacheKey),
      db
        .prepare(
          `INSERT INTO agent_search_cache_entries
             (id,
              cache_key,
              mode,
              provider,
              results_json,
              result_count,
              cost_dollars,
              created_at,
              expires_at,
              archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          entry.id,
          entry.cacheKey,
          entry.mode,
          entry.provider,
          resultsJson,
          entry.results.length,
          entry.costDollars,
          entry.createdAt,
          entry.expiresAt,
          null,
        ),
    ])
  },
})

export const agentSearchErrorCode = (error: AgentSearchFailure): string => {
  if (error instanceof AgentSearchValidationError) {
    return 'validation_error'
  }

  if (error instanceof AgentSearchPaymentRequired) {
    return 'payment_required'
  }

  if (error instanceof AgentSearchQuotaExceeded) {
    return 'free_quota_exceeded'
  }

  if (error instanceof AgentSearchProviderBudgetExceeded) {
    return 'provider_budget_exhausted'
  }

  if (error instanceof ExaConfigurationDisabled) {
    return 'exa_disabled'
  }

  if (error instanceof ExaProviderHttpError) {
    return `exa_http_${error.status}`
  }

  if (error instanceof ExaProviderTimeout) {
    return 'exa_timeout'
  }

  if (error instanceof ExaProviderFetchError) {
    return 'exa_fetch_error'
  }

  if (error instanceof ExaProviderInvalidJson) {
    return 'exa_invalid_json'
  }

  if (error instanceof ExaProviderSchemaError) {
    return 'exa_schema_error'
  }

  if (error instanceof AgentSearchStorageError) {
    return 'storage_error'
  }

  return 'unknown_error'
}

export const parseStoredAgentSearchProjection = (
  record: AgentSearchRequestRecord,
): Record<string, unknown> =>
  parseJsonRecord(record.publicProjectionJson) ?? {
    error: 'stored_projection_unavailable',
  }
