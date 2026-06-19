// #5493 live streaming: the bridge `session.subscribe` cursor read the mobile
// Sessions list + session-detail screens use to stream live state instead of
// polling. These tests mock global fetch so they run without a node or device.

import { afterEach, describe, expect, test } from "bun:test"

import { createBridgeTransport } from "@openagentsinc/autopilot-control-protocol"
import { sessionEventStreamFixture } from "@openagentsinc/autopilot-control-protocol/fixtures"

import { fetchSessionEventBatchViaBridge, type BridgeSession } from "./control-client"

const realFetch = globalThis.fetch

function bridge(baseUrl = "http://100.1.2.3:4716"): BridgeSession {
  const credential = { pairingRef: "pair.1", jti: "jti.1", capabilityRef: "observe_public" }
  return { transport: createBridgeTransport({ baseUrl, credential }), credential, baseUrl }
}

afterEach(() => {
  globalThis.fetch = realFetch
})

describe("fetchSessionEventBatchViaBridge", () => {
  test("posts a session.subscribe envelope over the bridge auth and parses the batch", async () => {
    const calls: { url: string; headers: Record<string, string>; body: any }[] = []
    globalThis.fetch = (async (url: string | URL | Request, init?: RequestInit) => {
      calls.push({
        url: String(url),
        headers: (init?.headers ?? {}) as Record<string, string>,
        body: init?.body ? JSON.parse(String(init.body)) : undefined,
      })
      return new Response(JSON.stringify({ ok: true, result: sessionEventStreamFixture }), {
        status: 200,
        headers: { "content-type": "application/json" },
      })
    }) as typeof fetch

    const events = await fetchSessionEventBatchViaBridge(bridge(), "session.pylon.codex_composer.fixture0001", 2)

    // Decoded into the typed protocol SessionEvent batch.
    expect(events).toEqual(sessionEventStreamFixture)
    // Hit /bridge on the credential's base with the Bridge auth header.
    expect(calls[0]?.url).toBe("http://100.1.2.3:4716/bridge")
    expect(calls[0]?.headers.authorization).toBe("Bridge pair.1:jti.1")
    // Carried the session.subscribe verb + the resume cursor.
    expect(calls[0]?.body.verb).toBe("session.subscribe")
    expect(calls[0]?.body.sessionRef).toBe("session.pylon.codex_composer.fixture0001")
    expect(calls[0]?.body.cursor).toBe(2)
    expect(calls[0]?.body.capabilityRef).toBe("observe_public")
  })

  test("omits the cursor for a fresh subscribe (cursor 0)", async () => {
    let body: any
    globalThis.fetch = (async (_url: string | URL | Request, init?: RequestInit) => {
      body = init?.body ? JSON.parse(String(init.body)) : undefined
      return new Response(JSON.stringify({ ok: true, result: [] }), {
        status: 200,
        headers: { "content-type": "application/json" },
      })
    }) as typeof fetch

    const events = await fetchSessionEventBatchViaBridge(bridge(), "session.fixture.0001", 0)

    expect(events).toEqual([])
    expect(body.verb).toBe("session.subscribe")
    expect(body).not.toHaveProperty("cursor")
  })

  test("throws on a non-ok bridge response so the caller can fall back to polling", async () => {
    globalThis.fetch = (async () =>
      new Response(JSON.stringify({ ok: false, error: "capability_denied" }), {
        status: 403,
        headers: { "content-type": "application/json" },
      })) as unknown as typeof fetch

    await expect(
      fetchSessionEventBatchViaBridge(bridge(), "session.fixture.0001", 0),
    ).rejects.toThrow("capability_denied")
  })
})
