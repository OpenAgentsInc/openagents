import { describe, expect, test } from "bun:test"
import {
  CHAT_MESSAGE_READ_ROUTE_PATH,
  RUNTIME_INTENTS_ROUTE_PATH,
  fetchChatMessage,
  readPendingRuntimeIntents,
} from "./runtime-intents.js"

const okIntentsBody = {
  intents: [
    {
      createdAt: "2026-07-05T15:20:11.412Z",
      intent: {
        bodyRef: "chat_message.msg-1",
        causalityRefs: [],
        createdAt: "2026-07-05T15:20:11.412Z",
        idempotencyKey: "idem.intent-1",
        intentId: "intent-1",
        kind: "turn.start",
        origin: { lane: "khala_sync_mobile_control", surface: "mobile" },
        redactionClass: "private_ref",
        schema: "openagents.khala_runtime_control_intent.v1",
        target: { adapterKind: "codex", lane: "codex_app_server" },
        threadId: "thread-1",
        turnId: "turn-1",
        visibility: "private",
      },
      intentId: "intent-1",
      kind: "turn.start",
      ownerUserId: "user-1",
      seq: 11,
      status: "accepted",
      threadId: "thread-1",
      turnId: "turn-1",
      updatedAt: "2026-07-05T15:20:11.412Z",
    },
  ],
  nextAfter: 11,
  ok: true,
  routeRef: "route.internal.khala_sync.runtime_intents.v0_1",
  upToDate: true,
}

const fakeFetch = (
  handler: (url: string, init: RequestInit | undefined) => Response,
): { calls: Array<{ url: string; init: RequestInit | undefined }>; fetchImpl: typeof globalThis.fetch } => {
  const calls: Array<{ url: string; init: RequestInit | undefined }> = []
  const fetchImpl = ((input: string | URL | Request, init?: RequestInit) => {
    const url = typeof input === "string" ? input : input instanceof URL ? input.toString() : input.url
    calls.push({ init, url })
    return Promise.resolve(handler(url, init))
  }) as typeof globalThis.fetch
  return { calls, fetchImpl }
}

describe("readPendingRuntimeIntents (pylon poller)", () => {
  test("polls the internal route with the admin bearer and decodes typed rows", async () => {
    const { calls, fetchImpl } = fakeFetch(() =>
      new Response(JSON.stringify(okIntentsBody), {
        headers: { "content-type": "application/json" },
        status: 200,
      }),
    )
    const result = await readPendingRuntimeIntents({
      adminToken: "admin-secret",
      after: 10,
      baseUrl: "https://openagents.com",
      fetchImpl,
      limit: 50,
      ownerUserId: "user-1",
    })
    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.intents.map((i) => i.intentId)).toEqual(["intent-1"])
      expect(result.intents[0]!.kind).toBe("turn.start")
      expect(result.intents[0]!.intent.kind).toBe("turn.start")
      expect(result.nextAfter).toBe(11)
      expect(result.upToDate).toBe(true)
    }
    expect(calls).toHaveLength(1)
    const requested = new URL(calls[0]!.url)
    expect(requested.pathname).toBe(RUNTIME_INTENTS_ROUTE_PATH)
    expect(requested.searchParams.get("after")).toBe("10")
    expect(requested.searchParams.get("limit")).toBe("50")
    expect(requested.searchParams.get("ownerUserId")).toBe("user-1")
    expect(new Headers(calls[0]!.init?.headers).get("authorization")).toBe("Bearer admin-secret")
  })

  test("typed failures: 401 / 400 / 503 / non-JSON / enablement gap / bad shape", async () => {
    const cases: Array<{ response: Response; error: string }> = [
      { error: "unauthorized", response: new Response("{}", { status: 401 }) },
      { error: "invalid_request", response: new Response("{}", { status: 400 }) },
      { error: "storage_unavailable", response: new Response("{}", { status: 503 }) },
      { error: "bad_response", response: new Response("not json", { status: 200 }) },
      {
        error: "not_enabled",
        response: new Response(JSON.stringify({ ok: false, reason: "binding absent" }), { status: 200 }),
      },
      { error: "bad_response", response: new Response(JSON.stringify({ ok: true }), { status: 200 }) },
      {
        error: "bad_response",
        response: new Response(
          JSON.stringify({ intents: [{ seq: "not-a-row" }], nextAfter: 1, ok: true }),
          { status: 200 },
        ),
      },
    ]
    for (const item of cases) {
      const { fetchImpl } = fakeFetch(() => item.response)
      const result = await readPendingRuntimeIntents({
        adminToken: "admin-secret",
        baseUrl: "https://openagents.com",
        fetchImpl,
      })
      expect(result.ok).toBe(false)
      if (!result.ok) {
        expect(result.error).toBe(item.error as never)
        expect(JSON.stringify(result)).not.toContain("admin-secret")
      }
    }
  })

  test("network failure never throws and never leaks the token", async () => {
    const fetchImpl = (() => Promise.reject(new Error("connect ECONNREFUSED"))) as unknown as typeof globalThis.fetch
    const result = await readPendingRuntimeIntents({
      adminToken: "admin-secret",
      baseUrl: "https://openagents.com",
      fetchImpl,
    })
    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.error).toBe("network_failed")
      expect(JSON.stringify(result)).not.toContain("admin-secret")
    }
  })
})

describe("fetchChatMessage (pylon prompt resolver)", () => {
  test("resolves a real message body against the internal route", async () => {
    const { calls, fetchImpl } = fakeFetch(() =>
      new Response(
        JSON.stringify({
          message: {
            authorUserId: "user-1",
            body: "the real prompt",
            createdAt: "2026-07-05T12:00:00.000Z",
            deletedAt: null,
            messageId: "msg-1",
            threadId: "thread-1",
            updatedAt: "2026-07-05T12:00:00.000Z",
          },
          ok: true,
        }),
        { status: 200 },
      ),
    )
    const result = await fetchChatMessage({
      adminToken: "admin-secret",
      baseUrl: "https://openagents.com",
      fetchImpl,
      messageId: "msg-1",
      threadId: "thread-1",
    })
    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.message?.body).toBe("the real prompt")
    }
    const requested = new URL(calls[0]!.url)
    expect(requested.pathname).toBe(CHAT_MESSAGE_READ_ROUTE_PATH)
    expect(requested.searchParams.get("threadId")).toBe("thread-1")
    expect(requested.searchParams.get("messageId")).toBe("msg-1")
  })

  test("a real ok:true, message:null response is a genuine 'not found', never thrown", async () => {
    const { fetchImpl } = fakeFetch(() => new Response(JSON.stringify({ message: null, ok: true }), { status: 200 }))
    const result = await fetchChatMessage({
      adminToken: "admin-secret",
      baseUrl: "https://openagents.com",
      fetchImpl,
      messageId: "msg-missing",
      threadId: "thread-1",
    })
    expect(result.ok).toBe(true)
    if (result.ok) expect(result.message).toBeNull()
  })

  test("network failure never throws and never leaks the token", async () => {
    const fetchImpl = (() => Promise.reject(new Error("connect ECONNREFUSED"))) as unknown as typeof globalThis.fetch
    const result = await fetchChatMessage({
      adminToken: "admin-secret",
      baseUrl: "https://openagents.com",
      fetchImpl,
      messageId: "msg-1",
      threadId: "thread-1",
    })
    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.error).toBe("network_failed")
      expect(JSON.stringify(result)).not.toContain("admin-secret")
    }
  })
})
