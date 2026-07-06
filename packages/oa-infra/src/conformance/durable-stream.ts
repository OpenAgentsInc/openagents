/**
 * DurableStream conformance suite (CFG-2, issue #8517).
 *
 * EVERY DurableStream backend must pass this suite unmodified (audit §5
 * hot-swap guarantee). Stream ids are namespaced per test with a fresh
 * UUID so shared backends can host the suite.
 */
import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import type { Layer } from "effect"
import { DurableStream, type DurableStreamShape } from "../durable-stream.ts"

export interface DurableStreamConformanceOptions {
  readonly label: string
  /** Called at TEST time (after any beforeAll infra setup). */
  readonly makeLayer: () => Layer.Layer<DurableStream>
  readonly skip?: boolean
}

export const runDurableStreamConformance = (
  options: DurableStreamConformanceOptions,
): void => {
  const suite = options.skip === true ? describe.skip : describe
  const streamId = () => `ds-conf-${crypto.randomUUID()}`

  const withStreams = async <A>(
    body: (streams: DurableStreamShape) => Promise<A>,
  ): Promise<A> => {
    const layer = options.makeLayer()
    return Effect.runPromise(
      Effect.provide(
        Effect.gen(function* () {
          const streams = yield* DurableStream
          return yield* Effect.promise(() => body(streams))
        }),
        layer,
      ),
    )
  }

  const runS = <A, E>(effect: Effect.Effect<A, E>): Promise<A> => Effect.runPromise(effect)

  suite(`DurableStream conformance [${options.label}]`, () => {
    test("a missing stream reads empty and reports exists=false", async () => {
      const id = streamId()
      await withStreams(async (streams) => {
        const read = await runS(streams.readFrom(id, 0))
        expect(read.chunks).toEqual([])
        expect(read.closed).toBe(false)
        expect(read.nextOffset).toBe(0)
        const status = await runS(streams.status(id))
        expect(status).toEqual({ exists: false, closed: false, nextOffset: 0 })
      })
    })

    test("appends are gapless from offset 0 and read back in order", async () => {
      const id = streamId()
      await withStreams(async (streams) => {
        const first = await runS(streams.append(id, "alpha"))
        const second = await runS(streams.append(id, "beta"))
        const third = await runS(streams.append(id, "gamma"))
        expect([first.offset, second.offset, third.offset]).toEqual([0, 1, 2])

        const all = await runS(streams.readFrom(id, 0))
        expect(all.chunks.map((chunk) => chunk.chunk)).toEqual(["alpha", "beta", "gamma"])
        expect(all.chunks.map((chunk) => chunk.chunkOffset)).toEqual([0, 1, 2])
        expect(all.nextOffset).toBe(3)
        expect(all.closed).toBe(false)
      })
    })

    test("readFrom(offset) resumes mid-stream; reading past the end is empty", async () => {
      const id = streamId()
      await withStreams(async (streams) => {
        await runS(streams.append(id, "a"))
        await runS(streams.append(id, "b"))
        await runS(streams.append(id, "c"))
        const tail = await runS(streams.readFrom(id, 1))
        expect(tail.chunks.map((chunk) => chunk.chunk)).toEqual(["b", "c"])
        const past = await runS(streams.readFrom(id, 3))
        expect(past.chunks).toEqual([])
        expect(past.nextOffset).toBe(3)
      })
    })

    test("close seals the stream: appends fail, reads keep working", async () => {
      const id = streamId()
      await withStreams(async (streams) => {
        await runS(streams.append(id, "only"))
        await runS(streams.close(id))
        await runS(streams.close(id)) // idempotent

        const appendExit = await Effect.runPromiseExit(streams.append(id, "too late"))
        expect(appendExit._tag).toBe("Failure")

        const read = await runS(streams.readFrom(id, 0))
        expect(read.closed).toBe(true)
        expect(read.chunks.map((chunk) => chunk.chunk)).toEqual(["only"])
        const status = await runS(streams.status(id))
        expect(status).toEqual({ exists: true, closed: true, nextOffset: 1 })
      })
    })

    test("closing a missing stream creates it empty-and-closed", async () => {
      const id = streamId()
      await withStreams(async (streams) => {
        await runS(streams.close(id))
        const status = await runS(streams.status(id))
        expect(status).toEqual({ exists: true, closed: true, nextOffset: 0 })
        const appendExit = await Effect.runPromiseExit(streams.append(id, "no"))
        expect(appendExit._tag).toBe("Failure")
      })
    })

    test("streams are isolated from each other", async () => {
      const a = streamId()
      const b = streamId()
      await withStreams(async (streams) => {
        await runS(streams.append(a, "in-a"))
        await runS(streams.close(a))
        const offsetInB = await runS(streams.append(b, "in-b"))
        expect(offsetInB.offset).toBe(0)
        const readB = await runS(streams.readFrom(b, 0))
        expect(readB.closed).toBe(false)
        expect(readB.chunks.map((chunk) => chunk.chunk)).toEqual(["in-b"])
      })
    })
  })
}
