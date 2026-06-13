import { describe, expect, test } from "bun:test"
import { sessionListFixture } from "./fixtures"
import { createBridgeTransport, pairBridge } from "./bridge-transport"

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
    expect(auth).toBe("Bridge p1:j1")
    expect(sessions.length).toBe(sessionListFixture.length)

    const errFetch = (async () => new Response(JSON.stringify({ ok: false, error: "invalid or expired pairing" }), { status: 401 })) as unknown as typeof fetch
    const t2 = createBridgeTransport({ baseUrl: "https://node.example", credential: { pairingRef: "p1", jti: "bad" }, fetchImpl: errFetch })
    await expect(t2.list()).rejects.toThrow(/invalid or expired/)
  })
})
