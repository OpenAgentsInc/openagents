import { describe, expect, test } from "bun:test"
import { sessionListFixture } from "./fixtures.js"
import { createBridgeTransport, pairBridge } from "./bridge-transport.js"

describe("client bridge transport (CL-14)", () => {
  test("pairBridge posts to /bridge/pair and returns claims", async () => {
    let captured: { url: string; body: any } | null = null
    const fetchImpl = (async (url: string, init?: RequestInit) => {
      captured = { url, body: JSON.parse(init!.body as string) }
      return new Response(JSON.stringify({ ok: true, claims: { pairingRef: "p1", jti: "j1", capabilities: ["observe_public"] } }), { status: 200 })
    }) as unknown as typeof fetch
    const r = await pairBridge({
      baseUrl: "https://node.example/", bootstrapId: "b1", secret: "s1",
      clientId: "phone", deviceClass: "mobile", capabilities: ["observe_public"], projectionLevel: "private",
      fetchImpl,
    })
    expect(r.ok).toBe(true)
    expect(captured!.url).toBe("https://node.example/bridge/pair")
    expect(captured!.body.bootstrapId).toBe("b1")
  })

  test("transport.list sends Bridge auth + parses sessions; throws on error", async () => {
    let auth: string | null = null
    const okFetch = (async (_url: string, init?: RequestInit) => {
      auth = (init!.headers as Record<string, string>).authorization
      return new Response(JSON.stringify({ ok: true, result: sessionListFixture }), { status: 200 })
    }) as unknown as typeof fetch
    const t = createBridgeTransport({ baseUrl: "https://node.example", credential: { pairingRef: "p1", jti: "j1" }, fetchImpl: okFetch })
    const sessions = await t.list()
    expect(auth as string | null).toBe("Bridge p1:j1")
    expect(sessions.length).toBe(sessionListFixture.length)

    const errFetch = (async () => new Response(JSON.stringify({ ok: false, error: "invalid or expired pairing" }), { status: 401 })) as unknown as typeof fetch
    const t2 = createBridgeTransport({ baseUrl: "https://node.example", credential: { pairingRef: "p1", jti: "bad" }, fetchImpl: errFetch })
    await expect(t2.list()).rejects.toThrow(/invalid or expired/)
  })

  test("transport.history sends session.history verb + sessionRef and returns the events projection", async () => {
    let body: any = null
    let auth: string | null = null
    const fetchImpl = (async (_url: string, init?: RequestInit) => {
      auth = (init!.headers as Record<string, string>).authorization
      body = JSON.parse(init!.body as string)
      return new Response(
        JSON.stringify({ ok: true, result: { recentEvents: [{ eventIndex: 0, state: "running" }] } }),
        { status: 200 },
      )
    }) as unknown as typeof fetch
    const t = createBridgeTransport({
      baseUrl: "https://node.example",
      credential: { pairingRef: "p1", jti: "j1" },
      fetchImpl,
    })
    const result = (await t.history("sess.42")) as { recentEvents: unknown[] }
    expect(auth as string | null).toBe("Bridge p1:j1")
    expect(body.verb).toBe("session.history")
    expect(body.sessionRef).toBe("sess.42")
    expect(result.recentEvents.length).toBe(1)
  })

  test("transport.subscribe sends session.subscribe + cursor and returns the parsed event batch", async () => {
    let body: any = null
    let auth: string | null = null
    const fetchImpl = (async (_url: string, init?: RequestInit) => {
      auth = (init!.headers as Record<string, string>).authorization
      body = JSON.parse(init!.body as string)
      return new Response(
        JSON.stringify({
          ok: true,
          result: {
            sessionRef: "sess.42",
            state: "running",
            recentEvents: [
              { eventIndex: 2, phase: "composer_event", state: "running", observedAt: "t2", messageText: "edit" },
              { eventIndex: 3, phase: "completed", state: "completed", observedAt: "t3", artifactRef: "art.1" },
            ],
          },
        }),
        { status: 200 },
      )
    }) as unknown as typeof fetch
    const t = createBridgeTransport({
      baseUrl: "https://node.example",
      credential: { pairingRef: "p1", jti: "j1", capabilityRef: "observe_private" },
      fetchImpl,
    })
    const batch = await t.subscribe({ sessionRef: "sess.42", cursor: 1 })
    expect(auth as string | null).toBe("Bridge p1:j1")
    expect(body.verb).toBe("session.subscribe")
    expect(body.sessionRef).toBe("sess.42")
    expect(body.cursor).toBe(1)
    expect(body.capabilityRef).toBe("observe_private")
    expect(batch.events.map((e) => e.eventIndex)).toEqual([2, 3])
    expect(batch.events[1]?.artifactRef).toBe("art.1")
    expect(batch.cursor).toBe(3)

    const errFetch = (async () =>
      new Response(JSON.stringify({ ok: false, error: "capability not granted" }), { status: 403 })) as unknown as typeof fetch
    const t2 = createBridgeTransport({
      baseUrl: "https://node.example",
      credential: { pairingRef: "p1", jti: "j1" },
      fetchImpl: errFetch,
    })
    await expect(t2.subscribe({ sessionRef: "sess.42" })).rejects.toThrow(/capability not granted/)
  })

  test("transport.readArtifact sends artifact.read + read_artifact cap and parses the envelope", async () => {
    let body: any = null
    let auth: string | null = null
    const fetchImpl = (async (_url: string, init?: RequestInit) => {
      auth = (init!.headers as Record<string, string>).authorization
      body = JSON.parse(init!.body as string)
      return new Response(
        JSON.stringify({
          ok: true,
          result: { sessionRef: "sess.42", kind: "proof", artifact: { schema: "x", executor: { outcome: "completed" } } },
        }),
        { status: 200 },
      )
    }) as unknown as typeof fetch
    const t = createBridgeTransport({
      baseUrl: "https://node.example",
      credential: { pairingRef: "p1", jti: "j1" },
      fetchImpl,
    })
    const result = await t.readArtifact("sess.42")
    expect(auth as string | null).toBe("Bridge p1:j1")
    expect(body.verb).toBe("artifact.read")
    expect(body.capabilityRef).toBe("read_artifact")
    expect(body.sessionRef).toBe("sess.42")
    expect(result.kind).toBe("proof")
    expect(result.sessionRef).toBe("sess.42")

    const errFetch = (async () =>
      new Response(JSON.stringify({ ok: false, error: "capability not granted" }), {
        status: 403,
      })) as unknown as typeof fetch
    const t2 = createBridgeTransport({
      baseUrl: "https://node.example",
      credential: { pairingRef: "p1", jti: "j1" },
      fetchImpl: errFetch,
    })
    await expect(t2.readArtifact("sess.42")).rejects.toThrow(/capability not granted/)
  })
})
