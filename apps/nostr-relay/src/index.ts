import relayWorker, {
  NostrRelayDO as BaseNostrRelayDO,
} from "nostr-effect/relay/backends/cloudflare/worker"
import type { Env as NostrRelayEnv } from "nostr-effect/relay/backends/cloudflare/NostrRelayDO"
import { verifyEvent, type Event as SignedNostrEvent } from "nostr-effect/pure"

import {
  MarketRelayPolicy,
  type PublishBucket,
  isAllowedMarketKind,
  isParameterizedReplaceableMarketKind,
  marketKindBucket,
  marketKindPolicySummary,
  nextPublishBucket,
  relayInformationDocument,
  validateReqFilters,
} from "./market-policy"

export interface Env extends NostrRelayEnv {
  OPENAGENTS_NOSTR_RELAY_ISSUE?: string
}

const json = (value: unknown, init: ResponseInit = {}) =>
  new Response(JSON.stringify(value, null, 2), {
    ...init,
    headers: {
      "content-type": "application/json; charset=utf-8",
      ...init.headers,
    },
  })

const relayInfoResponse = () =>
  json(relayInformationDocument, {
    headers: { "content-type": "application/nostr+json; charset=utf-8" },
  })

const isRelayInfoRequest = (request: Request, url: URL): boolean =>
  request.method === "GET" &&
  url.pathname === "/" &&
  (request.headers.get("accept") ?? "").includes("application/nostr+json")

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
  retention: Readonly<{
    deletedExpiredEvents: number
    lastRunAt: string | null
  }>
  events: Readonly<{
    total: number
    oldestCreatedAt: number | null
    newestCreatedAt: number | null
    byKindRange: Readonly<Record<NonNullable<ReturnType<typeof marketKindBucket>>, number>>
    byKind: Readonly<Record<string, number>>
  }>
}>

const marketRelayName = "openagents-market-and-coordination-relay"

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

const kindCountsDefault = () => ({
  nip01_text_note: 0,
  nip02_contacts: 0,
  nip17_private_dm: 0,
  nip38_status: 0,
  nip65_relay_list: 0,
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

export class NostrRelayDO extends BaseNostrRelayDO {
  private readonly marketSql: SqlStorage
  private readonly issue: string | null
  private readonly publishBuckets = new Map<string, PublishBucket>()
  private lastRetentionRunMs = 0
  private lastRetentionRunAt: string | null = null
  private lastRetentionDeletedCount = 0

  constructor(state: DurableObjectState, env: Env) {
    super(state, env)
    ensureParameterizedReplaceableStorage(state.storage.sql as SqlStorage)
    this.marketSql = state.storage.sql as SqlStorage
    this.issue = env.OPENAGENTS_NOSTR_RELAY_ISSUE ?? null
  }

  override async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url)

    if (url.pathname === "/health" || url.pathname === "/metrics") {
      this.runRetention(Date.now())
      return json(this.metrics())
    }

    if (isRelayInfoRequest(request, url)) {
      return relayInfoResponse()
    }

    return super.fetch(request)
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

    const screening = this.screenMessage(message)

    if (screening !== null) {
      ws.send(screening)
      return
    }

    const event = eventFromMessage(parsed)
    if (
      event !== null &&
      typeof event.kind === "number" &&
      isParameterizedReplaceableMarketKind(event.kind) &&
      this.storeParameterizedReplaceableMarketEvent(event, ws)
    ) {
      return
    }

    this.runRetention(Date.now())
    return super.webSocketMessage(ws, message)
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

    if (!isAllowedMarketKind(event.kind)) {
      return relayOk(
        event.id,
        false,
        `blocked: kind ${event.kind} is outside the OpenAgents scoped market and coordination relay policy`,
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

  private storeParameterizedReplaceableMarketEvent(
    event: MarketRelayEvent,
    ws: WebSocket,
  ): boolean {
    if (!isSignedNostrEvent(event)) {
      ws.send(
        relayOk(event.id, false, "blocked: malformed parameterized replaceable event"),
      )
      return true
    }

    if (!verifyEvent(event)) {
      ws.send(relayOk(event.id, false, "invalid: event signature verification failed"))
      return true
    }

    const dTag = dTagValue(event)
    if (dTag === null) {
      ws.send(
        relayOk(
          event.id,
          false,
          "blocked: parameterized replaceable event requires d tag",
        ),
      )
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
          `error: parameterized replaceable storage failed: ${(error as Error).message}`,
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

    const eventCutoff = Math.floor(nowMs / 1000) - MarketRelayPolicy.eventRetentionSeconds
    const handlerCutoff =
      Math.floor(nowMs / 1000) - MarketRelayPolicy.handlerRetentionSeconds

    const marketDelete = this.marketSql.exec(
      `DELETE FROM events
       WHERE created_at < ?
         AND kind NOT IN (31989, 31990)`,
      eventCutoff,
    )
    const handlerDelete = this.marketSql.exec(
      `DELETE FROM events
       WHERE created_at < ?
         AND kind IN (31989, 31990)`,
      handlerCutoff,
    )

    this.lastRetentionDeletedCount =
      marketDelete.rowsWritten + handlerDelete.rowsWritten
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

    for (const row of rows) {
      byKind[String(row.kind)] = row.count
      const bucket = marketKindBucket(row.kind)
      if (bucket !== null) {
        byKindRange[bucket] += row.count
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
      retention: {
        deletedExpiredEvents: this.lastRetentionDeletedCount,
        lastRunAt: this.lastRetentionRunAt,
      },
      events: {
        total: totals?.total ?? 0,
        oldestCreatedAt: totals?.oldest_created_at ?? null,
        newestCreatedAt: totals?.newest_created_at ?? null,
        byKindRange,
        byKind,
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

    if (isRelayInfoRequest(request, url)) {
      return relayInfoResponse()
    }

    return relayWorker.fetch(request, env)
  },
}
