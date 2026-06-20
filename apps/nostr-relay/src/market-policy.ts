import { isGeneralCoordinationKind } from "./general-policy"

export type MarketKindBucket =
  | "nip90_request"
  | "nip90_result"
  | "nip90_feedback"
  | "nip_ds"
  | "nip89_handler"

export type ReqFilter = Readonly<{
  ids?: ReadonlyArray<unknown>
  authors?: ReadonlyArray<unknown>
  kinds?: ReadonlyArray<unknown>
  limit?: unknown
  "#e"?: ReadonlyArray<unknown>
  "#p"?: ReadonlyArray<unknown>
  "#a"?: ReadonlyArray<unknown>
  "#d"?: ReadonlyArray<unknown>
  "#t"?: ReadonlyArray<unknown>
}>

export type PublishBucket = Readonly<{
  count: number
  startedAt: number
}>

export const MarketRelayPolicy = {
  eventRetentionSeconds: 60 * 60 * 24 * 30,
  handlerRetentionSeconds: 60 * 60 * 24 * 180,
  maxEventContentBytes: 64 * 1024,
  maxFiltersPerReq: 4,
  maxIdsPerFilter: 64,
  maxAuthorsPerFilter: 16,
  maxKindsPerFilter: 16,
  maxTagValuesPerFilter: 32,
  maxReqLimit: 100,
  publishRateLimitMaxEvents: 24,
  publishRateLimitWindowMs: 60_000,
} as const

export const marketKindBucket = (kind: number): MarketKindBucket | null => {
  if (kind >= 5000 && kind <= 5999) {
    return "nip90_request"
  }

  if (kind >= 6000 && kind <= 6999) {
    return "nip90_result"
  }

  if (kind === 7000) {
    return "nip90_feedback"
  }

  if (kind === 30404 || kind === 30406) {
    return "nip_ds"
  }

  if (kind === 31989 || kind === 31990) {
    return "nip89_handler"
  }

  return null
}

export const isAllowedMarketKind = (kind: number): boolean =>
  marketKindBucket(kind) !== null

export const marketKindPolicySummary = {
  allowedKinds: {
    nip90Requests: "5000-5999",
    nip90Results: "6000-6999",
    nip90Feedback: [7000],
    nipDs: [30404, 30406],
    nip89Handlers: [31989, 31990],
  },
  retention: {
    marketEventsSeconds: MarketRelayPolicy.eventRetentionSeconds,
    handlerEventsSeconds: MarketRelayPolicy.handlerRetentionSeconds,
  },
  reqLimits: {
    maxFiltersPerReq: MarketRelayPolicy.maxFiltersPerReq,
    maxIdsPerFilter: MarketRelayPolicy.maxIdsPerFilter,
    maxAuthorsPerFilter: MarketRelayPolicy.maxAuthorsPerFilter,
    maxKindsPerFilter: MarketRelayPolicy.maxKindsPerFilter,
    maxTagValuesPerFilter: MarketRelayPolicy.maxTagValuesPerFilter,
    maxLimit: MarketRelayPolicy.maxReqLimit,
  },
  publishRateLimit: {
    maxEvents: MarketRelayPolicy.publishRateLimitMaxEvents,
    windowMs: MarketRelayPolicy.publishRateLimitWindowMs,
  },
} as const

const arrayLength = (value: unknown): number =>
  Array.isArray(value) ? value.length : 0

const validateArrayLimit = (
  value: unknown,
  limit: number,
  label: string,
): string | null =>
  arrayLength(value) > limit
    ? `${label} exceeds ${limit} entries`
    : null

export const validateReqFilters = (
  filters: ReadonlyArray<ReqFilter>,
): string | null => {
  if (filters.length === 0) {
    return "REQ requires at least one filter"
  }

  if (filters.length > MarketRelayPolicy.maxFiltersPerReq) {
    return `REQ exceeds ${MarketRelayPolicy.maxFiltersPerReq} filters`
  }

  for (const filter of filters) {
    const limit =
      typeof filter.limit === "number" && Number.isFinite(filter.limit)
        ? filter.limit
        : null

    if (limit !== null && limit > MarketRelayPolicy.maxReqLimit) {
      return `REQ filter limit exceeds ${MarketRelayPolicy.maxReqLimit}`
    }

    const limitFailures = [
      validateArrayLimit(filter.ids, MarketRelayPolicy.maxIdsPerFilter, "ids"),
      validateArrayLimit(
        filter.authors,
        MarketRelayPolicy.maxAuthorsPerFilter,
        "authors",
      ),
      validateArrayLimit(
        filter.kinds,
        MarketRelayPolicy.maxKindsPerFilter,
        "kinds",
      ),
      validateArrayLimit(
        filter["#e"],
        MarketRelayPolicy.maxTagValuesPerFilter,
        "#e",
      ),
      validateArrayLimit(
        filter["#p"],
        MarketRelayPolicy.maxTagValuesPerFilter,
        "#p",
      ),
      validateArrayLimit(
        filter["#a"],
        MarketRelayPolicy.maxTagValuesPerFilter,
        "#a",
      ),
      validateArrayLimit(
        filter["#d"],
        MarketRelayPolicy.maxTagValuesPerFilter,
        "#d",
      ),
      validateArrayLimit(
        filter["#t"],
        MarketRelayPolicy.maxTagValuesPerFilter,
        "#t",
      ),
    ].find((failure): failure is string => failure !== null)

    if (limitFailures !== undefined) {
      return limitFailures
    }

    if (Array.isArray(filter.kinds)) {
      // REQ is read-only: both the scoped market kinds and the general
      // coordination/discovery kinds (#5537) are subscribable. The general
      // WRITE gate (allowlist / NIP-42 AUTH) is enforced on EVENT, not REQ.
      const disallowedKind = filter.kinds.find(
        kind =>
          typeof kind === "number" &&
          !isAllowedMarketKind(kind) &&
          !isGeneralCoordinationKind(kind),
      )

      if (typeof disallowedKind === "number") {
        return `kind ${disallowedKind} is outside the OpenAgents relay policy`
      }
    }
  }

  return null
}

export const nextPublishBucket = (
  bucket: PublishBucket | undefined,
  nowMs: number,
): Readonly<{ allowed: boolean; bucket: PublishBucket }> => {
  if (
    bucket === undefined ||
    nowMs - bucket.startedAt >= MarketRelayPolicy.publishRateLimitWindowMs
  ) {
    return { allowed: true, bucket: { count: 1, startedAt: nowMs } }
  }

  const next = { count: bucket.count + 1, startedAt: bucket.startedAt }

  return {
    allowed: next.count <= MarketRelayPolicy.publishRateLimitMaxEvents,
    bucket: next,
  }
}
