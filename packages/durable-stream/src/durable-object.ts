/**
 * Cloudflare Durable Object adapter.
 *
 * One DO instance == one durable stream (`idFromName(streamPath)`). The DO's
 * SQLite storage backs a `StreamStore`; the DO's single-threaded serialized
 * execution + implicit per-call transaction satisfy the protocol's
 * "serialize validate+append per (stream, producerId)" and "commit
 * producer-state + log atomically" requirements (PROTOCOL.md §5.2.1) for free.
 *
 * TTL/expiry (§5.1) is driven by DO alarms.
 *
 * This module imports Cloudflare runtime globals (`DurableObject`,
 * `SqlStorage`) and is intentionally NOT imported by the Bun-tested core.
 */
import { handleRequest, streamIdFromUrl } from "./http.ts"
import type { AppendResult, ProducerState, StreamMeta, StreamStore } from "./store.ts"

// Minimal structural typings for the DO SQLite surface (mirrors the world DO's
// approach of typing `ctx.storage.sql` locally rather than depending on
// @cloudflare/workers-types at the package level).
interface SqlCursor<T> {
  toArray(): Array<T>
  one(): T | undefined
}
interface SqlStorageLike {
  exec<T = Record<string, unknown>>(query: string, ...bindings: Array<unknown>): SqlCursor<T>
}
interface DurableObjectStorageLike {
  readonly sql: SqlStorageLike
  setAlarm(scheduledTimeMs: number): Promise<void> | void
  deleteAlarm(): Promise<void> | void
}
interface DurableObjectStateLike {
  readonly storage: DurableObjectStorageLike
  blockConcurrencyWhile<T>(fn: () => Promise<T>): Promise<T>
}

const MIGRATIONS: ReadonlyArray<string> = [
  `CREATE TABLE IF NOT EXISTS ds_meta (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    stream_id TEXT NOT NULL,
    content_type TEXT NOT NULL,
    ttl_seconds INTEGER,
    expires_at_ms INTEGER,
    closed INTEGER NOT NULL DEFAULT 0,
    created_at_ms INTEGER NOT NULL,
    last_stream_seq TEXT
  )`,
  `CREATE TABLE IF NOT EXISTS ds_log (
    pos INTEGER PRIMARY KEY,
    byte BLOB NOT NULL
  )`,
  `CREATE TABLE IF NOT EXISTS ds_producer (
    producer_id TEXT PRIMARY KEY,
    epoch INTEGER NOT NULL,
    last_seq INTEGER NOT NULL,
    closing_epoch INTEGER,
    closing_seq INTEGER
  )`,
]

/** SQLite-backed single-stream store. */
export class SqliteStreamStore implements StreamStore {
  constructor(private readonly sql: SqlStorageLike) {
    for (const m of MIGRATIONS) this.sql.exec(m)
  }

  getMeta(): StreamMeta | null {
    const row = this.sql
      .exec<{
        stream_id: string
        content_type: string
        ttl_seconds: number | null
        expires_at_ms: number | null
        closed: number
        created_at_ms: number
        last_stream_seq: string | null
      }>(`SELECT * FROM ds_meta WHERE id = 1`)
      .one()
    if (row === undefined) return null
    return {
      streamId: row.stream_id,
      contentType: row.content_type,
      ttlSeconds: row.ttl_seconds,
      expiresAtMs: row.expires_at_ms,
      closed: row.closed === 1,
      createdAtMs: row.created_at_ms,
      lastStreamSeq: row.last_stream_seq,
    }
  }

  create(meta: StreamMeta): void {
    this.sql.exec(`DELETE FROM ds_meta`)
    this.sql.exec(`DELETE FROM ds_log`)
    this.sql.exec(`DELETE FROM ds_producer`)
    this.sql.exec(
      `INSERT INTO ds_meta (id, stream_id, content_type, ttl_seconds, expires_at_ms, closed, created_at_ms, last_stream_seq)
       VALUES (1, ?, ?, ?, ?, ?, ?, ?)`,
      meta.streamId,
      meta.contentType,
      meta.ttlSeconds,
      meta.expiresAtMs,
      meta.closed ? 1 : 0,
      meta.createdAtMs,
      meta.lastStreamSeq,
    )
  }

  putMeta(meta: StreamMeta): void {
    this.sql.exec(
      `UPDATE ds_meta SET content_type = ?, ttl_seconds = ?, expires_at_ms = ?, closed = ?, last_stream_seq = ? WHERE id = 1`,
      meta.contentType,
      meta.ttlSeconds,
      meta.expiresAtMs,
      meta.closed ? 1 : 0,
      meta.lastStreamSeq,
    )
  }

  byteLength(): number {
    const row = this.sql
      .exec<{ n: number | null }>(`SELECT MAX(pos + LENGTH(byte)) AS n FROM ds_log`)
      .one()
    return row?.n ?? 0
  }

  readFrom(position: number, maxBytes: number): Uint8Array {
    // Each row stores a contiguous append at byte position `pos`. Stitch the
    // overlapping suffix from `position`.
    const rows = this.sql
      .exec<{ pos: number; byte: ArrayBuffer | Uint8Array }>(
        `SELECT pos, byte FROM ds_log ORDER BY pos`,
      )
      .toArray()
    const chunks: Array<Uint8Array> = []
    let total = 0
    for (const r of rows) {
      const bytes = r.byte instanceof Uint8Array ? r.byte : new Uint8Array(r.byte)
      const rowStart = r.pos
      const rowEnd = r.pos + bytes.length
      if (rowEnd <= position) continue
      const from = Math.max(0, position - rowStart)
      const slice = bytes.subarray(from)
      const room = maxBytes - total
      if (room <= 0) break
      const take = slice.length > room ? slice.subarray(0, room) : slice
      chunks.push(take)
      total += take.length
      if (total >= maxBytes) break
    }
    const out = new Uint8Array(total)
    let off = 0
    for (const c of chunks) {
      out.set(c, off)
      off += c.length
    }
    return out
  }

  appendAtomic(
    bytes: Uint8Array,
    streamSeq: string | null,
    producer: { id: string; state: ProducerState } | null,
  ): AppendResult {
    const pos = this.byteLength()
    if (bytes.length > 0) {
      this.sql.exec(`INSERT INTO ds_log (pos, byte) VALUES (?, ?)`, pos, bytes)
    }
    if (streamSeq !== null) {
      this.sql.exec(`UPDATE ds_meta SET last_stream_seq = ? WHERE id = 1`, streamSeq)
    }
    if (producer !== null) {
      this.sql.exec(
        `INSERT INTO ds_producer (producer_id, epoch, last_seq, closing_epoch, closing_seq)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(producer_id) DO UPDATE SET epoch = excluded.epoch, last_seq = excluded.last_seq,
           closing_epoch = excluded.closing_epoch, closing_seq = excluded.closing_seq`,
        producer.id,
        producer.state.epoch,
        producer.state.lastSeq,
        producer.state.closingTuple?.epoch ?? null,
        producer.state.closingTuple?.seq ?? null,
      )
    }
    return { tailLength: this.byteLength() }
  }

  getProducer(producerId: string): ProducerState | null {
    const row = this.sql
      .exec<{ epoch: number; last_seq: number; closing_epoch: number | null; closing_seq: number | null }>(
        `SELECT epoch, last_seq, closing_epoch, closing_seq FROM ds_producer WHERE producer_id = ?`,
        producerId,
      )
      .one()
    if (row === undefined) return null
    return {
      epoch: row.epoch,
      lastSeq: row.last_seq,
      closingTuple:
        row.closing_epoch !== null && row.closing_seq !== null
          ? { epoch: row.closing_epoch, seq: row.closing_seq }
          : null,
    }
  }

  destroy(): void {
    this.sql.exec(`DELETE FROM ds_meta`)
    this.sql.exec(`DELETE FROM ds_log`)
    this.sql.exec(`DELETE FROM ds_producer`)
  }
}

/**
 * The Durable Object class. Extend `DurableObject` in the host Worker and route
 * `/v1/stream/*` here, keying the DO by the stream path.
 *
 * Usage in a host Worker:
 *   export class DurableStreamObject extends makeDurableStreamObject(env) { ... }
 * or simply embed `SqliteStreamStore` + `handleRequest` in your own DO class.
 */
export interface DurableStreamHooks {
  readonly state: DurableObjectStateLike
}

export const handleDurableStreamFetch = async (
  state: DurableObjectStateLike,
  request: Request,
): Promise<Response> => {
  const streamId = streamIdFromUrl(request.url)
  if (streamId === null) {
    return new Response("not a stream url", { status: 404 })
  }
  const store = new SqliteStreamStore(state.storage.sql)
  const nowMs = Date.now()
  const res = await handleRequest(store, request, { streamId, nowMs })
  // Refresh TTL alarm on any reaching request (§5.1: reads/writes reset the
  // sliding window; HEAD does not).
  if (request.method !== "HEAD") {
    const meta = store.getMeta()
    if (meta?.ttlSeconds != null && meta.ttlSeconds > 0) {
      await state.storage.setAlarm(nowMs + meta.ttlSeconds * 1000)
    } else if (meta?.expiresAtMs != null) {
      await state.storage.setAlarm(meta.expiresAtMs)
    }
  }
  return res
}

/** Alarm handler: expire (destroy) the stream when its TTL/expiry elapses. */
export const handleDurableStreamAlarm = (state: DurableObjectStateLike): void => {
  const store = new SqliteStreamStore(state.storage.sql)
  store.destroy()
}

export type { DurableObjectStateLike, SqlStorageLike }
