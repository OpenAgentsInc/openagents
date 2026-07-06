/**
 * DurableStream — owned append-log primitive (CFG-2, issue #8517, audit §5).
 *
 * NOTE: this is the vendor-neutral infra interface for the Cloudflare exit.
 * The existing `@openagentsinc/durable-stream` package is the HTTP/Durable
 * Object protocol implementation; app code migrating off Workers should
 * depend on THIS interface and pick a backend Layer.
 *
 * Semantics WE define (any backend must pass
 * src/conformance/durable-stream.ts):
 * - Streams are created implicitly by the first `append`. Offsets are
 *   gapless and start at 0.
 * - `readFrom(streamId, offset)` returns every chunk with
 *   `chunkOffset >= offset` in order, plus the stream's `closed` flag and
 *   `nextOffset` (the resume cursor). Reading a missing stream yields no
 *   chunks, `closed: false`, `nextOffset: 0`.
 * - `close` is idempotent and seals the stream: further appends fail
 *   `DurableStreamClosedError`; reads keep working. Closing a missing
 *   stream creates it empty-and-closed (so producers can seal ahead).
 * - `status` never fails on missing streams (`exists: false`).
 *
 * Backends: in-memory (durable-stream-memory.ts) and a Postgres append
 * table (durable-stream-postgres.ts,
 * migrations/0003_oa_infra_durable_stream.sql). Swap targets per the
 * audit: Redis Streams, Kafka.
 */
import { Context, Schema } from "effect"
import type { Effect } from "effect"

/** Unrecoverable backend failure (connection loss, vendor error, ...). */
export class DurableStreamBackendError extends Schema.TaggedErrorClass<DurableStreamBackendError>()(
  "DurableStreamBackendError",
  {
    backend: Schema.String,
    operation: Schema.String,
    cause: Schema.Defect,
  },
) {}

/** Append attempted on a closed stream. */
export class DurableStreamClosedError extends Schema.TaggedErrorClass<DurableStreamClosedError>()(
  "DurableStreamClosedError",
  {
    streamId: Schema.String,
  },
) {}

export interface StreamChunk {
  readonly chunkOffset: number
  readonly chunk: string
}

export interface StreamReadResult {
  readonly chunks: ReadonlyArray<StreamChunk>
  readonly closed: boolean
  /** Resume cursor: pass back as `offset` to continue reading. */
  readonly nextOffset: number
}

export interface StreamStatus {
  readonly exists: boolean
  readonly closed: boolean
  readonly nextOffset: number
}

export interface DurableStreamShape {
  /** Returns the offset the chunk was written at. */
  readonly append: (
    streamId: string,
    chunk: string,
  ) => Effect.Effect<{ readonly offset: number }, DurableStreamBackendError | DurableStreamClosedError>
  readonly readFrom: (
    streamId: string,
    offset: number,
  ) => Effect.Effect<StreamReadResult, DurableStreamBackendError>
  /** Idempotent; creates the stream closed when it does not exist yet. */
  readonly close: (streamId: string) => Effect.Effect<void, DurableStreamBackendError>
  readonly status: (streamId: string) => Effect.Effect<StreamStatus, DurableStreamBackendError>
}

export class DurableStream extends Context.Service<DurableStream, DurableStreamShape>()(
  "@openagentsinc/oa-infra/DurableStream",
) {}
