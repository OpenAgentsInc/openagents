/**
 * General coordination/discovery policy for the OpenAgents relay (#5537).
 *
 * This is an ADDITION to the scoped market policy in `market-policy.ts`, not a
 * replacement. The market kinds (NIP-90 5000-7000, NIP-DS 30404/30406, NIP-89
 * 31989/31990) keep their existing open-write path so the public labor/compute
 * market bus other agents use is unaffected.
 *
 * The general kinds below (text notes, contacts, statuses, relay lists,
 * gift-wrapped DMs, public-chat) let OpenAgents agents use the OWNED relay for
 * outage coordination/discovery (the `agents.nostr_fallback_coordination.v1`
 * fallback path) instead of relying only on public relays.
 *
 * Because widening the kind allowlist also widens the DoS surface, these general
 * kinds are write-GATED. A write is authorized only when EITHER:
 *   1. the event pubkey is on the provisioned allowlist (Pylon provisions Nostr
 *      creds for OpenAgents agents), OR
 *   2. the connection has completed NIP-42 AUTH for that pubkey.
 * Unauthorized general-kind writes are rejected. Per-pubkey rate limits and an
 * event size cap apply on top, so an authorized-but-misbehaving key is still
 * bounded. This module is pure and fully unit-testable; the wiring lives in
 * `index.ts`.
 */

export type GeneralKindBucket =
  | "nip01_text_note"
  | "nip02_contacts"
  | "nip38_status"
  | "nip65_relay_list"
  | "nip17_seal"
  | "nip17_rumor"
  | "nip59_gift_wrap"
  | "nip28_channel"

export const GeneralRelayPolicy = {
  /**
   * General-kind events are smaller coordination/discovery signals than market
   * payloads; a tighter content cap keeps the widened surface cheap. Gift wraps
   * carry NIP-44 ciphertext (~1.4 KiB for a short DM) so the cap leaves ample
   * headroom while staying well under the 64 KiB market cap.
   */
  maxEventContentBytes: 32 * 1024,
  /**
   * Per authorized pubkey per Durable Object instance. Lower than the market
   * publish budget because coordination/discovery traffic is bursty-but-small
   * and we never want a single authorized key to flood the owned relay.
   */
  publishRateLimitMaxEvents: 12,
  publishRateLimitWindowMs: 60_000,
  /**
   * General coordination events are ephemeral by nature; retain them for a week.
   * Market retention is unchanged (30d / 180d for handlers).
   */
  eventRetentionSeconds: 60 * 60 * 24 * 7,
} as const

/** NIP-42 client AUTH event kind (the AUTH message payload, not a stored kind). */
export const NIP42_CLIENT_AUTH_KIND = 22242

export const generalKindBucket = (kind: number): GeneralKindBucket | null => {
  switch (kind) {
    case 1:
      return "nip01_text_note"
    case 3:
      return "nip02_contacts"
    case 30315:
      return "nip38_status"
    case 10002:
      return "nip65_relay_list"
    case 13:
      return "nip17_seal"
    case 14:
      return "nip17_rumor"
    case 1059:
      return "nip59_gift_wrap"
    default:
      // NIP-28 public chat (channel create/metadata/message/hide/mute).
      if (kind >= 40 && kind <= 44) {
        return "nip28_channel"
      }
      return null
  }
}

export const isGeneralCoordinationKind = (kind: number): boolean =>
  generalKindBucket(kind) !== null

/** Explicit enumeration of every general coordination kind, for SQL filters. */
export const GENERAL_COORDINATION_KINDS: ReadonlyArray<number> = [
  1, 3, 13, 14, 40, 41, 42, 43, 44, 1059, 10002, 30315,
]

/**
 * Parse the comma/whitespace-separated authorized-pubkey allowlist from config.
 * Pubkeys are normalized to lowercase 64-char hex. Anything malformed is
 * dropped rather than silently widening the gate.
 */
export const parseAuthorizedPubkeys = (
  raw: string | undefined | null,
): ReadonlySet<string> => {
  if (typeof raw !== "string" || raw.length === 0) {
    return new Set<string>()
  }
  const out = new Set<string>()
  for (const token of raw.split(/[\s,]+/)) {
    const candidate = token.trim().toLowerCase()
    if (/^[0-9a-f]{64}$/.test(candidate)) {
      out.add(candidate)
    }
  }
  return out
}

export type GeneralWriteAuthorization = Readonly<{
  /** allowed = true means the general-kind write may proceed to storage. */
  allowed: boolean
  /** machine-readable reason; "" when allowed. */
  reason: string
}>

/**
 * Kinds whose on-wire `pubkey` is an intentionally one-time ephemeral key and
 * therefore can never be allowlisted or matched to an AUTH identity. NIP-59 gift
 * wraps (kind 1059) re-sign with a throwaway key to hide sender metadata. For
 * these, we still require an AUTHENTICATED connection (so an anonymous flood is
 * blocked), but we do not require the event pubkey to equal the auth identity.
 */
export const isEphemeralSenderKind = (kind: number): boolean => kind === 1059

/**
 * Decide whether a general-kind write is authorized. A write is allowed when the
 * pubkey is on the provisioned allowlist OR the connection has authenticated
 * that pubkey via NIP-42. Gift wraps (ephemeral-sender kinds) are allowed on any
 * authenticated connection because their wire pubkey is throwaway by design.
 * Market kinds never reach this function.
 */
export const authorizeGeneralWrite = (input: {
  readonly kind: number
  readonly pubkey: string
  readonly allowlist: ReadonlySet<string>
  readonly authenticatedPubkey: string | null
}): GeneralWriteAuthorization => {
  const pubkey = input.pubkey.toLowerCase()

  if (input.allowlist.has(pubkey)) {
    return { allowed: true, reason: "" }
  }

  if (input.authenticatedPubkey !== null) {
    // Gift wraps: any authenticated connection may relay them (ephemeral wire
    // pubkey). Other general kinds: the event pubkey must be the auth identity.
    if (
      isEphemeralSenderKind(input.kind) ||
      input.authenticatedPubkey.toLowerCase() === pubkey
    ) {
      return { allowed: true, reason: "" }
    }
  }

  return {
    allowed: false,
    reason:
      "auth-required: general coordination kinds require an allowlisted pubkey or NIP-42 AUTH",
  }
}

export type Nip42AuthEvent = Readonly<{
  id?: unknown
  pubkey?: unknown
  created_at?: unknown
  kind?: unknown
  tags?: unknown
  content?: unknown
  sig?: unknown
}>

export type Nip42VerifyResult =
  | Readonly<{ ok: true; pubkey: string }>
  | Readonly<{ ok: false; reason: string }>

/**
 * Validate the structural NIP-42 claims of a client AUTH event against the
 * challenge this connection was issued and the relay's own URL. Signature
 * verification is performed by the caller (it owns the crypto dependency); this
 * function checks everything else so it stays pure and testable.
 *
 * Per NIP-42 the AUTH event (kind 22242) must carry a `relay` tag matching this
 * relay and a `challenge` tag matching the value the relay sent. It must be
 * recent (the relay rejects stale auth to bound replay).
 */
export const validateNip42AuthClaims = (input: {
  readonly event: Nip42AuthEvent
  readonly expectedChallenge: string
  readonly relayUrls: ReadonlyArray<string>
  readonly nowSeconds: number
  readonly maxAgeSeconds?: number
}): Nip42VerifyResult => {
  const event = input.event

  if (event.kind !== NIP42_CLIENT_AUTH_KIND) {
    return { ok: false, reason: "auth: event kind must be 22242" }
  }
  if (typeof event.pubkey !== "string" || !/^[0-9a-f]{64}$/i.test(event.pubkey)) {
    return { ok: false, reason: "auth: event pubkey is required" }
  }
  if (!Array.isArray(event.tags)) {
    return { ok: false, reason: "auth: event tags are required" }
  }

  const tagValue = (name: string): string | null => {
    const tag = (event.tags as Array<unknown>).find(
      (candidate): candidate is Array<string> =>
        Array.isArray(candidate) &&
        candidate[0] === name &&
        typeof candidate[1] === "string",
    )
    return tag ? tag[1] : null
  }

  const challenge = tagValue("challenge")
  if (challenge === null || challenge !== input.expectedChallenge) {
    return { ok: false, reason: "auth: challenge mismatch" }
  }

  const relay = tagValue("relay")
  if (relay === null) {
    return { ok: false, reason: "auth: relay tag is required" }
  }
  // Match by host, ignoring ws/wss scheme and trailing slash: the same relay is
  // reachable over both ws (local) and wss (production), and over multiple
  // hostnames (custom domains, workers.dev). A relay-tag is acceptable when its
  // host matches any host this relay answers on.
  const hostOf = (value: string): string =>
    value
      .trim()
      .toLowerCase()
      .replace(/^wss?:\/\//, "")
      .replace(/\/.*$/, "")
  const relayHost = hostOf(relay)
  const relayMatch = input.relayUrls.map(hostOf).includes(relayHost)
  if (!relayMatch) {
    return { ok: false, reason: "auth: relay tag does not match this relay" }
  }

  const maxAge = input.maxAgeSeconds ?? 600
  if (
    typeof event.created_at !== "number" ||
    Math.abs(input.nowSeconds - event.created_at) > maxAge
  ) {
    return { ok: false, reason: "auth: event is stale" }
  }

  return { ok: true, pubkey: (event.pubkey as string).toLowerCase() }
}

export type GeneralPublishBucket = Readonly<{
  count: number
  startedAt: number
}>

/**
 * Per-pubkey fixed-window rate limiter for the general coordination kinds.
 * Mirrors the market `nextPublishBucket` but uses the tighter general budget.
 */
export const nextGeneralPublishBucket = (
  bucket: GeneralPublishBucket | undefined,
  nowMs: number,
): Readonly<{ allowed: boolean; bucket: GeneralPublishBucket }> => {
  if (
    bucket === undefined ||
    nowMs - bucket.startedAt >= GeneralRelayPolicy.publishRateLimitWindowMs
  ) {
    return { allowed: true, bucket: { count: 1, startedAt: nowMs } }
  }

  const next = { count: bucket.count + 1, startedAt: bucket.startedAt }

  return {
    allowed: next.count <= GeneralRelayPolicy.publishRateLimitMaxEvents,
    bucket: next,
  }
}

export const generalKindPolicySummary = {
  description:
    "General coordination/discovery kinds, write-gated by provisioned allowlist or NIP-42 AUTH.",
  allowedKinds: {
    nip01TextNote: [1],
    nip02Contacts: [3],
    nip17Seal: [13],
    nip17Rumor: [14],
    nip28PublicChat: "40-44",
    nip59GiftWrap: [1059],
    nip65RelayList: [10002],
    nip38UserStatus: [30315],
  },
  writeGate:
    "allowlisted_pubkey_or_nip42_auth_required (market kinds remain open-write)",
  limits: {
    maxEventContentBytes: GeneralRelayPolicy.maxEventContentBytes,
    publishRateLimitMaxEvents: GeneralRelayPolicy.publishRateLimitMaxEvents,
    publishRateLimitWindowMs: GeneralRelayPolicy.publishRateLimitWindowMs,
  },
  retention: {
    generalEventsSeconds: GeneralRelayPolicy.eventRetentionSeconds,
  },
} as const
