/**
 * JobQueue conformance suite (CFG-2, issue #8517).
 *
 * EVERY JobQueue backend must pass this suite unmodified (audit §5 hot-swap
 * guarantee). Topics are namespaced per test with a fresh UUID so shared
 * backends (one Postgres database) can host the suite.
 */
import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import type { Layer } from "effect"
import { JobQueue, type JobQueueShape } from "../job-queue.ts"

export interface JobQueueConformanceOptions {
  readonly label: string
  /** Called at TEST time (after any beforeAll infra setup). */
  readonly makeLayer: () => Layer.Layer<JobQueue>
  readonly skip?: boolean
}

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms))

export const runJobQueueConformance = (options: JobQueueConformanceOptions): void => {
  const suite = options.skip === true ? describe.skip : describe
  const topic = () => `jq-conf-${crypto.randomUUID()}`

  /** One layer per test so multi-step scenarios share backend state. */
  const withQueue = async <A>(
    body: (queue: JobQueueShape) => Promise<A>,
  ): Promise<A> => {
    const layer = options.makeLayer()
    return Effect.runPromise(
      Effect.provide(
        Effect.gen(function* () {
          const queue = yield* JobQueue
          return yield* Effect.promise(() => body(queue))
        }),
        layer,
      ),
    )
  }

  const runQ = <A, E>(effect: Effect.Effect<A, E>): Promise<A> => Effect.runPromise(effect)

  suite(`JobQueue conformance [${options.label}]`, () => {
    test("enqueue then lease delivers the payload with attempts=1", async () => {
      const t = topic()
      await withQueue(async (queue) => {
        const id = await runQ(queue.enqueue(t, JSON.stringify({ n: 1 })))
        const jobs = await runQ(queue.lease(t))
        expect(jobs.length).toBe(1)
        expect(jobs[0]?.id).toBe(id)
        expect(jobs[0]?.topic).toBe(t)
        expect(jobs[0]?.payload).toBe(JSON.stringify({ n: 1 }))
        expect(jobs[0]?.attempts).toBe(1)
      })
    })

    test("ack completes the job — it is never delivered again", async () => {
      const t = topic()
      await withQueue(async (queue) => {
        await runQ(queue.enqueue(t, "job"))
        const [job] = await runQ(queue.lease(t))
        expect(job).toBeDefined()
        await runQ(queue.ack((job as { id: string }).id))
        expect(await runQ(queue.lease(t))).toEqual([])
      })
    })

    test("lease respects batch and does not double-claim", async () => {
      const t = topic()
      await withQueue(async (queue) => {
        await runQ(queue.enqueue(t, "a"))
        await runQ(queue.enqueue(t, "b"))
        await runQ(queue.enqueue(t, "c"))
        const first = await runQ(queue.lease(t, { batch: 2 }))
        expect(first.length).toBe(2)
        const second = await runQ(queue.lease(t, { batch: 2 }))
        expect(second.length).toBe(1)
        const ids = new Set([...first, ...second].map((job) => job.id))
        expect(ids.size).toBe(3)
      })
    })

    test("topics are isolated", async () => {
      const a = topic()
      const b = topic()
      await withQueue(async (queue) => {
        await runQ(queue.enqueue(a, "for-a"))
        const fromB = await runQ(queue.lease(b))
        expect(fromB).toEqual([])
        const fromA = await runQ(queue.lease(a))
        expect(fromA.length).toBe(1)
      })
    })

    test("delayMs defers visibility", async () => {
      const t = topic()
      await withQueue(async (queue) => {
        await runQ(queue.enqueue(t, "later", { delayMs: 300 }))
        expect(await runQ(queue.lease(t))).toEqual([])
        await sleep(450)
        const jobs = await runQ(queue.lease(t))
        expect(jobs.length).toBe(1)
      })
    })

    test("nack redelivers after retryDelayMs with attempts incremented", async () => {
      const t = topic()
      await withQueue(async (queue) => {
        await runQ(queue.enqueue(t, "retry-me"))
        const [first] = await runQ(queue.lease(t))
        expect(first?.attempts).toBe(1)
        await runQ(queue.nack((first as { id: string }).id, { retryDelayMs: 200 }))
        expect(await runQ(queue.lease(t))).toEqual([])
        await sleep(350)
        const [second] = await runQ(queue.lease(t))
        expect(second?.id).toBe(first?.id)
        expect(second?.attempts).toBe(2)
      })
    })

    test("exhausted attempts dead-letter the job", async () => {
      const t = topic()
      await withQueue(async (queue) => {
        const id = await runQ(queue.enqueue(t, "doomed", { maxAttempts: 2 }))
        for (let attempt = 1; attempt <= 2; attempt++) {
          const [job] = await runQ(queue.lease(t))
          expect(job?.id).toBe(id)
          expect(job?.attempts).toBe(attempt)
          await runQ(queue.nack(id, { error: `boom ${attempt}` }))
        }
        // Dead: not leasable, visible in deadLetters with the last error.
        expect(await runQ(queue.lease(t))).toEqual([])
        const dead = await runQ(queue.deadLetters(t))
        expect(dead.length).toBe(1)
        expect(dead[0]?.id).toBe(id)
        expect(dead[0]?.attempts).toBe(2)
        expect(dead[0]?.lastError).toBe("boom 2")
      })
    })

    test("a lapsed visibility window makes the job leasable again", async () => {
      const t = topic()
      await withQueue(async (queue) => {
        const id = await runQ(queue.enqueue(t, "slow-worker"))
        const [first] = await runQ(queue.lease(t, { visibilityMs: 250 }))
        expect(first?.id).toBe(id)
        // Still invisible while leased.
        expect(await runQ(queue.lease(t))).toEqual([])
        await sleep(400)
        const [second] = await runQ(queue.lease(t))
        expect(second?.id).toBe(id)
        expect(second?.attempts).toBe(2)
      })
    })

    test("ack/nack of an unknown or unleased job fails JobNotFoundError", async () => {
      const t = topic()
      await withQueue(async (queue) => {
        const ackExit = await Effect.runPromiseExit(queue.ack(crypto.randomUUID()))
        expect(ackExit._tag).toBe("Failure")
        const id = await runQ(queue.enqueue(t, "pending-only"))
        // Not leased yet -> nack must fail too.
        const nackExit = await Effect.runPromiseExit(queue.nack(id))
        expect(nackExit._tag).toBe("Failure")
        // Double-ack fails the second time.
        await runQ(queue.lease(t))
        await runQ(queue.ack(id))
        const doubleAck = await Effect.runPromiseExit(queue.ack(id))
        expect(doubleAck._tag).toBe("Failure")
      })
    })
  })
}
