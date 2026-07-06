import { describe, expect, test } from "bun:test"

import {
  fetchKhalaMobileModelPreference,
  putKhalaMobileModelPreference,
  type KhalaModelPreferenceFetchLike,
} from "../src/sync/khala-mobile-model-preference-api"

const samplePreference = {
  availableModelIds: ["gemini", "vertex-anthropic-claude"],
  effectiveModelId: "gemini",
  fallback: "no_preference_set" as const,
  preferredModelId: null,
  updatedAt: null,
  usedPreference: false,
}

const fakeFetch = (response: { body?: unknown; ok: boolean; status?: number }): KhalaModelPreferenceFetchLike =>
  (async () => ({ json: async () => response.body ?? {}, ok: response.ok, status: response.status })) as KhalaModelPreferenceFetchLike

describe("fetchKhalaMobileModelPreference", () => {
  test("parses a successful GET response", async () => {
    const result = await fetchKhalaMobileModelPreference(
      "https://openagents.com",
      "tok",
      fakeFetch({ body: samplePreference, ok: true }),
    )
    expect(result).toEqual({ ok: true, value: samplePreference })
  })

  test("reports unauthorized on a 401", async () => {
    const result = await fetchKhalaMobileModelPreference(
      "https://openagents.com",
      "tok",
      fakeFetch({ body: { error: "unauthorized" }, ok: false, status: 401 }),
    )
    expect(result).toEqual({ kind: "unauthorized", ok: false })
  })

  test("reports unavailable when the network call throws", async () => {
    const throwingFetch: KhalaModelPreferenceFetchLike = (async () => {
      throw new Error("network unavailable")
    }) as KhalaModelPreferenceFetchLike
    const result = await fetchKhalaMobileModelPreference("https://openagents.com", "tok", throwingFetch)
    expect(result).toEqual({ kind: "unavailable", ok: false })
  })
})

describe("putKhalaMobileModelPreference", () => {
  test("parses a successful PUT response", async () => {
    let capturedBody = ""
    const fetchImpl: KhalaModelPreferenceFetchLike = (async (_url, init) => {
      capturedBody = init.body ?? ""
      return { json: async () => ({ ...samplePreference, preferredModelId: "gemini", usedPreference: true }), ok: true }
    }) as KhalaModelPreferenceFetchLike
    const result = await putKhalaMobileModelPreference("https://openagents.com", "tok", "gemini", fetchImpl)
    expect(capturedBody).toBe(JSON.stringify({ modelId: "gemini" }))
    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.value.preferredModelId).toBe("gemini")
      expect(result.value.usedPreference).toBe(true)
    }
  })

  test("reports model_unavailable with the available list on a 409", async () => {
    const result = await putKhalaMobileModelPreference(
      "https://openagents.com",
      "tok",
      "nonexistent-model",
      fakeFetch({
        body: { availableModelIds: ["gemini"], error: "model_unavailable", modelId: "nonexistent-model" },
        ok: false,
        status: 409,
      }),
    )
    expect(result).toEqual({ availableModelIds: ["gemini"], kind: "model_unavailable", ok: false })
  })

  test("reports bad_request on a 400", async () => {
    const result = await putKhalaMobileModelPreference(
      "https://openagents.com",
      "tok",
      "",
      fakeFetch({ body: { error: "bad_request", reason: "modelId is required" }, ok: false, status: 400 }),
    )
    expect(result).toEqual({ kind: "bad_request", ok: false })
  })
})
