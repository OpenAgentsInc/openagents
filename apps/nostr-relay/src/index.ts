import relayWorker, {
  NostrRelayDO as BaseNostrRelayDO,
} from "nostr-effect/relay/backends/cloudflare/worker"
import type { Env as NostrRelayEnv } from "nostr-effect/relay/backends/cloudflare/NostrRelayDO"

import {
  MarketRelayPolicy,
  type PublishBucket,
  isAllowedMarketKind,
  marketKindBucket,
  marketKindPolicySummary,
  nextPublishBucket,
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
  kind?: unknown
  content?: unknown
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

const filtersFromReqMessage = (message: unknown): ReadonlyArray<Record<string, unknown>> | null =>
  Array.isArray(message) && message[0] === "REQ"
    ? message.slice(2).filter(isRecord)
    : null

const kindCountsDefault = () => ({
  nip90_request: 0,
  nip90_result: 0,
  nip90_feedback: 0,
  nip_ds: 0,
  nip89_handler: 0,
})

export class NostrRelayDO extends BaseNostrRelayDO {
  private readonly marketSql: SqlStorage
  private readonly issue: string | null
  private readonly publishBuckets = new Map<string, PublishBucket>()
  private lastRetentionRunMs = 0
  private lastRetentionRunAt: string | null = null
  private lastRetentionDeletedCount = 0

  constructor(state: DurableObjectState, env: Env) {
    super(state, env)
    this.marketSql = state.storage.sql as SqlStorage
    this.issue = env.OPENAGENTS_NOSTR_RELAY_ISSUE ?? null
  }

  override async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url)

    if (url.pathname === "/health" || url.pathname === "/metrics") {
      this.runRetention(Date.now())
      return json(this.metrics())
    }

    return super.fetch(request)
  }

  override async webSocketMessage(
    ws: WebSocket,
    message: string | ArrayBuffer,
  ): Promise<void> {
    const screening = this.screenMessage(message)

    if (screening !== null) {
      ws.send(screening)
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
        `blocked: kind ${event.kind} is outside the OpenAgents scoped market relay policy`,
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

    return relayWorker.fetch(request, env)
  },
}
