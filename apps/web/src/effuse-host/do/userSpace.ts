import { ConvexHttpClient } from "convex/browser"

import { api } from "../../../convex/_generated/api"
import type { WorkerEnv } from "../env"

type AgentRow = {
  readonly id: string
  readonly json: string
  readonly updated_at: number
}

type EventRow = {
  readonly seq: number
  readonly event_id: string
  readonly kind: string
  readonly json: string
  readonly created_at: number
}

const json = (body: unknown, init?: ResponseInit): Response =>
  new Response(JSON.stringify(body), {
    ...init,
    headers: {
      "content-type": "application/json; charset=utf-8",
      ...(init?.headers ?? {}),
    },
  })

const readJson = async (request: Request): Promise<unknown> => {
  try {
    return await request.json()
  } catch {
    return null
  }
}

const bearerToken = (request: Request): string | null => {
  const header = request.headers.get("authorization") ?? ""
  if (!header.toLowerCase().startsWith("bearer ")) return null
  return header.slice("bearer ".length).trim() || null
}

/**
 * Durable Object: canonical "user-space" store (DO SQLite).
 *
 * v1 surface (intentionally small):
 * - agents table (JSON blobs)
 * - append-only events table (seq + eventId) for Convex replication
 */
export class UserSpaceDO {
  constructor(
    private readonly state: DurableObjectState,
    private readonly env: WorkerEnv,
  ) {
    // Tables are created once per object lifetime.
    const sql = this.state.storage.sql
    sql.exec(
      `create table if not exists agents (
        id text primary key,
        json text not null,
        updated_at integer not null
      )`,
    )
    sql.exec(
      `create table if not exists events (
        seq integer primary key autoincrement,
        event_id text not null unique,
        kind text not null,
        json text not null,
        created_at integer not null
      )`,
    )
  }

  private insertEvent(
    userSpaceId: string,
    token: string | null,
    kind: string,
    payload: unknown,
  ): EventRow {
    const now = Date.now()
    const eventId = crypto.randomUUID()
    const jsonPayload = JSON.stringify(payload ?? null)

    const sql = this.state.storage.sql
    sql.exec(
      "insert into events (event_id, kind, json, created_at) values (?, ?, ?, ?)",
      eventId,
      kind,
      jsonPayload,
      now,
    )

    const seqRow = sql.exec<{ seq: number }>("select last_insert_rowid() as seq").one()
    const seq = Number(seqRow.seq)

    const row: EventRow = {
      seq,
      event_id: eventId,
      kind,
      json: jsonPayload,
      created_at: now,
    }

    // Best-effort replication (non-blocking).
    const convexUrl = this.env.VITE_CONVEX_URL ?? process.env.VITE_CONVEX_URL
    if (convexUrl && token) {
      this.state.waitUntil(this.replicateToConvex(convexUrl, token, userSpaceId, [row]))
    }

    return row
  }

  private async replicateToConvex(
    convexUrl: string,
    token: string,
    userSpaceId: string,
    rows: ReadonlyArray<EventRow>,
  ): Promise<void> {
    try {
      const client = new ConvexHttpClient(convexUrl, { logger: false })
      client.setAuth(token)

      const events = rows.map((r) => ({
        eventId: r.event_id,
        seq: r.seq,
        kind: r.kind,
        json: r.json,
        createdAtMs: r.created_at,
      }))

      await client.mutation(api.userSpace.replicateEvents.replicateEvents, {
        userSpaceId,
        events,
      } as any)
    } catch (err) {
      console.warn("[UserSpaceDO] Convex replication failed", err)
    }
  }

  async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url)
    const userSpaceId = request.headers.get("x-user-id") ?? null
    if (!userSpaceId) {
      return json({ ok: false, error: "missing_user" }, { status: 401 })
    }

    const token = bearerToken(request)

    const sql = this.state.storage.sql

    // GET /api/user-space/agents
    if (url.pathname.endsWith("/agents") && request.method === "GET") {
      const rows = sql
        .exec<AgentRow>("select id, json, updated_at from agents order by updated_at desc")
        .toArray()
      const agents = rows.map((r) => ({
        id: r.id,
        json: r.json,
        updatedAtMs: r.updated_at,
      }))
      return json({ ok: true, agents })
    }

    // POST /api/user-space/agents { json: ... }
    if (url.pathname.endsWith("/agents") && request.method === "POST") {
      const body = (await readJson(request)) as any
      const agentJson = body?.json ?? body ?? null
      const id = crypto.randomUUID()
      const updatedAt = Date.now()

      sql.exec(
        "insert into agents (id, json, updated_at) values (?, ?, ?)",
        id,
        JSON.stringify(agentJson),
        updatedAt,
      )

      const evt = this.insertEvent(userSpaceId, token, "agent.created", { agentId: id })
      return json({ ok: true, agentId: id, event: { eventId: evt.event_id, seq: evt.seq } })
    }

    // POST /api/user-space/events { eventId, kind, json, createdAtMs }
    // Idempotent by eventId to support replay / replication without duplicates.
    if (url.pathname.endsWith("/events") && request.method === "POST") {
      const body = (await readJson(request)) as any
      const eventId = typeof body?.eventId === "string" ? body.eventId : ""
      const kind = typeof body?.kind === "string" ? body.kind : ""
      const jsonPayload =
        typeof body?.json === "string"
          ? body.json
          : JSON.stringify(body?.payload ?? null)

      const createdAtMsRaw = body?.createdAtMs
      const createdAtMs =
        typeof createdAtMsRaw === "number" && Number.isFinite(createdAtMsRaw)
          ? createdAtMsRaw
          : Date.now()

      if (!eventId || !kind) {
        return json({ ok: false, error: "invalid_event" }, { status: 400 })
      }

      const existing = sql
        .exec<EventRow>(
          "select seq, event_id, kind, json, created_at from events where event_id = ?",
          eventId,
        )
        .toArray()

      if (existing.length > 0) {
        const row = existing[0]
        return json({
          ok: true,
          inserted: false,
          event: {
            seq: row.seq,
            eventId: row.event_id,
            kind: row.kind,
            json: row.json,
            createdAtMs: row.created_at,
          },
        })
      }

      sql.exec(
        "insert into events (event_id, kind, json, created_at) values (?, ?, ?, ?)",
        eventId,
        kind,
        jsonPayload,
        createdAtMs,
      )

      const seqRow = sql.exec<{ seq: number }>("select last_insert_rowid() as seq").one()
      const seq = Number(seqRow.seq)

      const row: EventRow = {
        seq,
        event_id: eventId,
        kind,
        json: jsonPayload,
        created_at: createdAtMs,
      }

      // Best-effort replication (non-blocking).
      const convexUrl = this.env.VITE_CONVEX_URL ?? process.env.VITE_CONVEX_URL
      if (convexUrl && token) {
        this.state.waitUntil(this.replicateToConvex(convexUrl, token, userSpaceId, [row]))
      }

      return json({
        ok: true,
        inserted: true,
        event: { eventId, seq, kind, json: jsonPayload, createdAtMs },
      })
    }

    // GET /api/user-space/events?after=123
    if (url.pathname.endsWith("/events") && request.method === "GET") {
      const afterRaw = url.searchParams.get("after")
      const after = afterRaw ? Number(afterRaw) : 0
      const rows = sql
        .exec<EventRow>(
          "select seq, event_id, kind, json, created_at from events where seq > ? order by seq asc limit 200",
          Number.isFinite(after) ? after : 0,
        )
        .toArray()
      const events = rows.map((r) => ({
        seq: r.seq,
        eventId: r.event_id,
        kind: r.kind,
        json: r.json,
        createdAtMs: r.created_at,
      }))
      return json({ ok: true, events })
    }

    return new Response("Not found", { status: 404 })
  }
}
