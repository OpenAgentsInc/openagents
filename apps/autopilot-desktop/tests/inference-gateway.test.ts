import { describe, expect, test } from "bun:test"

import { buildInferenceGatewayReadiness } from "../src/bun/inference-gateway"
import {
  inferenceGatewayChatCompletionsUrl,
  inferenceGatewayCreditsUrl,
  resolveInferenceGatewaySettings,
} from "../src/shared/inference-gateway"

// #5485 (EPIC #5474): the gateway settings resolver + the Bun-host credit read.

describe("resolveInferenceGatewaySettings — INERT default + flag gate", () => {
  test("defaults to disabled with the openagents.com base + no key (INERT)", () => {
    const s = resolveInferenceGatewaySettings({})
    expect(s.enabled).toBe(false)
    expect(s.apiKeyPresent).toBe(false)
    expect(s.baseUrl).toBe("https://openagents.com")
    expect(s.lowBalanceThreshold).toBe(1)
  })

  test("flips on with the server flag + API key, honouring base/model/threshold overrides", () => {
    const s = resolveInferenceGatewaySettings({
      OPENAGENTS_INFERENCE_GATEWAY_ENABLED: "1",
      OPENAGENTS_INFERENCE_API_KEY: "sk-test",
      OPENAGENTS_INFERENCE_GATEWAY_BASE_URL: "https://gw.test",
      OPENAGENTS_INFERENCE_GATEWAY_MODEL: "oa-fast",
      OPENAGENTS_INFERENCE_GATEWAY_LOW_BALANCE: "5",
    })
    expect(s.enabled).toBe(true)
    expect(s.apiKeyPresent).toBe(true)
    expect(s.baseUrl).toBe("https://gw.test")
    expect(s.model).toBe("oa-fast")
    expect(s.lowBalanceThreshold).toBe(5)
  })

  test("the OpenAI-compatible endpoints are derived from the base url", () => {
    expect(inferenceGatewayChatCompletionsUrl("https://gw.test/")).toBe(
      "https://gw.test/api/v1/chat/completions",
    )
    expect(inferenceGatewayCreditsUrl("https://gw.test")).toBe(
      "https://gw.test/v1/credits",
    )
  })
})

const fixedNow = () => "2026-06-19T00:00:00.000Z"

describe("buildInferenceGatewayReadiness — INERT-safe network behaviour", () => {
  test("flag off: NO network call, enabled:false, disabled blocker", async () => {
    let called = false
    const fetchFn = (() => {
      called = true
      return Promise.resolve(new Response("{}"))
    }) as unknown as typeof fetch
    const r = await buildInferenceGatewayReadiness({
      env: {},
      apiKey: "sk-test",
      fetchFn,
      nowIso: fixedNow,
    })
    expect(called).toBe(false)
    expect(r.ok).toBe(true)
    expect(r.enabled).toBe(false)
    expect(r.creditBalance).toBe(null)
    expect(r.blockerRefs).toContain("blocker.inference.gateway_disabled")
  })

  test("flag on but NO key: no network call, key-missing blocker, apiKeyPresent:false", async () => {
    let called = false
    const fetchFn = (() => {
      called = true
      return Promise.resolve(new Response("{}"))
    }) as unknown as typeof fetch
    const r = await buildInferenceGatewayReadiness({
      env: { OPENAGENTS_INFERENCE_GATEWAY_ENABLED: "1" },
      apiKey: null,
      fetchFn,
      nowIso: fixedNow,
    })
    expect(called).toBe(false)
    expect(r.apiKeyPresent).toBe(false)
    expect(r.blockerRefs).toContain("blocker.inference.gateway_api_key_missing")
  })

  test("flag on + key: fetches credits with a Bearer header and projects the balance", async () => {
    let seenAuth: string | null = null
    let seenUrl: string | null = null
    const fetchFn = ((url: string, init?: RequestInit) => {
      seenUrl = url
      seenAuth =
        (init?.headers as Record<string, string> | undefined)?.authorization ??
        null
      return Promise.resolve(
        new Response(JSON.stringify({ creditBalance: 7 }), { status: 200 }),
      )
    }) as unknown as typeof fetch
    const r = await buildInferenceGatewayReadiness({
      env: {
        OPENAGENTS_INFERENCE_GATEWAY_ENABLED: "1",
        OPENAGENTS_INFERENCE_GATEWAY_BASE_URL: "https://gw.test",
      },
      apiKey: "sk-secret",
      fetchFn,
      nowIso: fixedNow,
    })
    expect(seenUrl).toBe("https://gw.test/v1/credits")
    // The raw key is only ever placed in the outbound Authorization header.
    expect(seenAuth).toBe("Bearer sk-secret")
    expect(r.ok).toBe(true)
    expect(r.enabled).toBe(true)
    expect(r.apiKeyPresent).toBe(true)
    expect(r.creditBalance).toBe(7)
    // The raw key NEVER appears in the public-safe projection.
    expect(JSON.stringify(r)).not.toContain("sk-secret")
  })

  test("zero balance surfaces the out-of-credits blocker", async () => {
    const fetchFn = (() =>
      Promise.resolve(
        new Response(JSON.stringify({ balance: 0 }), { status: 200 }),
      )) as unknown as typeof fetch
    const r = await buildInferenceGatewayReadiness({
      env: { OPENAGENTS_INFERENCE_GATEWAY_ENABLED: "1" },
      apiKey: "sk-test",
      fetchFn,
      nowIso: fixedNow,
    })
    expect(r.creditBalance).toBe(0)
    expect(r.blockerRefs).toContain("blocker.inference.gateway_out_of_credits")
  })

  test("a credits fetch failure degrades to unknown balance + a not-ok projection", async () => {
    const fetchFn = (() =>
      Promise.reject(new Error("network down"))) as unknown as typeof fetch
    const r = await buildInferenceGatewayReadiness({
      env: { OPENAGENTS_INFERENCE_GATEWAY_ENABLED: "1" },
      apiKey: "sk-test",
      fetchFn,
      nowIso: fixedNow,
    })
    expect(r.ok).toBe(false)
    expect(r.creditBalance).toBe(null)
    expect(r.blockerRefs).toContain(
      "blocker.inference.gateway_credits_unavailable",
    )
  })
})
