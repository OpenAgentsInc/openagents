/**
 * BlobStore conformance suite (CFG-2, issue #8517).
 *
 * EVERY BlobStore backend must pass this suite unmodified (audit §5
 * hot-swap guarantee). Keys are namespaced per test with a fresh UUID so
 * shared buckets can host the suite.
 */
import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import type { Layer } from "effect"
import { BlobStore } from "../blob-store.ts"

export interface BlobStoreConformanceOptions {
  readonly label: string
  /** Called at TEST time (after any beforeAll infra setup). */
  readonly makeLayer: () => Layer.Layer<BlobStore>
  readonly skip?: boolean
}

const bytes = (text: string): Uint8Array => new TextEncoder().encode(text)
const text = (data: Uint8Array): string => new TextDecoder().decode(data)

export const runBlobStoreConformance = (options: BlobStoreConformanceOptions): void => {
  const suite = options.skip === true ? describe.skip : describe
  const run = <A, E>(effect: Effect.Effect<A, E, BlobStore>): Promise<A> =>
    Effect.runPromise(Effect.provide(effect, options.makeLayer()))
  const ns = () => `blob-conf-${crypto.randomUUID()}`

  suite(`BlobStore conformance [${options.label}]`, () => {
    test("get of a missing key is null", async () => {
      const result = await run(Effect.gen(function* () {
        const blobs = yield* BlobStore
        return yield* blobs.get(`${ns()}/missing`)
      }))
      expect(result).toBeNull()
    })

    test("put then get round-trips bytes; put overwrites", async () => {
      const key = `${ns()}/roundtrip`
      const [first, second] = await run(Effect.gen(function* () {
        const blobs = yield* BlobStore
        yield* blobs.put(key, bytes("payload one"), { contentType: "text/plain" })
        const a = yield* blobs.get(key)
        yield* blobs.put(key, bytes("payload two"))
        const b = yield* blobs.get(key)
        return [a, b] as const
      }))
      expect(first).not.toBeNull()
      expect(text(first as Uint8Array)).toBe("payload one")
      expect(text(second as Uint8Array)).toBe("payload two")
    })

    test("delete removes the blob and is idempotent on missing keys", async () => {
      const key = `${ns()}/delete`
      const after = await run(Effect.gen(function* () {
        const blobs = yield* BlobStore
        yield* blobs.put(key, bytes("value"))
        yield* blobs.delete(key)
        yield* blobs.delete(key) // idempotent
        yield* blobs.delete(`${key}/never-existed`) // idempotent
        return yield* blobs.get(key)
      }))
      expect(after).toBeNull()
    })

    test("list(prefix) returns exactly the matching keys, sorted", async () => {
      const prefix = `${ns()}/list/`
      const listed = await run(Effect.gen(function* () {
        const blobs = yield* BlobStore
        yield* blobs.put(`${prefix}b`, bytes("b"))
        yield* blobs.put(`${prefix}a`, bytes("a"))
        yield* blobs.put(`${prefix}nested/c`, bytes("c"))
        yield* blobs.put(`${prefix.slice(0, -1)}-outside`, bytes("x"))
        return yield* blobs.list(prefix)
      }))
      expect(listed).toEqual([`${prefix}a`, `${prefix}b`, `${prefix}nested/c`])
    })

    test("list of an unused prefix is empty", async () => {
      const listed = await run(Effect.gen(function* () {
        const blobs = yield* BlobStore
        return yield* blobs.list(`${ns()}/empty/`)
      }))
      expect(listed).toEqual([])
    })

    test("signedUrl returns a URL for the key", async () => {
      const key = `${ns()}/signed`
      const [readUrl, writeUrl] = await run(Effect.gen(function* () {
        const blobs = yield* BlobStore
        yield* blobs.put(key, bytes("content"))
        const r = yield* blobs.signedUrl(key, { expiresInMs: 60_000 })
        const w = yield* blobs.signedUrl(key, { expiresInMs: 60_000, method: "PUT" })
        return [r, w] as const
      }))
      for (const url of [readUrl, writeUrl]) {
        expect(url.length).toBeGreaterThan(0)
        // Must be an absolute URL (any scheme; memory backend uses memory://).
        expect(() => new URL(url)).not.toThrow()
      }
    })
  })
}
