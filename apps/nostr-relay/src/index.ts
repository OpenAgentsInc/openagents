import relayWorker, {
  NostrRelayDO as BaseNostrRelayDO,
} from "nostr-effect/relay/backends/cloudflare/worker"
import type { Env as NostrRelayEnv } from "nostr-effect/relay/backends/cloudflare/NostrRelayDO"
import { verifyEvent, type Event as SignedNostrEvent } from "nostr-effect/pure"

import {
  MarketRelayPolicy,
  type PublishBucket,
  isAllowedMarketKind,
  marketKindBucket,
  marketKindPolicySummary,
  nextPublishBucket,
  validateReqFilters,
} from "./market-policy"
import {
  GeneralRelayPolicy,
  type GeneralPublishBucket,
  GENERAL_COORDINATION_KINDS,
  authorizeGeneralWrite,
  generalKindBucket,
  generalKindPolicySummary,
  isGeneralCoordinationKind,
  nextGeneralPublishBucket,
  parseAuthorizedPubkeys,
  validateNip42AuthClaims,
} from "./general-policy"

export interface Env extends NostrRelayEnv {
  OPENAGENTS_NOSTR_RELAY_ISSUE?: string
  /**
   * Comma/whitespace-separated 64-char-hex pubkeys that may write the general
   * coordination/discovery kinds (#5537). Pylon provisions these. Empty means
   * only NIP-42-authenticated connections can write general kinds.
   */
  OPENAGENTS_RELAY_AUTHORIZED_PUBKEYS?: string
}

const json = (value: unknown, init: ResponseInit = {}) =>
  new Response(JSON.stringify(value, null, 2), {
    ...init,
    headers: {
      "content-type": "application/json; charset=utf-8",
      ...init.headers,
    },
  })

type SqlStorage = {
  exec<T = Record<string, unknown>>(
    query: string,
    ...params: ReadonlyArray<unknown>
  ): {
    one(): T | null
    toArray(): Array<T>
    readonly rowsRead: number
    readonly rowsWritten: number
  }
}

type MarketRelayEvent = Readonly<{
  id?: unknown
  pubkey?: unknown
  created_at?: unknown
  kind?: unknown
  tags?: unknown
  content?: unknown
  sig?: unknown
}>

type MarketRelayMetrics = Readonly<{
  ok: true
  relay: string
  backend: string
  issue: string | null
  boundary: string
  policy: typeof marketKindPolicySummary
  generalPolicy: typeof generalKindPolicySummary
  authorizedPubkeyCount: number
  retention: Readonly<{
    deletedExpiredEvents: number
    lastRunAt: string | null
  }>
  events: Readonly<{
    total: number
    oldestCreatedAt: number | null
    newestCreatedAt: number | null
    byKindRange: Readonly<Record<NonNullable<ReturnType<typeof marketKindBucket>>, number>>
    generalCoordinationEvents: number
    byKind: Readonly<Record<string, number>>
  }>
}>

const marketRelayName = "openagents-scoped-market-relay"

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)

const relayNotice = (message: string): string =>
  JSON.stringify(["NOTICE", message])

const relayClosed = (subscriptionId: unknown, message: string): string =>
  JSON.stringify(["CLOSED", typeof subscriptionId === "string" ? subscriptionId : "", message])

const relayOk = (eventId: unknown, ok: boolean, message: string): string =>
  JSON.stringify([
    "OK",
    typeof eventId === "string" ? eventId : "",
    ok,
    message,
  ])

const parseClientMessage = (message: string | ArrayBuffer): unknown => {
  const raw =
    typeof message === "string" ? message : new TextDecoder().decode(message)
  return JSON.parse(raw) as unknown
}

const eventFromMessage = (message: unknown): MarketRelayEvent | null =>
  Array.isArray(message) && message[0] === "EVENT" && isRecord(message[1])
    ? message[1]
    : null

const isSignedNostrEvent = (event: MarketRelayEvent): event is SignedNostrEvent =>
  typeof event.id === "string" &&
  typeof event.pubkey === "string" &&
  typeof event.created_at === "number" &&
  typeof event.kind === "number" &&
  Array.isArray(event.tags) &&
  event.tags.every(
    tag => Array.isArray(tag) && tag.every(value => typeof value === "string"),
  ) &&
  typeof event.content === "string" &&
  typeof event.sig === "string"

const dTagValue = (event: SignedNostrEvent): string | null => {
  const tag = event.tags.find(candidate => candidate[0] === "d")
  return typeof tag?.[1] === "string" && tag[1].length > 0 ? tag[1] : null
}

const filtersFromReqMessage = (message: unknown): ReadonlyArray<Record<string, unknown>> | null =>
  Array.isArray(message) && message[0] === "REQ"
    ? message.slice(2).filter(isRecord)
    : null

// NIP-42 client AUTH: ["AUTH", <signed kind-22242 event>].
const authEventFromMessage = (message: unknown): MarketRelayEvent | null =>
  Array.isArray(message) && message[0] === "AUTH" && isRecord(message[1])
    ? message[1]
    : null

const relayAuth = (challenge: string): string =>
  JSON.stringify(["AUTH", challenge])

// The relay URLs an AUTH event's `relay` tag may legitimately reference. The
// workers.dev host stays valid for compatibility alongside the canonical hosts.
const relayAuthUrls = [
  "wss://relay.openagents.com",
  "wss://nexus.openagents.com",
  "wss://openagents-market-relay.openagents.workers.dev",
] as const

// Honest NIP-11 supported_nips for the expanded relay (#5537):
//   1  text notes / base protocol
//   2  contacts
//   9  event deletion (base relay behavior)
//   11 relay information document
//   17 private direct messages (gift-wrapped)
//   28 public chat
//   38 user statuses
//   42 authentication of clients to relays  (NEWLY ADDED)
//   44 encrypted payloads (gift-wrap seal encryption)
//   59 gift wrap
//   65 relay list metadata
//   89 recommended application handlers (market)
//   90 data vending machines / labor-market jobs (market)
const SUPPORTED_NIPS = [1, 2, 9, 11, 17, 28, 38, 42, 44, 59, 65, 89, 90] as const

const kindCountsDefault = () => ({
  nip90_request: 0,
  nip90_result: 0,
  nip90_feedback: 0,
  nip_ds: 0,
  nip89_handler: 0,
})

const ensureParameterizedReplaceableStorage = (sql: SqlStorage): void => {
  try {
    const columns = sql.exec<{ name: string }>("PRAGMA table_info(events)").toArray()
    const hasDTag = columns.some(column => column.name === "d_tag")
    if (!hasDTag) {
      sql.exec("ALTER TABLE events ADD COLUMN d_tag TEXT")
    }
  } catch {
    try {
      sql.exec("ALTER TABLE events ADD COLUMN d_tag TEXT")
    } catch {
      // The column already exists or the upstream store will report the real
      // storage failure during publish.
    }
  }

  try {
    sql.exec(
      "CREATE INDEX IF NOT EXISTS idx_pubkey_kind_dtag ON events(pubkey, kind, d_tag)",
    )
  } catch {
    // If the events table does not exist yet, nostr-effect creates it during
    // base DO construction. The next object construction will create the index.
  }
}

type ConnectionAttachment = Readonly<{
  challenge: string
  authenticatedPubkey: string | null
  /** The ws/wss URL this connection actually arrived on, for NIP-42 relay-tag matching. */
  relayUrl?: string
}>

// Derive the ws/wss relay URL a request arrived on, so a NIP-42 AUTH event's
// `relay` tag can be matched against the host the client actually connected to
// (in addition to the known production hosts). Strips path/query.
const relayUrlFromRequest = (request: Request): string | null => {
  try {
    const url = new URL(request.url)
    const scheme = url.protocol === "http:" ? "ws:" : "wss:"
    return `${scheme}//${url.host}`
  } catch {
    return null
  }
}

export class NostrRelayDO extends BaseNostrRelayDO {
  private readonly marketSql: SqlStorage
  private readonly issue: string | null
  private readonly authorizedPubkeys: ReadonlySet<string>
  private readonly publishBuckets = new Map<string, PublishBucket>()
  private readonly generalPublishBuckets = new Map<string, GeneralPublishBucket>()
  private lastRetentionRunMs = 0
  private lastRetentionRunAt: string | null = null
  private lastRetentionDeletedCount = 0

  constructor(state: DurableObjectState, env: Env) {
    super(state, env)
    ensureParameterizedReplaceableStorage(state.storage.sql as SqlStorage)
    this.marketSql = state.storage.sql as SqlStorage
    this.issue = env.OPENAGENTS_NOSTR_RELAY_ISSUE ?? null
    this.authorizedPubkeys = parseAuthorizedPubkeys(
      env.OPENAGENTS_RELAY_AUTHORIZED_PUBKEYS,
    )
  }

  override async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url)

    if (url.pathname === "/health" || url.pathname === "/metrics") {
      this.runRetention(Date.now())
      return json(this.metrics())
    }

    // Serve an OpenAgents-authored, honest NIP-11 document for the root info
    // request (#5537). The upstream relay info would not list NIP-42 or the
    // expanded scope; this keeps supported_nips and limitations truthful.
    if (
      request.method === "GET" &&
      url.pathname === "/" &&
      !request.headers.get("upgrade")
    ) {
      const accept = request.headers.get("accept") ?? ""
      if (accept.includes("application/nostr+json")) {
        return new Response(JSON.stringify(this.relayInformationDocument()), {
          headers: {
            "Content-Type": "application/nostr+json",
            "Access-Control-Allow-Origin": "*",
          },
        })
      }
    }

    // Handle the WebSocket upgrade ourselves so we can issue a NIP-42 AUTH
    // challenge and persist it on the connection attachment (survives
    // hibernation). Non-upgrade requests fall through to the base relay.
    if (request.headers.get("upgrade") === "websocket") {
      const pair = new WebSocketPair()
      const client = pair[0]
      const server = pair[1]
      const challenge = crypto.randomUUID()
      const connectionId = `conn_${Date.now()}_${Math.floor(
        Math.random() * 1_000_000,
      )}`

      this.ctxAcceptWebSocket(server, connectionId)
      const relayUrl = relayUrlFromRequest(request)
      const attachment: ConnectionAttachment = {
        challenge,
        authenticatedPubkey: null,
        ...(relayUrl !== null ? { relayUrl } : {}),
      }
      server.serializeAttachment(attachment)
      server.send(relayAuth(challenge))

      return new Response(null, { status: 101, webSocket: client })
    }

    return super.fetch(request)
  }

  private ctxAcceptWebSocket(server: WebSocket, connectionId: string): void {
    // The base DO retrieves the connection id via state.getTags(ws)[0]; keep
    // that contract so super.webSocketMessage continues to work.
    ;(
      this as unknown as {
        state: { acceptWebSocket: (ws: WebSocket, tags: string[]) => void }
      }
    ).state.acceptWebSocket(server, [connectionId])
  }

  private connectionAttachment(ws: WebSocket): ConnectionAttachment {
    const raw = ws.deserializeAttachment() as ConnectionAttachment | null
    if (
      raw !== null &&
      typeof raw === "object" &&
      typeof raw.challenge === "string"
    ) {
      return raw
    }
    // Connections that predate this deploy (or any without an attachment) get a
    // fresh challenge so AUTH still works after a redeploy.
    return { challenge: crypto.randomUUID(), authenticatedPubkey: null }
  }

  override async webSocketMessage(
    ws: WebSocket,
    message: string | ArrayBuffer,
  ): Promise<void> {
    let parsed: unknown
    try {
      parsed = parseClientMessage(message)
    } catch {
      ws.send(relayNotice("invalid: client message must be valid JSON"))
      return
    }

    // NIP-42 AUTH handshake (must run before screening so the result can
    // authorize subsequent general-kind writes on this connection).
    const authEvent = authEventFromMessage(parsed)
    if (authEvent !== null) {
      this.handleAuth(ws, authEvent)
      return
    }

    const screening = this.screenMessage(message)

    if (screening !== null) {
      ws.send(screening)
      return
    }

    const event = eventFromMessage(parsed)

    // Addressable market listings/offers keep their existing dedicated path.
    if (
      event !== null &&
      (event.kind === 30404 || event.kind === 30406) &&
      this.storeAddressableMarketEvent(event, ws)
    ) {
      return
    }

    // General coordination/discovery kinds (#5537): authorized + verified +
    // size/rate-checked, then stored on the shared events table so REQ readers
    // (including the fallback drill's read-back) can fetch them by id.
    if (
      event !== null &&
      typeof event.kind === "number" &&
      isGeneralCoordinationKind(event.kind) &&
      this.storeGeneralCoordinationEvent(event, ws)
    ) {
      return
    }

    this.runRetention(Date.now())
    return super.webSocketMessage(ws, message)
  }

  private handleAuth(ws: WebSocket, event: MarketRelayEvent): void {
    const attachment = this.connectionAttachment(ws)

    const claims = validateNip42AuthClaims({
      event,
      expectedChallenge: attachment.challenge,
      relayUrls: [
        ...relayAuthUrls,
        ...(attachment.relayUrl !== undefined ? [attachment.relayUrl] : []),
      ],
      nowSeconds: Math.floor(Date.now() / 1000),
    })

    if (!claims.ok) {
      ws.send(relayOk(event.id, false, claims.reason))
      return
    }

    if (!isSignedNostrEvent(event) || !verifyEvent(event)) {
      ws.send(relayOk(event.id, false, "auth: signature verification failed"))
      return
    }

    const next: ConnectionAttachment = {
      challenge: attachment.challenge,
      authenticatedPubkey: claims.pubkey,
      ...(attachment.relayUrl !== undefined
        ? { relayUrl: attachment.relayUrl }
        : {}),
    }
    ws.serializeAttachment(next)
    ws.send(relayOk(event.id, true, "auth: ok"))
  }

  private storeGeneralCoordinationEvent(
    event: MarketRelayEvent,
    ws: WebSocket,
  ): boolean {
    if (!isSignedNostrEvent(event)) {
      ws.send(relayOk(event.id, false, "blocked: malformed coordination event"))
      return true
    }

    const attachment = this.connectionAttachment(ws)
    const authorization = authorizeGeneralWrite({
      kind: event.kind,
      pubkey: event.pubkey,
      allowlist: this.authorizedPubkeys,
      authenticatedPubkey: attachment.authenticatedPubkey,
    })

    if (!authorization.allowed) {
      ws.send(relayOk(event.id, false, authorization.reason))
      return true
    }

    if (
      new TextEncoder().encode(event.content).byteLength >
      GeneralRelayPolicy.maxEventContentBytes
    ) {
      ws.send(
        relayOk(event.id, false, "blocked: coordination event exceeds relay limit"),
      )
      return true
    }

    const now = Date.now()
    const current = this.generalPublishBuckets.get(event.pubkey)
    const nextRate = nextGeneralPublishBucket(current, now)
    this.generalPublishBuckets.set(event.pubkey, nextRate.bucket)
    if (!nextRate.allowed) {
      ws.send(
        relayOk(
          event.id,
          false,
          "rate-limited: per-pubkey coordination publish limit exceeded",
        ),
      )
      return true
    }

    if (!verifyEvent(event)) {
      ws.send(relayOk(event.id, false, "invalid: event signature verification failed"))
      return true
    }

    // Authorized + verified: store directly with robust replaceable/addressable
    // handling so REQ read-back-by-id works. We do NOT defer to the base relay's
    // store here: its parameterized-replaceable path uses a SQL cursor `.one()`
    // that throws on the zero-existing-row case (StorageError), so we own the
    // upsert exactly as `storeAddressableMarketEvent` does for market listings.
    this.runRetention(now)
    return this.storeSignedEvent(event, ws)
  }

  /**
   * Store a signed event with the correct Nostr replaceability semantics:
   *   - addressable (30000-39999, incl. NIP-38 30315): unique by pubkey+kind+d
   *   - replaceable (10000-19999 + kind 3, incl. NIP-02 3 / NIP-65 10002):
   *     unique by pubkey+kind
   *   - regular (everything else, incl. NIP-01 1 / NIP-28 / NIP-59 1059): append
   * Sends the appropriate OK frame and returns true (message fully handled).
   */
  private storeSignedEvent(event: SignedNostrEvent, ws: WebSocket): boolean {
    const kind = event.kind
    const isAddressable = kind >= 30000 && kind <= 39999
    const isReplaceable =
      kind === 0 || kind === 3 || (kind >= 10000 && kind <= 19999)

    try {
      ensureParameterizedReplaceableStorage(this.marketSql)

      if (isAddressable || isReplaceable) {
        const dTag = isAddressable ? dTagValue(event) : null
        if (isAddressable && dTag === null) {
          ws.send(relayOk(event.id, false, "blocked: addressable event requires d tag"))
          return true
        }

        const existing = (isAddressable
          ? this.marketSql.exec<{ id: string; created_at: number }>(
              "SELECT id, created_at FROM events WHERE pubkey = ? AND kind = ? AND d_tag = ?",
              event.pubkey,
              kind,
              dTag,
            )
          : this.marketSql.exec<{ id: string; created_at: number }>(
              "SELECT id, created_at FROM events WHERE pubkey = ? AND kind = ?",
              event.pubkey,
              kind,
            )
        ).toArray()[0] ?? null

        if (existing !== null) {
          if (existing.id === event.id) {
            ws.send(relayOk(event.id, true, "duplicate: event already exists"))
            return true
          }
          const shouldReplace =
            event.created_at > existing.created_at ||
            (event.created_at === existing.created_at && event.id < existing.id)
          if (!shouldReplace) {
            ws.send(relayOk(event.id, true, "duplicate: older replaceable event ignored"))
            return true
          }
          this.marketSql.exec("DELETE FROM events WHERE id = ?", existing.id)
        }

        this.marketSql.exec(
          `INSERT INTO events (id, pubkey, created_at, kind, tags, content, sig, d_tag)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
          event.id,
          event.pubkey,
          event.created_at,
          kind,
          JSON.stringify(event.tags),
          event.content,
          event.sig,
          dTag,
        )
        ws.send(relayOk(event.id, true, ""))
        return true
      }

      // Regular event: append (idempotent on id).
      const already = this.marketSql
        .exec<{ id: string }>("SELECT id FROM events WHERE id = ?", event.id)
        .toArray()[0] ?? null
      if (already !== null) {
        ws.send(relayOk(event.id, true, "duplicate: event already exists"))
        return true
      }
      this.marketSql.exec(
        `INSERT INTO events (id, pubkey, created_at, kind, tags, content, sig, d_tag)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
        event.id,
        event.pubkey,
        event.created_at,
        kind,
        JSON.stringify(event.tags),
        event.content,
        event.sig,
        null,
      )
      ws.send(relayOk(event.id, true, ""))
      return true
    } catch (error) {
      ws.send(
        relayOk(
          event.id,
          false,
          `error: coordination storage failed: ${(error as Error).message}`,
        ),
      )
      return true
    }
  }

  private screenMessage(message: string | ArrayBuffer): string | null {
    let parsed: unknown

    try {
      parsed = parseClientMessage(message)
    } catch {
      return relayNotice("invalid: client message must be valid JSON")
    }

    const event = eventFromMessage(parsed)

    if (event !== null) {
      return this.screenEvent(event)
    }

    const filters = filtersFromReqMessage(parsed)

    if (filters !== null) {
      const failure = validateReqFilters(filters)
      return failure === null ? null : relayClosed((parsed as Array<unknown>)[1], failure)
    }

    return null
  }

  private screenEvent(event: MarketRelayEvent): string | null {
    if (typeof event.kind !== "number" || !Number.isInteger(event.kind)) {
      return relayOk(event.id, false, "blocked: event kind must be an integer")
    }

    // General coordination/discovery kinds (#5537) are screened in their own
    // authorized path (storeGeneralCoordinationEvent); let them pass here.
    if (isGeneralCoordinationKind(event.kind)) {
      return null
    }

    if (!isAllowedMarketKind(event.kind)) {
      return relayOk(
        event.id,
        false,
        `blocked: kind ${event.kind} is outside the OpenAgents relay policy`,
      )
    }

    if (typeof event.pubkey !== "string") {
      return relayOk(event.id, false, "blocked: event pubkey is required")
    }

    if (
      typeof event.content === "string" &&
      new TextEncoder().encode(event.content).byteLength >
        MarketRelayPolicy.maxEventContentBytes
    ) {
      return relayOk(event.id, false, "blocked: event content exceeds relay limit")
    }

    const now = Date.now()
    const current = this.publishBuckets.get(event.pubkey)
    const next = nextBucket(current, now)
    this.publishBuckets.set(event.pubkey, next.bucket)

    return next.allowed
      ? null
      : relayOk(event.id, false, "rate-limited: per-pubkey publish limit exceeded")
  }

  private storeAddressableMarketEvent(
    event: MarketRelayEvent,
    ws: WebSocket,
  ): boolean {
    if (!isSignedNostrEvent(event)) {
      ws.send(relayOk(event.id, false, "blocked: malformed addressable market event"))
      return true
    }

    if (!verifyEvent(event)) {
      ws.send(relayOk(event.id, false, "invalid: event signature verification failed"))
      return true
    }

    const dTag = dTagValue(event)
    if (dTag === null) {
      ws.send(relayOk(event.id, false, "blocked: addressable market event requires d tag"))
      return true
    }

    try {
      ensureParameterizedReplaceableStorage(this.marketSql)
      const existing = this.marketSql
        .exec<{ id: string; created_at: number }>(
          "SELECT id, created_at FROM events WHERE pubkey = ? AND kind = ? AND d_tag = ?",
          event.pubkey,
          event.kind,
          dTag,
        )
        .toArray()[0] ?? null

      if (existing !== null) {
        if (existing.id === event.id) {
          ws.send(relayOk(event.id, true, "duplicate: event already exists"))
          return true
        }

        const shouldReplace =
          event.created_at > existing.created_at ||
          (event.created_at === existing.created_at && event.id < existing.id)

        if (!shouldReplace) {
          ws.send(relayOk(event.id, true, "duplicate: older addressable event ignored"))
          return true
        }

        this.marketSql.exec("DELETE FROM events WHERE id = ?", existing.id)
      }

      this.marketSql.exec(
        `INSERT INTO events (id, pubkey, created_at, kind, tags, content, sig, d_tag)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
        event.id,
        event.pubkey,
        event.created_at,
        event.kind,
        JSON.stringify(event.tags),
        event.content,
        event.sig,
        dTag,
      )
      ws.send(relayOk(event.id, true, ""))
      return true
    } catch (error) {
      ws.send(
        relayOk(
          event.id,
          false,
          `error: addressable market storage failed: ${(error as Error).message}`,
        ),
      )
      return true
    }
  }

  private runRetention(nowMs: number): void {
    if (nowMs - this.lastRetentionRunMs < 60_000) {
      return
    }

    this.lastRetentionRunMs = nowMs
    this.lastRetentionRunAt = new Date(nowMs).toISOString()

    const nowSeconds = Math.floor(nowMs / 1000)
    const eventCutoff = nowSeconds - MarketRelayPolicy.eventRetentionSeconds
    const handlerCutoff = nowSeconds - MarketRelayPolicy.handlerRetentionSeconds
    const generalCutoff = nowSeconds - GeneralRelayPolicy.eventRetentionSeconds
    const generalKindList = GENERAL_COORDINATION_KINDS.join(", ")

    const marketDelete = this.marketSql.exec(
      `DELETE FROM events
       WHERE created_at < ?
         AND kind NOT IN (31989, 31990)
         AND kind NOT IN (${generalKindList})`,
      eventCutoff,
    )
    const handlerDelete = this.marketSql.exec(
      `DELETE FROM events
       WHERE created_at < ?
         AND kind IN (31989, 31990)`,
      handlerCutoff,
    )
    // General coordination/discovery events (#5537) expire faster than market
    // events; they are ephemeral liveness/discovery signals.
    const generalDelete = this.marketSql.exec(
      `DELETE FROM events
       WHERE created_at < ?
         AND kind IN (${generalKindList})`,
      generalCutoff,
    )

    this.lastRetentionDeletedCount =
      marketDelete.rowsWritten +
      handlerDelete.rowsWritten +
      generalDelete.rowsWritten
  }

  private metrics(): MarketRelayMetrics {
    const rows = this.marketSql
      .exec<{ kind: number; count: number }>(
        `SELECT kind, COUNT(*) AS count
         FROM events
         GROUP BY kind
         ORDER BY kind ASC`,
      )
      .toArray()
    const totals = this.marketSql
      .exec<{
        total: number
        oldest_created_at: number | null
        newest_created_at: number | null
      }>(
        `SELECT COUNT(*) AS total,
                MIN(created_at) AS oldest_created_at,
                MAX(created_at) AS newest_created_at
         FROM events`,
      )
      .one()
    const byKindRange = kindCountsDefault()
    const byKind: Record<string, number> = {}
    let generalCoordinationEvents = 0

    for (const row of rows) {
      byKind[String(row.kind)] = row.count
      const bucket = marketKindBucket(row.kind)
      if (bucket !== null) {
        byKindRange[bucket] += row.count
      }
      if (generalKindBucket(row.kind) !== null) {
        generalCoordinationEvents += row.count
      }
    }

    return {
      ok: true,
      relay: marketRelayName,
      backend: "cloudflare-durable-object",
      issue: this.issue,
      boundary:
        "event_transport_only_no_payment_identity_moderation_assignment_or_settlement_authority",
      policy: marketKindPolicySummary,
      generalPolicy: generalKindPolicySummary,
      authorizedPubkeyCount: this.authorizedPubkeys.size,
      retention: {
        deletedExpiredEvents: this.lastRetentionDeletedCount,
        lastRunAt: this.lastRetentionRunAt,
      },
      events: {
        total: totals?.total ?? 0,
        oldestCreatedAt: totals?.oldest_created_at ?? null,
        newestCreatedAt: totals?.newest_created_at ?? null,
        byKindRange,
        generalCoordinationEvents,
        byKind,
      },
    }
  }

  /**
   * Honest NIP-11 relay information document (#5537). Reflects the expanded
   * supported_nips (incl. 42), the scope (market + gated general coordination),
   * and the anti-abuse posture in the `limitation` block.
   */
  private relayInformationDocument(): Record<string, unknown> {
    return {
      name: "OpenAgents Relay",
      description:
        "OpenAgents relay. Open-write market rails (NIP-90 jobs, NIP-DS dataset " +
        "listings, NIP-89 handlers) plus general coordination/discovery kinds " +
        "(NIP-01/02/17/28/38/65) that are write-gated by a provisioned-pubkey " +
        "allowlist or NIP-42 AUTH. Event transport only: no payment, identity, " +
        "moderation, assignment, or settlement authority.",
      contact: "https://github.com/OpenAgentsInc/openagents/issues/4636",
      software:
        "https://github.com/OpenAgentsInc/openagents/tree/main/apps/nostr-relay",
      supported_nips: [...SUPPORTED_NIPS],
      limitation: {
        max_message_length: MarketRelayPolicy.maxEventContentBytes,
        max_subscriptions: MarketRelayPolicy.maxFiltersPerReq,
        max_filters: MarketRelayPolicy.maxFiltersPerReq,
        max_limit: MarketRelayPolicy.maxReqLimit,
        // AUTH is supported but only REQUIRED for the general coordination kinds;
        // the market kinds remain open-write so the public job bus is unaffected.
        auth_required: false,
        payment_required: false,
        restricted_writes: true,
      },
      relay_policy: {
        market_kinds: marketKindPolicySummary.allowedKinds,
        general_coordination_kinds: generalKindPolicySummary.allowedKinds,
        general_write_gate: generalKindPolicySummary.writeGate,
        anti_abuse:
          "General coordination/discovery kinds require an allowlisted pubkey " +
          "or NIP-42 AUTH, plus per-pubkey rate limits and a per-event size cap. " +
          "Market kinds keep their existing open-write rate-limited path.",
      },
    }
  }
}

const nextBucket = nextPublishBucket

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url)

    if (url.pathname === "/health" || url.pathname === "/metrics") {
      const id = env.NOSTR_RELAY.idFromName("global")
      const stub = env.NOSTR_RELAY.get(id)
      return stub.fetch(request)
    }

    return relayWorker.fetch(request, env)
  },
}
