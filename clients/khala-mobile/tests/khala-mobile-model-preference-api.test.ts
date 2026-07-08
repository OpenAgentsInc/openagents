import { describe, expect, test } from "bun:test"

import {
  fetchKhalaMobileModelPreference,
  putKhalaMobileModelPreference,
  type KhalaModelPreferenceFetchLike,
} from "../src/sync/khala-mobile-model-preference-api"

const samplePreference = {
  availableModelIds: ["gemini", "vertex-anthropic-claude"],
  availableTargetIds: ["gemini", "auto", "codex:owner-account"],
  effectiveModelId: "gemini",
  effectiveTargetId: "codex:owner-account",
  fallback: "no_preference_set" as const,
  preferredModelId: null,
  preferredTargetId: "codex:owner-account",
  updatedAt: null,
  usedPreference: false,
}

// CX-4 (#8548): `parsePreference` always fills these in (`null`/`[]`
// defaults) even when the wire body omits them — the never-undefined
// baseline every response-parsing test below expects.
const CX4_DEFAULTS = { autoResolution: null, claudeAccounts: [], codexAccounts: [] } as const

const fakeFetch = (response: { body?: unknown; ok: boolean; status?: number }): KhalaModelPreferenceFetchLike =>
  (async () => ({ json: async () => response.body ?? {}, ok: response.ok, status: response.status })) as KhalaModelPreferenceFetchLike

describe("fetchKhalaMobileModelPreference", () => {
  test("parses a successful GET response", async () => {
    const result = await fetchKhalaMobileModelPreference(
      "https://openagents.com",
      "tok",
      fakeFetch({ body: samplePreference, ok: true }),
    )
    expect(result).toEqual({ ok: true, value: { ...samplePreference, ...CX4_DEFAULTS } })
  })

  test("parses legacy model-only responses as target-compatible preferences", async () => {
    const legacyPreference = {
      availableModelIds: ["gemini"],
      effectiveModelId: "gemini",
      fallback: "none" as const,
      preferredModelId: "gemini",
      updatedAt: null,
      usedPreference: true,
    }
    const result = await fetchKhalaMobileModelPreference(
      "https://openagents.com",
      "tok",
      fakeFetch({ body: legacyPreference, ok: true }),
    )
    expect(result).toEqual({
      ok: true,
      value: {
        ...legacyPreference,
        ...CX4_DEFAULTS,
        availableTargetIds: ["gemini"],
        effectiveTargetId: "gemini",
        preferredTargetId: "gemini",
      },
    })
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
      return {
        json: async () => ({
          ...samplePreference,
          effectiveTargetId: "codex:owner-account",
          preferredTargetId: "codex:owner-account",
          usedPreference: true,
        }),
        ok: true,
      }
    }) as KhalaModelPreferenceFetchLike
    const result = await putKhalaMobileModelPreference("https://openagents.com", "tok", "codex:owner-account", fetchImpl)
    expect(capturedBody).toBe(JSON.stringify({ targetId: "codex:owner-account" }))
    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.value.preferredTargetId).toBe("codex:owner-account")
      expect(result.value.usedPreference).toBe(true)
    }
  })

  test("reports target_unavailable with available target and model lists on a 409", async () => {
    const result = await putKhalaMobileModelPreference(
      "https://openagents.com",
      "tok",
      "codex:missing",
      fakeFetch({
        body: {
          availableModelIds: ["gemini"],
          availableTargetIds: ["gemini", "auto"],
          error: "target_unavailable",
          targetId: "codex:missing",
        },
        ok: false,
        status: 409,
      }),
    )
    expect(result).toEqual({
      availableModelIds: ["gemini"],
      availableTargetIds: ["gemini", "auto"],
      kind: "target_unavailable",
      ok: false,
    })
  })

  test("reports bad_request on a 400", async () => {
    const result = await putKhalaMobileModelPreference(
      "https://openagents.com",
      "tok",
      "",
      fakeFetch({ body: { error: "bad_request", reason: "targetId is required" }, ok: false, status: 400 }),
    )
    expect(result).toEqual({ kind: "bad_request", ok: false })
  })
})
