/**
 * KvStore conformance suite (CFG-2, issue #8517).
 *
 * EVERY KvStore backend must pass this suite unmodified — that is the
 * hot-swap guarantee from the consolidation audit §5. Backends register by
 * calling `runKvStoreConformance` from a bun test file.
 *
 * Tests namespace their keys with a fresh UUID so backends with shared
 * state (one Postgres database) can run the suite without cross-test
 * interference.
 */
import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import type { Layer } from "effect"
import { KvStore } from "../kv-store.ts"

export interface KvStoreConformanceOptions {
  /** Backend label, e.g. "memory", "postgres". */
  readonly label: string
  /** Called at TEST time (after any beforeAll infra setup). */
  readonly makeLayer: () => Layer.Layer<KvStore>
  readonly skip?: boolean
}

export const runKvStoreConformance = (options: KvStoreConformanceOptions): void => {
  const suite = options.skip === true ? describe.skip : describe
  const run = <A, E>(effect: Effect.Effect<A, E, KvStore>): Promise<A> =>
    Effect.runPromise(Effect.provide(effect, options.makeLayer()))
  const ns = () => `kv-conf-${crypto.randomUUID()}`

  suite(`KvStore conformance [${options.label}]`, () => {
    test("get of a missing key is null", async () => {
      const key = `${ns()}/missing`
      const value = await run(Effect.gen(function* () {
        const kv = yield* KvStore
        return yield* kv.get(key)
      }))
      expect(value).toBeNull()
    })

    test("put then get round-trips; put overwrites", async () => {
      const key = `${ns()}/roundtrip`
      const [first, second] = await run(Effect.gen(function* () {
        const kv = yield* KvStore
        yield* kv.put(key, "one")
        const a = yield* kv.get(key)
        yield* kv.put(key, "two")
        const b = yield* kv.get(key)
        return [a, b] as const
      }))
      expect(first).toBe("one")
      expect(second).toBe("two")
    })

    test("delete removes the key and is idempotent on missing keys", async () => {
      const key = `${ns()}/delete`
      const after = await run(Effect.gen(function* () {
        const kv = yield* KvStore
        yield* kv.put(key, "value")
        yield* kv.delete(key)
        yield* kv.delete(key) // idempotent
        yield* kv.delete(`${key}/never-existed`) // idempotent
        return yield* kv.get(key)
      }))
      expect(after).toBeNull()
    })

    test("TTL: readable before expiry, null after", async () => {
      const key = `${ns()}/ttl`
      await run(Effect.gen(function* () {
        const kv = yield* KvStore
        yield* kv.put(key, "ephemeral", { ttlMs: 250 })
        const before = yield* kv.get(key)
        expect(before).toBe("ephemeral")
        yield* Effect.promise(() => new Promise((resolve) => setTimeout(resolve, 400)))
        const after = yield* kv.get(key)
        expect(after).toBeNull()
      }))
    })

    test("no TTL means no expiry; re-put without TTL clears a previous TTL", async () => {
      const key = `${ns()}/no-ttl`
      await run(Effect.gen(function* () {
        const kv = yield* KvStore
        yield* kv.put(key, "short-lived", { ttlMs: 200 })
        yield* kv.put(key, "permanent") // overwrite drops the TTL
        yield* Effect.promise(() => new Promise((resolve) => setTimeout(resolve, 350)))
        const value = yield* kv.get(key)
        expect(value).toBe("permanent")
      }))
    })

    test("listPrefix: matching non-expired entries only, ordered by key", async () => {
      const base = ns()
      const entries = await run(Effect.gen(function* () {
        const kv = yield* KvStore
        yield* kv.put(`${base}/scan/b`, "two")
        yield* kv.put(`${base}/scan/a`, "one")
        yield* kv.put(`${base}/scan/c`, "gone", { ttlMs: 100 })
        yield* kv.put(`${base}/other/x`, "outside")
        yield* Effect.promise(() => new Promise((resolve) => setTimeout(resolve, 250)))
        return yield* kv.listPrefix(`${base}/scan/`)
      }))
      expect(entries).toEqual([
        { key: `${base}/scan/a`, value: "one" },
        { key: `${base}/scan/b`, value: "two" },
      ])
    })

    test("listPrefix: empty for a prefix with no matches", async () => {
      const entries = await run(Effect.gen(function* () {
        const kv = yield* KvStore
        return yield* kv.listPrefix(`${ns()}/never-written/`)
      }))
      expect(entries).toEqual([])
    })

    test("listPrefix: the prefix is literal — %, _ and \\ never pattern-match", async () => {
      const base = ns()
      const [percent, underscore] = await run(Effect.gen(function* () {
        const kv = yield* KvStore
        yield* kv.put(`${base}/p%q/1`, "literal-percent")
        yield* kv.put(`${base}/pXXq/1`, "would-match-%-as-pattern")
        yield* kv.put(`${base}/u_v/1`, "literal-underscore")
        yield* kv.put(`${base}/uXv/1`, "would-match-_-as-pattern")
        const a = yield* kv.listPrefix(`${base}/p%q/`)
        const b = yield* kv.listPrefix(`${base}/u_v/`)
        return [a, b] as const
      }))
      expect(percent).toEqual([{ key: `${base}/p%q/1`, value: "literal-percent" }])
      expect(underscore).toEqual([{ key: `${base}/u_v/1`, value: "literal-underscore" }])
    })
  })
}
