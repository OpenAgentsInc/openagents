import { describe, expect, test } from "bun:test"
import { electWriter, type LockManagerLike } from "./election.js"

/**
 * Web Locks single-writer election (KS-5.4) against a faked LockManager
 * implementing the relevant Web Locks contract: exclusive named locks,
 * FIFO grant queueing, callback-promise-lifetime holds, and AbortSignal
 * withdrawal of pending requests.
 */

interface Waiter {
  readonly callback: (lock: unknown) => Promise<unknown> | unknown
  readonly resolve: (value: unknown) => void
  readonly reject: (error: unknown) => void
  readonly signal: AbortSignal | undefined
}

const createFakeLocks = (): LockManagerLike & {
  readonly heldCount: (name: string) => number
} => {
  const held = new Map<string, boolean>()
  const queues = new Map<string, Array<Waiter>>()

  const tryGrant = (name: string): void => {
    if (held.get(name) === true) return
    const queue = queues.get(name) ?? []
    while (queue.length > 0) {
      const waiter = queue.shift()!
      if (waiter.signal?.aborted === true) continue // already rejected
      held.set(name, true)
      void Promise.resolve()
        .then(() => waiter.callback({ name }))
        .then(
          (value) => {
            held.set(name, false)
            waiter.resolve(value)
            tryGrant(name)
          },
          (error: unknown) => {
            held.set(name, false)
            waiter.reject(error)
            tryGrant(name)
          },
        )
      return
    }
  }

  return {
    heldCount: (name) => (held.get(name) === true ? 1 : 0),
    request: (name, options, callback) =>
      new Promise((resolve, reject) => {
        const waiter: Waiter = {
          callback,
          resolve,
          reject,
          signal: options.signal,
        }
        options.signal?.addEventListener("abort", () => {
          const queue = queues.get(name)
          const index = queue?.indexOf(waiter) ?? -1
          if (queue !== undefined && index >= 0) {
            queue.splice(index, 1)
            reject(new Error("aborted"))
          }
        })
        const queue = queues.get(name)
        if (queue === undefined) queues.set(name, [waiter])
        else queue.push(waiter)
        tryGrant(name)
      }),
  }
}

const settle = () => new Promise<void>((resolve) => setTimeout(resolve, 0))

describe("electWriter (Web Locks single-writer election)", () => {
  test("a free lock elects the requester immediately", async () => {
    const locks = createFakeLocks()
    const election = electWriter(locks, "khala-sync:writer")
    expect(await election.becameWriter).toBe(true)
    expect(election.isWriter()).toBe(true)
    expect(locks.heldCount("khala-sync:writer")).toBe(1)
  })

  test("contenders queue; releasing the writer elects the next tab", async () => {
    const locks = createFakeLocks()
    const first = electWriter(locks, "khala-sync:writer")
    const second = electWriter(locks, "khala-sync:writer")
    expect(await first.becameWriter).toBe(true)

    // second is queued, not elected
    let secondElected = false
    void second.becameWriter.then((won) => {
      secondElected = won
    })
    await settle()
    expect(secondElected).toBe(false)
    expect(second.isWriter()).toBe(false)

    // writer-tab death/close = lock release → automatic handover
    first.release()
    await settle()
    expect(first.isWriter()).toBe(false)
    expect(await second.becameWriter).toBe(true)
    expect(second.isWriter()).toBe(true)
  })

  test("release before the grant withdraws the request (becameWriter false)", async () => {
    const locks = createFakeLocks()
    const holder = electWriter(locks, "khala-sync:writer")
    expect(await holder.becameWriter).toBe(true)

    const pending = electWriter(locks, "khala-sync:writer")
    pending.release()
    expect(await pending.becameWriter).toBe(false)
    expect(pending.isWriter()).toBe(false)

    // the withdrawn request must not block the next contender
    const next = electWriter(locks, "khala-sync:writer")
    holder.release()
    await settle()
    expect(await next.becameWriter).toBe(true)
  })

  test("release is idempotent and clears isWriter", async () => {
    const locks = createFakeLocks()
    const election = electWriter(locks, "khala-sync:writer")
    expect(await election.becameWriter).toBe(true)
    election.release()
    election.release()
    expect(election.isWriter()).toBe(false)
    await settle()
    expect(locks.heldCount("khala-sync:writer")).toBe(0)
  })
})
