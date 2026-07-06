import { describe, expect, test } from "bun:test"

import {
  fetchKhalaMobileCreditsBalance,
  fetchKhalaMobileCreditsTransactions,
  type KhalaMobileCreditsFetchLike,
} from "../src/sync/khala-mobile-credits-api"

const fakeFetch = (response: { body?: unknown; ok: boolean; status?: number }): KhalaMobileCreditsFetchLike =>
  (async () => ({ json: async () => response.body ?? {}, ok: response.ok, status: response.status })) as KhalaMobileCreditsFetchLike

describe("fetchKhalaMobileCreditsBalance", () => {
  test("parses a successful balance response", async () => {
    const result = await fetchKhalaMobileCreditsBalance(
      "https://openagents.com",
      "tok",
      fakeFetch({ body: { balanceUsdCents: 1000 }, ok: true }),
    )
    expect(result).toEqual({ ok: true, value: 1000 })
  })

  test("reports unavailable on a 404 (endpoint not built yet)", async () => {
    const result = await fetchKhalaMobileCreditsBalance(
      "https://openagents.com",
      "tok",
      fakeFetch({ ok: false, status: 404 }),
    )
    expect(result).toEqual({ kind: "unavailable", ok: false })
  })

  test("reports unauthorized on a 401", async () => {
    const result = await fetchKhalaMobileCreditsBalance(
      "https://openagents.com",
      "tok",
      fakeFetch({ body: { error: "unauthorized" }, ok: false, status: 401 }),
    )
    expect(result).toEqual({ kind: "unauthorized", ok: false })
  })

  test("reports unavailable when the network call throws", async () => {
    const throwingFetch: KhalaMobileCreditsFetchLike = (async () => {
      throw new Error("network unavailable")
    }) as KhalaMobileCreditsFetchLike
    const result = await fetchKhalaMobileCreditsBalance("https://openagents.com", "tok", throwingFetch)
    expect(result).toEqual({ kind: "unavailable", ok: false })
  })

  test("reports unknown for a malformed 200 body", async () => {
    const result = await fetchKhalaMobileCreditsBalance(
      "https://openagents.com",
      "tok",
      fakeFetch({ body: { balanceUsdCents: "ten dollars" }, ok: true }),
    )
    expect(result).toEqual({ kind: "unknown", ok: false })
  })
})

describe("fetchKhalaMobileCreditsTransactions", () => {
  const sampleTransaction = {
    amountUsdCents: 1000,
    description: "GitHub signup credit",
    id: "txn_1",
    kind: "grant" as const,
    occurredAt: "2026-07-06T00:00:00.000Z",
  }

  test("parses a successful transactions page", async () => {
    const result = await fetchKhalaMobileCreditsTransactions(
      "https://openagents.com",
      "tok",
      {},
      fakeFetch({ body: { nextCursor: null, transactions: [sampleTransaction] }, ok: true }),
    )
    expect(result).toEqual({ ok: true, value: { nextCursor: null, transactions: [sampleTransaction] } })
  })

  test("reports unavailable on a 404", async () => {
    const result = await fetchKhalaMobileCreditsTransactions(
      "https://openagents.com",
      "tok",
      {},
      fakeFetch({ ok: false, status: 404 }),
    )
    expect(result).toEqual({ kind: "unavailable", ok: false })
  })

  test("builds the request URL with limit/cursor params", async () => {
    let capturedUrl = ""
    const fetchImpl: KhalaMobileCreditsFetchLike = (async (url: string) => {
      capturedUrl = url
      return { json: async () => ({ nextCursor: null, transactions: [] }), ok: true }
    }) as KhalaMobileCreditsFetchLike
    await fetchKhalaMobileCreditsTransactions("https://openagents.com", "tok", { cursor: "abc", limit: 20 }, fetchImpl)
    expect(capturedUrl).toBe("https://openagents.com/api/mobile/credits/transactions?limit=20&cursor=abc")
  })

  test("rejects a page containing a malformed transaction entry", async () => {
    const result = await fetchKhalaMobileCreditsTransactions(
      "https://openagents.com",
      "tok",
      {},
      fakeFetch({ body: { nextCursor: null, transactions: [{ id: "txn_1" }] }, ok: true }),
    )
    expect(result).toEqual({ kind: "unknown", ok: false })
  })
})
