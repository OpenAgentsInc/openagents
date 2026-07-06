// Postgres-backed durable inference stream (CFG-6, #8521, epic #8515).
//
// REPLACES the `DurableInferenceStreamObject` Durable Object: the per-request
// durable offset log now lives in Postgres (`oa_infra_streams` /
// `oa_infra_stream_chunks`, khala-sync migration 0040) behind the owned
// `@openagentsinc/oa-infra` DurableStream interface, reached through the SAME
// workerd-compatible client/connection path khala-sync uses — the
// KHALA_SYNC_DB Hyperdrive binding with postgres.js (`prepare: false`,
// `max: 1`; see `defaultMakeKhalaSyncSqlClient` in khala-sync-push-routes.ts).
//
// HOW THE SWAP STAYS BYTE-COMPATIBLE: the production transport
// (durable-inference-do-transport.ts — `seedDurableInferenceStreamDO`,
// `teeUpstreamToDurableDO`, `replayFromOffsetDO`) and every consumer of it are
// UNCHANGED. This module implements their `DurableStreamNamespace` port — the
// `/v1/stream/{id}` HTTP contract the DO used to serve (PUT create, POST
// append + optional `stream-closed`, GET replay with `?offset=` returning
// `stream-next-offset` / `stream-closed` / `stream-up-to-date`, 404 unknown
// id, 400 malformed offset) — over the oa-infra `DurableStreamShape`. Public
// offsets remain BYTE positions into the concatenated stored SSE frames,
// minted with the same zero-padded codec (`offsetForPosition`) and the same
// `-1` / `now` sentinels (`parseOffset`) as the DO, so
// `GET /v1/chat/completions/durable/{requestId}?offset=N` (the Khala MCP
// `khala resume <durableRequestId> --offset 0` contract) is byte-identical.
//
// PRODUCER-TUPLE NOTE: the DO deduped appends on the `(producer-id, epoch,
// seq)` headers. Every producer of this namespace is the in-process gateway
// writing sequential awaited appends with NO retries (a failed persist
// degrades the mirror; it never re-sends), so the tuple's exactly-once role is
// carried by Postgres's single-transaction append; the headers are accepted
// and ignored.
//
// METERING: exactly as before, metering lives ONLY in the route's `onEof`
// producer drain. This module stores and serves bytes; it has no metering
// hook, so replays/resumes can never bill.
//
// FAIL-SAFE: any Postgres/connection fault rejects the stub `fetch()`, which
// the unchanged transport already maps to "durable unavailable" (producer
// degrades to pass-through; the read route answers 502) — the same degraded
// behavior as a broken DO binding.
import { offsetForPosition, parseOffset } from '@openagentsinc/durable-stream'
import type { DurableStreamShape } from '@openagentsinc/oa-infra/durable-stream'
import {
  type DurableStreamSqlClient,
  type DurableStreamSqlTx,
  deleteExpiredStreams,
  makePostgresDurableStream,
} from '@openagentsinc/oa-infra/durable-stream-postgres-core'
import { Effect } from 'effect'

import {
  DURABLE_INFERENCE_CONTENT_TYPE,
  DURABLE_INFERENCE_TTL_SECONDS,
} from './durable-inference-proxy'
import { type DurableStreamNamespace } from './durable-inference-do-transport'

// The DO addressed streams at `/v1/stream/{streamId}` (durable-stream
// `http.ts` STREAM_URL_PREFIX). Same URL scheme here so the transport's wire
// requests resolve identically.
const STREAM_URL_PREFIX = '/v1/stream/'

// Same server-defined read cap as the package core (core.ts MAX_CHUNK_BYTES):
// one catch-up GET returns at most 1 MiB and the client resumes from
// `stream-next-offset`.
const MAX_READ_BYTES = 1 << 20

/**
 * One backend session: the stream store plus its connection teardown. The
 * postgres.js session below is the production shape; tests inject the
 * oa-infra in-memory backend with a no-op `end`.
 */
export interface DurableInferenceStreamSession {
  readonly streams: DurableStreamShape
  /** Release the underlying connection(s). Safe to call more than once. */
  readonly end: () => Promise<void>
  /** Optional TTL sweep (production Postgres session only). Best-effort. */
  readonly cleanupExpired?: ((ttlSeconds: number) => Promise<void>) | undefined
}

export type MakeDurableInferenceStreamSession =
  () => Promise<DurableInferenceStreamSession>

const encoder = new TextEncoder()

// The ONE named Effect bridge for this module (zero-debt architecture
// budget: 1). The `DurableStreamNamespace` port is Promise-shaped
// (`stub.fetch(Request): Promise<Response>` — the DO wire contract this
// backend preserves), so every oa-infra DurableStream Effect is run through
// this single helper at that boundary. Ratchet away if the transport port
// becomes an Effect program end-to-end (CFG-9 monolith is the natural
// moment).
const runStreamEffect = <A, E>(effect: Effect.Effect<A, E>): Promise<A> =>
  Effect.runPromise(effect)

const isStreamClosedHeader = (request: Request): boolean =>
  request.headers.get('stream-closed') === 'true'

const plainResponse = (
  status: number,
  headers: Record<string, string> = {},
): Response => new Response(null, { headers, status })

/**
 * Implement the transport's `DurableStreamNamespace` port over an oa-infra
 * `DurableStreamShape` session.
 *
 * SESSION LIFECYCLE: one lazy session per namespace instance (the Worker
 * wiring creates one namespace per request), opened on the first stub fetch
 * and shared across the producer's PUT + appends + close. It is ended after
 * the terminal shapes — a `stream-closed` POST (the producer's EOF/fault
 * close) and every GET (the read route issues exactly one) — so the
 * postgres.js connection never outlives its work; a later operation on the
 * same namespace transparently opens a fresh session. The production session
 * also sets a driver idle timeout as a backstop for abandoned producers.
 */
export const makeDurableInferenceStreamNamespace = (
  makeSession: MakeDurableInferenceStreamSession,
): DurableStreamNamespace => {
  let sessionPromise: Promise<DurableInferenceStreamSession> | undefined

  const session = (): Promise<DurableInferenceStreamSession> => {
    sessionPromise ??= makeSession().catch((error: unknown) => {
      // A failed open must not poison the namespace: clear the memo so a
      // later operation can retry, then let the caller's fetch reject.
      sessionPromise = undefined
      throw error
    })
    return sessionPromise
  }

  const endSession = async (): Promise<void> => {
    const pending = sessionPromise
    sessionPromise = undefined
    if (pending === undefined) {
      return
    }
    try {
      await (await pending).end()
    } catch {
      // Teardown is best-effort; the next operation opens a fresh session.
    }
  }

  const handlePut = async (
    active: DurableInferenceStreamSession,
    streamId: string,
  ): Promise<Response> => {
    const status = await runStreamEffect(active.streams.status(streamId))
    const wantClosed = false // the inference producer never PUT-creates closed
    if (status.exists) {
      // Idempotent re-create; a closed stream no longer matches the open
      // config → 409, exactly the package core's PUT conflict (the transport
      // treats non-200/201 as "durable unavailable" and fails safe).
      return status.closed === wantClosed
        ? plainResponse(200, {
            'content-type': DURABLE_INFERENCE_CONTENT_TYPE,
          })
        : plainResponse(409)
    }
    // The oa-infra interface creates streams implicitly on first append, but
    // the resume contract must serve `200 ` (empty, open) for a created
    // stream BEFORE its first frame — so creation appends one empty chunk
    // (zero bytes: invisible to the byte-offset contract).
    await runStreamEffect(
      active.streams.append(streamId, '').pipe(
        Effect.catchTag('DurableStreamClosedError', () => Effect.void),
      ),
    )
    // Opportunistic TTL sweep, once per stream creation (the DO used a
    // storage alarm). Best-effort: a sweep failure never blocks the create.
    if (active.cleanupExpired !== undefined) {
      try {
        await active.cleanupExpired(DURABLE_INFERENCE_TTL_SECONDS)
      } catch {
        // ignore — bounded growth is restored by the next successful sweep
      }
    }
    return plainResponse(201, {
      'content-type': DURABLE_INFERENCE_CONTENT_TYPE,
      location: streamId,
    })
  }

  const handlePost = async (
    active: DurableInferenceStreamSession,
    streamId: string,
    request: Request,
  ): Promise<Response> => {
    const wantClose = isStreamClosedHeader(request)
    const body = await request.text()
    if (body.length === 0 && !wantClose) {
      return plainResponse(400)
    }
    if (body.length > 0) {
      const appended = await runStreamEffect(
        active.streams.append(streamId, body).pipe(
          Effect.map(() => 'appended' as const),
          Effect.catchTag('DurableStreamClosedError', () =>
            Effect.succeed('closed' as const),
          ),
        ),
      )
      if (appended === 'closed') {
        // Append to a sealed stream → 409 + stream-closed, like the DO core.
        return plainResponse(409, { 'stream-closed': 'true' })
      }
    }
    if (wantClose) {
      await runStreamEffect(active.streams.close(streamId))
      // The close is the producer's last write; release the connection.
      await endSession()
      return plainResponse(200, { 'stream-closed': 'true' })
    }
    return plainResponse(200)
  }

  const handleGet = async (
    active: DurableInferenceStreamSession,
    streamId: string,
    url: URL,
  ): Promise<Response> => {
    const parsed = parseOffset(url.searchParams.get('offset'))
    if (parsed === null) {
      return plainResponse(400)
    }
    const status = await runStreamEffect(active.streams.status(streamId))
    if (!status.exists) {
      return plainResponse(404)
    }
    const read = await runStreamEffect(active.streams.readFrom(streamId, 0))
    const full = encoder.encode(read.chunks.map(chunk => chunk.chunk).join(''))
    const tailLen = full.byteLength
    const startPos =
      parsed.kind === 'beginning'
        ? 0
        : parsed.kind === 'now'
          ? tailLen
          : Math.min(parsed.position, tailLen)
    const bodyBytes = full.slice(startPos, startPos + MAX_READ_BYTES)
    const endPos = startPos + bodyBytes.byteLength
    const upToDate = endPos >= tailLen
    // §5.6 of the stream protocol: `stream-closed` only when closed AND the
    // reader has reached the final offset.
    const closedAtTail = read.closed && upToDate

    const headers: Record<string, string> = {
      'cache-control': 'no-store',
      'content-type': DURABLE_INFERENCE_CONTENT_TYPE,
      'stream-next-offset': offsetForPosition(endPos),
    }
    if (upToDate) {
      headers['stream-up-to-date'] = 'true'
    }
    if (closedAtTail) {
      headers['stream-closed'] = 'true'
    }
    return new Response(bodyBytes, { headers, status: 200 })
  }

  const handle = async (request: Request): Promise<Response> => {
    const url = new URL(request.url)
    if (!url.pathname.startsWith(STREAM_URL_PREFIX)) {
      return plainResponse(404)
    }
    const rawId = url.pathname.slice(STREAM_URL_PREFIX.length)
    if (rawId.length === 0) {
      return plainResponse(404)
    }
    const streamId = decodeURIComponent(rawId)

    switch (request.method.toUpperCase()) {
      case 'PUT':
        return handlePut(await session(), streamId)
      case 'POST':
        return handlePost(await session(), streamId, request)
      case 'GET': {
        try {
          return await handleGet(await session(), streamId, url)
        } finally {
          // The read route issues exactly one GET per request; release the
          // connection whether the read succeeded or faulted.
          await endSession()
        }
      }
      default:
        return plainResponse(405, { allow: 'GET, PUT, POST' })
    }
  }

  return {
    getByName: () => ({ fetch: handle }),
  }
}

// ---------------------------------------------------------------------------
// Production postgres.js session over the KHALA_SYNC_DB Hyperdrive binding
// ---------------------------------------------------------------------------

/** Structural slice of a postgres.js client (same seam as khala-sync). */
type PostgresJsClient = DurableStreamSqlTx & {
  end: (options?: { timeout?: number }) => Promise<void>
}

/**
 * Transaction-mode-safe postgres.js session for the durable inference
 * stream: one connection, unnamed statements only (`prepare: false`), no
 * session state — the exact discipline of `defaultMakeKhalaSyncSqlClient`
 * (khala-sync-push-routes.ts) for the same Hyperdrive binding. The dynamic
 * import keeps the driver out of test bundles; tests inject fakes.
 *
 * `idle_timeout` is the abandoned-producer backstop: if a faulting producer
 * never reaches its close (so the namespace never calls `end`), the driver
 * closes the idle connection itself.
 */
export const makePostgresJsDurableInferenceStreamSession = async (
  connectionString: string,
): Promise<DurableInferenceStreamSession> => {
  const mod = (await import('postgres')) as unknown as {
    default: (
      connectionString: string,
      options: Record<string, unknown>,
    ) => PostgresJsClient
  }
  const sql = mod.default(connectionString, {
    connect_timeout: 10,
    idle_timeout: 60,
    max: 1,
    prepare: false,
  })
  return {
    cleanupExpired: ttlSeconds =>
      deleteExpiredStreams(sql as unknown as DurableStreamSqlTx, ttlSeconds),
    end: () => sql.end({ timeout: 5 }),
    // postgres.js exposes the same tagged-template + `begin` surface as Bun
    // SQL; the cast is the single deliberate driver seam, proven equivalent
    // by the oa-infra conformance suite's `postgres.js` run.
    streams: makePostgresDurableStream(sql as unknown as DurableStreamSqlClient),
  }
}

/**
 * Resolve the production namespace from the Worker env: durable streaming is
 * LIVE when the durable-stream flag is on AND the KHALA_SYNC_DB Hyperdrive
 * binding is wired; otherwise `undefined` and every consumer stays on its
 * fail-safe non-durable path (exactly the old DO-binding-absent behavior).
 */
export const durableInferenceStreamNamespaceForEnv = (
  env: Readonly<{
    KHALA_SYNC_DB?: Readonly<{ connectionString: string }> | undefined
  }>,
  options: Readonly<{ enabled: boolean }>,
): DurableStreamNamespace | undefined => {
  const binding = env.KHALA_SYNC_DB
  if (!options.enabled || binding === undefined) {
    return undefined
  }
  return makeDurableInferenceStreamNamespace(() =>
    makePostgresJsDurableInferenceStreamSession(binding.connectionString),
  )
}
