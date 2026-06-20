export type MarketKindBucket =
  | "nip01_text_note"
  | "nip02_contacts"
  | "nip17_private_dm"
  | "nip38_status"
  | "nip65_relay_list"
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

const privateDirectMessageKinds = [13, 14, 1059] as const

const isPrivateDirectMessageKind = (kind: number): boolean =>
  privateDirectMessageKinds.some(candidate => candidate === kind)

export const marketKindBucket = (kind: number): MarketKindBucket | null => {
  if (kind === 1) {
    return "nip01_text_note"
  }

  if (kind === 3) {
    return "nip02_contacts"
  }

  if (isPrivateDirectMessageKind(kind)) {
    return "nip17_private_dm"
  }

  if (kind === 10002) {
    return "nip65_relay_list"
  }

  if (kind === 30315) {
    return "nip38_status"
  }

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

export const isParameterizedReplaceableMarketKind = (kind: number): boolean =>
  kind >= 30000 && kind <= 39999 && isAllowedMarketKind(kind)

export const relaySupportedNips = [
  1, 2, 11, 17, 38, 44, 59, 65, 89, 90,
] as const

export const marketKindPolicySummary = {
  allowedKinds: {
    nip01TextNotes: [1],
    nip02Contacts: [3],
    nip17PrivateDirectMessages: privateDirectMessageKinds,
    nip38UserStatuses: [30315],
    nip65RelayLists: [10002],
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
  antiAbuse: {
    perPubkeyPublishLimit: true,
    maxEventContentBytes: MarketRelayPolicy.maxEventContentBytes,
    boundedReqFilters: true,
    authority:
      "event_transport_only_no_payment_identity_moderation_assignment_or_settlement_authority",
  },
} as const

export const relayInformationDocument = {
  name: "OpenAgents Market and Coordination Relay",
  description:
    "OpenAgents-owned Nostr relay for scoped market events and OpenAgents coordination/discovery traffic. Event transport only; no payment, identity, moderation, assignment, payout, or settlement authority.",
  pubkey: "",
  contact: "https://openagents.com",
  supported_nips: relaySupportedNips,
  software:
    "https://github.com/OpenAgentsInc/openagents/tree/main/apps/nostr-relay",
  version: "0.1.0",
  limitation: {
    max_content_length: MarketRelayPolicy.maxEventContentBytes,
    max_filters: MarketRelayPolicy.maxFiltersPerReq,
    max_limit: MarketRelayPolicy.maxReqLimit,
    auth_required: false,
    payment_required: false,
    restricted_writes: true,
  },
  retention: [
    { kinds: [31989, 31990], time: MarketRelayPolicy.handlerRetentionSeconds },
    {
      kinds: [
        1,
        3,
        13,
        14,
        1059,
        7000,
        10002,
        30315,
        30404,
        30406,
        [5000, 5999],
        [6000, 6999],
      ],
      time: MarketRelayPolicy.eventRetentionSeconds,
    },
  ],
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
      const disallowedKind = filter.kinds.find(
        kind => typeof kind === "number" && !isAllowedMarketKind(kind),
      )

      if (typeof disallowedKind === "number") {
        return `kind ${disallowedKind} is outside the OpenAgents scoped market and coordination relay policy`
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
