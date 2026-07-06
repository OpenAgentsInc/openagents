/**
 * Postgres DurableStream backend
 * (migrations/0003_oa_infra_durable_stream.sql).
 *
 * Appends run in one transaction: bump the header row's `next_offset`
 * (guarded by `closed = false`) and insert the chunk at the claimed offset.
 * Row-level locking on the header row serializes concurrent appends per
 * stream, so offsets are gapless.
 */
import { Effect, Layer } from "effect"
import {
  DurableStream,
  DurableStreamBackendError,
  DurableStreamClosedError,
  type DurableStreamShape,
  type StreamChunk,
} from "./durable-stream.ts"
import { OaInfraSql } from "./sql.ts"
import type { SQL } from "bun"

const BACKEND = "postgres"

const tryPg = <A>(operation: string, run: () => Promise<A>) =>
  Effect.tryPromise({
    try: run,
    catch: (cause) => new DurableStreamBackendError({ backend: BACKEND, operation, cause }),
  })

export const makePostgresDurableStream = (sql: SQL): DurableStreamShape => {
  const append = (streamId: string, chunk: string) =>
    tryPg("append", async (): Promise<number | "closed"> => {
      return sql.begin(async (tx) => {
        const rows: Array<{ next_offset: string | number; closed: boolean }> = await tx`
          INSERT INTO oa_infra_streams (stream_id, next_offset)
          VALUES (${streamId}, 1)
          ON CONFLICT (stream_id) DO UPDATE SET
            next_offset = oa_infra_streams.next_offset + 1,
            updated_at = now()
          WHERE oa_infra_streams.closed = false
          RETURNING next_offset, closed
        `
        const row = rows[0]
        if (row === undefined) return "closed" as const
        const offset = Number(row.next_offset) - 1
        await tx`
          INSERT INTO oa_infra_stream_chunks (stream_id, chunk_offset, chunk)
          VALUES (${streamId}, ${offset}, ${chunk})
        `
        return offset
      })
    }).pipe(
      Effect.flatMap((result) =>
        result === "closed"
          ? Effect.fail(new DurableStreamClosedError({ streamId }))
          : Effect.succeed({ offset: result }),
      ),
    )

  const readFrom = (streamId: string, offset: number) =>
    tryPg("readFrom", async () => {
      const headers: Array<{ closed: boolean; next_offset: string | number }> = await sql`
        SELECT closed, next_offset FROM oa_infra_streams WHERE stream_id = ${streamId}
      `
      const header = headers[0]
      if (header === undefined) {
        return { chunks: [] as ReadonlyArray<StreamChunk>, closed: false, nextOffset: 0 }
      }
      const rows: Array<{ chunk_offset: string | number; chunk: string }> = await sql`
        SELECT chunk_offset, chunk
        FROM oa_infra_stream_chunks
        WHERE stream_id = ${streamId} AND chunk_offset >= ${Math.max(0, offset)}
        ORDER BY chunk_offset
      `
      return {
        chunks: rows.map(
          (row): StreamChunk => ({ chunkOffset: Number(row.chunk_offset), chunk: row.chunk }),
        ) as ReadonlyArray<StreamChunk>,
        closed: header.closed,
        nextOffset: Number(header.next_offset),
      }
    })

  const close = (streamId: string) =>
    tryPg("close", async () => {
      await sql`
        INSERT INTO oa_infra_streams (stream_id, closed)
        VALUES (${streamId}, true)
        ON CONFLICT (stream_id) DO UPDATE SET closed = true, updated_at = now()
      `
    })

  const status = (streamId: string) =>
    tryPg("status", async () => {
      const rows: Array<{ closed: boolean; next_offset: string | number }> = await sql`
        SELECT closed, next_offset FROM oa_infra_streams WHERE stream_id = ${streamId}
      `
      const row = rows[0]
      if (row === undefined) return { exists: false, closed: false, nextOffset: 0 }
      return { exists: true, closed: row.closed, nextOffset: Number(row.next_offset) }
    })

  return { append, readFrom, close, status }
}

/** Postgres DurableStream Layer; requires `OaInfraSql` (see src/sql.ts). */
export const layerPostgres: Layer.Layer<DurableStream, never, OaInfraSql> = Layer.effect(
  DurableStream,
  Effect.gen(function* () {
    const { sql } = yield* OaInfraSql
    return makePostgresDurableStream(sql)
  }),
)
