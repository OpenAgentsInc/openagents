/**
 * In-memory DurableStream backend — reference implementation and default
 * test Layer.
 */
import { Effect, Layer } from "effect"
import {
  DurableStream,
  DurableStreamClosedError,
  type DurableStreamShape,
  type StreamChunk,
} from "./durable-stream.ts"

interface MemoryStream {
  closed: boolean
  readonly chunks: Array<string>
}

export const makeMemoryDurableStream = (): DurableStreamShape => {
  const streams = new Map<string, MemoryStream>()

  const append = (streamId: string, chunk: string) =>
    Effect.suspend(() => {
      let stream = streams.get(streamId)
      if (stream === undefined) {
        stream = { closed: false, chunks: [] }
        streams.set(streamId, stream)
      }
      if (stream.closed) {
        return Effect.fail(new DurableStreamClosedError({ streamId }))
      }
      stream.chunks.push(chunk)
      return Effect.succeed({ offset: stream.chunks.length - 1 })
    })

  const readFrom = (streamId: string, offset: number) =>
    Effect.sync(() => {
      const stream = streams.get(streamId)
      if (stream === undefined) {
        return { chunks: [] as ReadonlyArray<StreamChunk>, closed: false, nextOffset: 0 }
      }
      const start = Math.max(0, offset)
      const chunks: Array<StreamChunk> = []
      for (let index = start; index < stream.chunks.length; index++) {
        const chunk = stream.chunks[index]
        if (chunk !== undefined) chunks.push({ chunkOffset: index, chunk })
      }
      return {
        chunks: chunks as ReadonlyArray<StreamChunk>,
        closed: stream.closed,
        nextOffset: stream.chunks.length,
      }
    })

  const close = (streamId: string) =>
    Effect.sync(() => {
      const stream = streams.get(streamId)
      if (stream === undefined) {
        streams.set(streamId, { closed: true, chunks: [] })
        return
      }
      stream.closed = true
    })

  const status = (streamId: string) =>
    Effect.sync(() => {
      const stream = streams.get(streamId)
      if (stream === undefined) return { exists: false, closed: false, nextOffset: 0 }
      return { exists: true, closed: stream.closed, nextOffset: stream.chunks.length }
    })

  return { append, readFrom, close, status }
}

export const layerMemory = (): Layer.Layer<DurableStream> =>
  Layer.sync(DurableStream, makeMemoryDurableStream)
