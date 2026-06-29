/**
 * Storage port for a single stream's durable state, plus an in-memory
 * implementation used by Bun unit tests. The DO adapter (durable-object.ts)
 * provides a SQLite-backed implementation of the same port.
 *
 * The port is intentionally synchronous and single-stream: it models exactly
 * the surface a Cloudflare Durable Object gives us — serialized, single-threaded
 * access with an implicit per-call transaction. The protocol's "serialize
 * validate+append per (stream, producerId)" and "commit producer-state + log
 * atomically" requirements (PROTOCOL.md §5.2.1) are satisfied because each
 * `StreamStore` instance IS one stream's DO, and `appendAtomic` writes the log
 * row and producer-state row in one call.
 */

export interface StreamMeta {
  readonly contentType: string
  readonly ttlSeconds: number | null
  readonly expiresAtMs: number | null
  readonly closed: boolean
  readonly createdAtMs: number
  /** Stable internal id for ETag composition; never exposed as an offset. */
  readonly streamId: string
  /** Last accepted Stream-Seq (lexicographic), or null. §5.2 */
  readonly lastStreamSeq: string | null
}

export interface ProducerState {
  readonly epoch: number
  readonly lastSeq: number
  /** The (producerId,epoch,seq) tuple that performed the closing append, if any. */
  readonly closingTuple: { readonly epoch: number; readonly seq: number } | null
}

export interface AppendResult {
  /** Tail byte length after the append. */
  readonly tailLength: number
}

/**
 * Per-stream durable storage. All methods are synchronous to model the DO's
 * serialized transactional access.
 */
export interface StreamStore {
  /** Stream metadata, or null if the stream does not exist. */
  getMeta(): StreamMeta | null

  /** Create the stream. Caller guarantees it does not already exist. */
  create(meta: StreamMeta): void

  /** Replace metadata (e.g. mark closed, refresh expiry). */
  putMeta(meta: StreamMeta): void

  /** Total stored byte length (== tail position). */
  byteLength(): number

  /** Read the exact byte suffix starting at `position`, capped at `maxBytes`. */
  readFrom(position: number, maxBytes: number): Uint8Array

  /**
   * Append bytes and (optionally) producer state in one atomic step.
   * Returns the new tail length.
   */
  appendAtomic(
    bytes: Uint8Array,
    streamSeq: string | null,
    producer: { id: string; state: ProducerState } | null,
  ): AppendResult

  /** Producer state for `producerId`, or null if unknown. */
  getProducer(producerId: string): ProducerState | null

  /** Delete all stream state. */
  destroy(): void
}

/** In-memory single-stream store for unit tests. */
export class MemoryStreamStore implements StreamStore {
  private meta: StreamMeta | null = null
  private buf: Uint8Array = new Uint8Array(0)
  private producers = new Map<string, ProducerState>()

  getMeta(): StreamMeta | null {
    return this.meta
  }

  create(meta: StreamMeta): void {
    this.meta = meta
    this.buf = new Uint8Array(0)
    this.producers.clear()
  }

  putMeta(meta: StreamMeta): void {
    this.meta = meta
  }

  byteLength(): number {
    return this.buf.length
  }

  readFrom(position: number, maxBytes: number): Uint8Array {
    const start = Math.max(0, Math.min(position, this.buf.length))
    const end = Math.min(this.buf.length, start + maxBytes)
    return this.buf.slice(start, end)
  }

  appendAtomic(
    bytes: Uint8Array,
    streamSeq: string | null,
    producer: { id: string; state: ProducerState } | null,
  ): AppendResult {
    if (bytes.length > 0) {
      const next = new Uint8Array(this.buf.length + bytes.length)
      next.set(this.buf, 0)
      next.set(bytes, this.buf.length)
      this.buf = next
    }
    if (this.meta !== null && streamSeq !== null) {
      this.meta = { ...this.meta, lastStreamSeq: streamSeq }
    }
    if (producer !== null) {
      this.producers.set(producer.id, producer.state)
    }
    return { tailLength: this.buf.length }
  }

  getProducer(producerId: string): ProducerState | null {
    return this.producers.get(producerId) ?? null
  }

  destroy(): void {
    this.meta = null
    this.buf = new Uint8Array(0)
    this.producers.clear()
  }
}
