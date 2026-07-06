import { describe, expect, test } from "bun:test"
import { decodeKhalaRuntimeEvent } from "@openagentsinc/khala-sync"
import type { KhalaRuntimeEvent } from "@openagentsinc/khala-sync"

import {
  KHALA_CLOUD_RUNTIME_USAGE_INGEST_PATH,
  KHALA_CLOUD_RUNTIME_USAGE_SCHEMA_VERSION,
  recordRuntimeTurnUsageReceipt,
} from "./runtime-turn-usage-receipts.js"

const event = decodeKhalaRuntimeEvent({
  causalityRefs: [],
  eventId: "event.runtime.usage.1",
  kind: "usage.recorded",
  observedAt: "2026-07-06T12:00:00.000Z",
  redactionClass: "private_ref",
  schema: "openagents.khala_runtime_event.v1",
  sequence: 1,
  source: {
    lane: "hosted_khala",
    modelRef: "gemini-3.5-flash",
    providerRef: "vertex-gemini",
    surface: "server",
  },
  threadId: "thread-1",
  turnId: "turn-1",
  usage: {
    cacheReadInputTokens: 2,
    inputTokens: 10,
    outputTokens: 5,
    reasoningTokens: 3,
    totalTokens: 18,
    usageRef: "usage.runtime.1",
  },
  visibility: "private",
}) as Extract<KhalaRuntimeEvent, { kind: "usage.recorded" }>

describe("recordRuntimeTurnUsageReceipt", () => {
  test("posts the exact usage.recorded event to the cloud runtime usage route", async () => {
    const requests: Array<{ body: unknown; headers: Headers; url: string }> = []
    const fetchImpl: typeof globalThis.fetch = Object.assign(
      async (
        url: Parameters<typeof globalThis.fetch>[0],
        init?: Parameters<typeof globalThis.fetch>[1],
      ) => {
        requests.push({
          body: JSON.parse(String(init?.body)),
          headers: new Headers(init?.headers),
          url: String(url),
        })
        return new Response(
          JSON.stringify({
            insertedTokenUsage: true,
            tokenUsageEventRef: "event.inference.served-tokens.khala-cloud-runtime.1",
            tokensServedDelta: 18,
          }),
          { status: 200 },
        )
      },
      { preconnect: globalThis.fetch.preconnect },
    )
    const result = await recordRuntimeTurnUsageReceipt({
      agentToken: "oa_agent_fixture",
      baseUrl: "https://openagents.com",
      event,
      fetchImpl,
      lane: "hosted_khala",
      ownerUserId: "user-owner-1",
      provider: {
        backendProfile: "vertex-gemini",
        model: "gemini-3.5-flash",
        provider: "vertex-gemini",
      },
      pylonRef: "pylon.org-cloud.1",
      threadId: "thread-1",
      turnId: "turn-1",
    })

    expect(result).toEqual({
      insertedTokenUsage: true,
      ok: true,
      tokenUsageEventRef: "event.inference.served-tokens.khala-cloud-runtime.1",
      tokensServedDelta: 18,
    })
    expect(requests).toHaveLength(1)
    expect(new URL(requests[0]!.url).pathname).toBe(
      KHALA_CLOUD_RUNTIME_USAGE_INGEST_PATH,
    )
    expect(requests[0]!.headers.get("authorization")).toBe(
      "Bearer oa_agent_fixture",
    )
    expect(requests[0]!.body).toMatchObject({
      schemaVersion: KHALA_CLOUD_RUNTIME_USAGE_SCHEMA_VERSION,
      lane: "hosted_khala",
      model: "gemini-3.5-flash",
      ownerUserId: "user-owner-1",
      provider: "vertex-gemini",
      threadId: "thread-1",
      turnId: "turn-1",
      usage: {
        inputTokens: 10,
        outputTokens: 5,
        reasoningTokens: 3,
        totalTokens: 18,
        usageRef: "usage.runtime.1",
      },
    })
  })

  test("returns typed failures for validation errors", async () => {
    const fetchImpl: typeof globalThis.fetch = Object.assign(
      async () =>
        new Response(
          JSON.stringify({
            error: "khala_cloud_runtime_validation_error",
            reason: "usage must be exact",
          }),
          { status: 400 },
        ),
      { preconnect: globalThis.fetch.preconnect },
    )
    const result = await recordRuntimeTurnUsageReceipt({
      agentToken: "oa_agent_fixture",
      baseUrl: "https://openagents.com",
      event,
      fetchImpl,
      lane: "hosted_khala",
      ownerUserId: "user-owner-1",
      provider: {
        backendProfile: "vertex-gemini",
        model: "gemini-3.5-flash",
        provider: "vertex-gemini",
      },
      pylonRef: "pylon.org-cloud.1",
      threadId: "thread-1",
      turnId: "turn-1",
    })

    expect(result).toEqual({
      error: "validation_failed",
      ok: false,
      reason: "usage must be exact",
      status: 400,
    })
  })
})
