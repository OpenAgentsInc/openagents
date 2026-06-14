// Dev-token-free bridge pairing (connectBridgeWithBootstrap): pair onto a node's
// /bridge using a single-use bootstrap decoded from a QR/pasted pairing code,
// with no dev token minting the bootstrap. These tests mock global fetch so they
// run without a node or a device.

import { afterEach, describe, expect, test } from "bun:test"

import { encodeBootstrapPayload } from "@openagentsinc/autopilot-control-protocol"

import { connectBridgeWithBootstrap } from "./control-client"

const realFetch = globalThis.fetch

function bootstrapCode(addresses: { loopback?: string; lan?: string; tailnet?: string }): string {
  return encodeBootstrapPayload({
    version: 1,
    addresses,
    bootstrapId: "boot.test",
    secret: "s3cr3t-single-use",
    projectionLevel: "public_safe",
    capabilities: ["observe_public"],
  })
}

afterEach(() => {
  globalThis.fetch = realFetch
})

describe("connectBridgeWithBootstrap", () => {
  test("pairs over the decoded bootstrap and returns the resolved tailnet baseUrl", async () => {
    const calls: { url: string; body: unknown }[] = []
    globalThis.fetch = (async (url: string | URL | Request, init?: RequestInit) => {
      calls.push({ url: String(url), body: init?.body ? JSON.parse(String(init.body)) : undefined })
      return new Response(
        JSON.stringify({ ok: true, claims: { pairingRef: "pair.1", jti: "jti.1" } }),
        { status: 200, headers: { "content-type": "application/json" } },
      )
    }) as typeof fetch

    const code = bootstrapCode({ loopback: "http://127.0.0.1:4716", tailnet: "http://100.1.2.3:4716" })
    const result = await connectBridgeWithBootstrap(code)

    expect(result).not.toBeNull()
    // tailnet is preferred over loopback by resolveBaseUrls' default order.
    expect(result?.baseUrl).toBe("http://100.1.2.3:4716")
    expect(result?.session.credential.pairingRef).toBe("pair.1")
    // The exchange hit /bridge/pair on the resolved base, carrying the
    // single-use bootstrap secret (not a dev token).
    expect(calls[0]?.url).toBe("http://100.1.2.3:4716/bridge/pair")
    expect((calls[0]?.body as { secret?: string })?.secret).toBe("s3cr3t-single-use")
  })

  test("returns null for an undecodable code without calling fetch", async () => {
    let called = false
    globalThis.fetch = (async () => {
      called = true
      return new Response("{}", { status: 200 })
    }) as unknown as typeof fetch

    const result = await connectBridgeWithBootstrap("not-a-valid-bootstrap")
    expect(result).toBeNull()
    expect(called).toBe(false)
  })

  test("returns null when the node rejects the exchange", async () => {
    globalThis.fetch = (async () =>
      new Response(JSON.stringify({ ok: false, reason: "expired" }), {
        status: 401,
        headers: { "content-type": "application/json" },
      })) as unknown as typeof fetch

    const code = bootstrapCode({ tailnet: "http://100.1.2.3:4716" })
    const result = await connectBridgeWithBootstrap(code)
    expect(result).toBeNull()
  })
})
