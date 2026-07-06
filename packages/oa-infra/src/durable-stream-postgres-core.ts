/**
 * Postgres DurableStream backend — driver-agnostic core (CFG-2 #8517,
 * CFG-6 #8521; migrations/0003_oa_infra_durable_stream.sql).
 *
 * Appends run in one transaction: bump the header row's `next_offset`
 * (guarded by `closed = false`) and insert the chunk at the claimed offset.
 * Row-level locking on the header row serializes concurrent appends per
 * stream, so offsets are gapless.
 *
 * WHY THIS FILE IMPORTS NO BUN TYPES: the openagents.com Worker (workerd,
 * until CFG-9 moves it to the Bun monolith) reaches the same tables through
 * its KHALA_SYNC_DB Hyperdrive binding with postgres.js — the exact
 * transaction-mode-safe client discipline khala-sync-server uses. Bun's
 * built-in `SQL` and postgres.js expose the same tagged-template + `begin`
 * surface, so the backend accepts that structural slice
 * (`DurableStreamSqlClient`) and both drivers plug in. The Bun `SQL` Layer
 * stays in ./durable-stream-postgres.ts; the postgres.js driver run of the
 * conformance suite lives in ./postgres-backends.test.ts.
 */
import { Effect } from "effect"
import {
  DurableStreamBackendError,
  DurableStreamClosedError,
  type DurableStreamShape,
  type StreamChunk,
} from "./durable-stream.ts"

/** One transaction handle: a tagged-template query function. */
export interface DurableStreamSqlTx {
  <T = Record<string, unknown>>(
    strings: TemplateStringsArray,
    ...values: ReadonlyArray<unknown>
  ): Promise<Array<T>>
}

/**
 * The structural client slice this backend needs — satisfied by Bun's
 * built-in `SQL` and by postgres.js (`postgres(url, { prepare: false })`).
 * Callers hand their driver instance across this seam with a single
 * deliberate cast (the same driver-seam convention as khala-sync-server's
 * `SyncSql`); behavioral equivalence is proven by running the conformance
 * suite against both drivers.
 */
export interface DurableStreamSqlClient extends DurableStreamSqlTx {
  readonly begin: <T>(run: (tx: DurableStreamSqlTx) => Promise<T>) => Promise<T>
}

const BACKEND = "postgres"

const tryPg = <A>(operation: string, run: () => Promise<A>) =>
  Effect.tryPromise({
    try: run,
    catch: (cause) => new DurableStreamBackendError({ backend: BACKEND, operation, cause }),
  })

export const makePostgresDurableStream = (sql: DurableStreamSqlClient): DurableStreamShape => {
  const append = (streamId: string, chunk: string) =>
    tryPg("append", async (): Promise<number | "closed"> => {
      return sql.begin(async (tx) => {
        const rows = await tx<{ next_offset: string | number; closed: boolean }>`
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
      const headers = await sql<{ closed: boolean; next_offset: string | number }>`
        SELECT closed, next_offset FROM oa_infra_streams WHERE stream_id = ${streamId}
      `
      const header = headers[0]
      if (header === undefined) {
        return { chunks: [] as ReadonlyArray<StreamChunk>, closed: false, nextOffset: 0 }
      }
      const rows = await sql<{ chunk_offset: string | number; chunk: string }>`
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
      const rows = await sql<{ closed: boolean; next_offset: string | number }>`
        SELECT closed, next_offset FROM oa_infra_streams WHERE stream_id = ${streamId}
      `
      const row = rows[0]
      if (row === undefined) return { exists: false, closed: false, nextOffset: 0 }
      return { exists: true, closed: row.closed, nextOffset: Number(row.next_offset) }
    })

  return { append, readFrom, close, status }
}

/**
 * Best-effort TTL sweep (CFG-6 #8521): delete stream headers (chunks follow
 * via ON DELETE CASCADE) whose last write is older than `ttlSeconds`. The
 * DO backend this replaces expired per-stream state with a storage alarm;
 * on Postgres the producer path runs this opportunistically at stream
 * creation, so the table stays bounded without a scheduler.
 */
export const deleteExpiredStreams = async (
  sql: DurableStreamSqlTx,
  ttlSeconds: number,
): Promise<void> => {
  await sql`
    DELETE FROM oa_infra_streams
    WHERE updated_at < now() - make_interval(secs => ${ttlSeconds})
  `
}
