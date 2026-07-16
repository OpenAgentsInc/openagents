import { describe, expect, test } from "vite-plus/test"

import { createGoogleCloudReleaseSetFeedStore } from "./release-set-gcs-store.ts"
import type { ReleaseSetPointer } from "./release-set-feed.ts"

type ObjectRow = { bytes: Uint8Array; generation: number }

const pointer = (revision: number, generation: string): ReleaseSetPointer => ({
  schema: "openagents.desktop.release_pointer.v2",
  channel: "rc",
  revision,
  generation,
  previousGeneration: null,
  payloadSha256: generation,
  signatureSha256: "b".repeat(64),
  publishedAt: "2026-07-16T18:00:00.000Z",
})

describe("Google Cloud ReleaseSet pointer store", () => {
  test("uses object-generation preconditions so only one promotion race wins", async () => {
    const objects = new Map<string, ObjectRow>()
    let nextGeneration = 1
    const fetchFn: typeof fetch = async (input, init) => {
      const url = new URL(String(input))
      if (url.pathname.startsWith("/upload/storage/v1/")) {
        const name = url.searchParams.get("name") ?? ""
        const expected = Number(url.searchParams.get("ifGenerationMatch"))
        const current = objects.get(name)
        if ((current?.generation ?? 0) !== expected) return new Response(null, { status: 412 })
        const claimedGeneration = nextGeneration++
        objects.set(name, { bytes: new Uint8Array(), generation: claimedGeneration })
        const bytes = new Uint8Array(await new Response(init?.body).arrayBuffer())
        objects.set(name, { bytes, generation: claimedGeneration })
        return Response.json({ ok: true })
      }
      const encodedName = url.pathname.split("/o/")[1]
      if (encodedName !== undefined) {
        const name = decodeURIComponent(encodedName)
        const current = objects.get(name)
        if (current === undefined) return new Response(null, { status: 404 })
        return new Response(Uint8Array.from(current.bytes).buffer, {
          headers: { "x-goog-generation": String(current.generation) },
        })
      }
      throw new Error(`unexpected fake GCS request: ${url}`)
    }
    const store = createGoogleCloudReleaseSetFeedStore({
      bucket: "openagents-release-test",
      fetch: fetchFn,
      token: async () => "fixture-token",
    })
    const first = pointer(1, "a".repeat(64))
    expect(await store.compareAndSwapPointer("rc", null, first)).toBe(true)
    expect(await store.readPointer("rc")).toEqual(first)

    const outcomes = await Promise.all([
      store.compareAndSwapPointer("rc", 1, pointer(2, "c".repeat(64))),
      store.compareAndSwapPointer("rc", 1, pointer(2, "d".repeat(64))),
    ])
    expect(outcomes.toSorted()).toEqual([false, true])
    expect((await store.readPointer("rc"))?.revision).toBe(2)
  })

  test("strictly rejects malformed/extra candidate documents and bounds every storage request", async () => {
    const seenSignals: AbortSignal[] = []
    const malformed = new TextEncoder().encode(JSON.stringify({
      schema: "openagents.desktop.release_candidate.v2",
      channel: "rc",
      generation: "a".repeat(64),
      payloadBase64: "e30=",
      signatureBase64: "e30=",
      unexpected: true,
    }))
    const store = createGoogleCloudReleaseSetFeedStore({
      bucket: "openagents-release-test",
      token: async () => "fixture-token",
      operationTimeoutMs: 25,
      fetch: async (_input, init) => {
        if (init?.signal) seenSignals.push(init.signal)
        return new Response(malformed, { headers: { "x-goog-generation": "1" } })
      },
    })
    await expect(store.readCandidate("rc", "a".repeat(64)))
      .rejects.toThrow("storage_candidate_invalid")
    expect(seenSignals).toHaveLength(1)
  })

  test("bounds credential acquisition before any GCS operation", async () => {
    const store = createGoogleCloudReleaseSetFeedStore({
      bucket: "openagents-release-test",
      operationTimeoutMs: 5,
      token: () => new Promise<string>(() => undefined),
      fetch: async () => { throw new Error("fetch must not run without credentials") },
    })
    await expect(store.readPointer("rc")).rejects.toThrow("storage_operation_timeout")
  })
})
