import { describe, expect, test } from "bun:test"

import { createBridgePairingService } from "../src/node/bridge-pairing-service"
import {
  pairBridge,
  createBridgeTransport,
  verbAllowedByCapabilities,
  type BridgeCredential,
  type Capability,
} from "@openagentsinc/autopilot-control-protocol"

// CL-14 end-to-end proof: the shared client bridge transport (pairBridge +
// createBridgeTransport, used by mobile AND desktop) works against the REAL
// node pairing service + capability gating. Only HTTP routing + the session
// data are stubbed; the bootstrap mint, exchange, authorize, and capability
// enforcement are the exact node code paths.
describe("bridge client transport (CL-14)", () => {
  // A fetchImpl that routes /bridge/pair and /bridge through the real service,
  // mirroring apps/pylon/src/node/control-server.ts.
  const makeFetch = (service: ReturnType<typeof createBridgePairingService>, sessions: unknown[]) =>
    (async (input: any, init?: any) => {
      const url = typeof input === "string" ? input : input.url
      const path = new URL(url).pathname
      const body = init?.body ? JSON.parse(init.body) : {}
      if (path === "/bridge/pair") {
        const result = service.exchange({
          bootstrapId: body.bootstrapId,
          secret: body.secret,
          now: new Date(),
          ttlSeconds: body.ttlSeconds ?? 86_400,
          clientId: body.clientId ?? "client",
          deviceClass: body.deviceClass ?? "unknown",
          capabilities: (body.capabilities ?? ["observe_public"]) as Capability[],
          projectionLevel: body.projectionLevel ?? "public_safe",
          issuer: "pylon.node",
          audience: body.clientId ?? "client",
          jti: "test-jti-deterministic",
        })
        return Response.json(result.ok ? { ok: true, claims: result.claims } : { ok: false, reason: result.reason }, {
          status: result.ok ? 200 : 401,
        })
      }
      if (path === "/bridge") {
        const header = (init?.headers?.authorization as string) ?? ""
        const m = /^Bridge\s+([^:]+):(.+)$/.exec(header)
        if (!m) return Response.json({ error: "bridge credential required" }, { status: 401 })
        const claims = service.authorize(m[1]!, m[2]!, new Date())
        if (claims === null) return Response.json({ error: "invalid or expired pairing" }, { status: 401 })
        const verb = body.verb ?? ""
        if (!verbAllowedByCapabilities(verb, claims.capabilities)) {
          return Response.json({ error: "capability not granted", verb }, { status: 403 })
        }
        if (verb === "session.list") return Response.json({ ok: true, result: sessions })
        return Response.json({ error: "unsupported bridge verb", verb }, { status: 501 })
      }
      return Response.json({ error: "not found" }, { status: 404 })
    }) as unknown as typeof fetch

  const sessionRow = {
    sessionRef: "session.1",
    adapter: "codex",
    state: "running",
    accountRefHash: null,
    latestActivity: "Bash: bun test",
    updatedAt: "2026-06-13T12:00:00.000Z",
  }

  test("pairs a bootstrap then lists sessions over the bridge transport", async () => {
    const service = createBridgePairingService()
    const fetchImpl = makeFetch(service, [sessionRow])
    const boot = service.issueBootstrap()

    const pair = await pairBridge({
      baseUrl: "http://node.test",
      bootstrapId: boot.bootstrapId,
      secret: boot.secret,
      clientId: "mobile",
      deviceClass: "ios",
      capabilities: ["observe_public"],
      projectionLevel: "public_safe",
      fetchImpl,
    })
    expect(pair.ok).toBe(true)
    if (!pair.ok) return

    const credential: BridgeCredential = {
      pairingRef: pair.claims.pairingRef,
      jti: pair.claims.jti,
      capabilityRef: "observe_public",
    }
    const transport = createBridgeTransport({ baseUrl: "http://node.test", credential, fetchImpl })
    const list = await transport.list()
    expect(list).toHaveLength(1)
    expect(list[0].sessionRef).toBe("session.1")
    expect(list[0].latestActivity).toBe("Bash: bun test")
  })

  test("rejects an invalid bootstrap secret", async () => {
    const service = createBridgePairingService()
    const fetchImpl = makeFetch(service, [])
    const boot = service.issueBootstrap()
    const pair = await pairBridge({
      baseUrl: "http://node.test",
      bootstrapId: boot.bootstrapId,
      secret: "wrong-secret",
      clientId: "mobile",
      deviceClass: "ios",
      capabilities: ["observe_public"],
      projectionLevel: "public_safe",
      fetchImpl,
    })
    expect(pair.ok).toBe(false)
  })

  test("rejects bridge reads with a bogus credential", async () => {
    const service = createBridgePairingService()
    const fetchImpl = makeFetch(service, [sessionRow])
    const transport = createBridgeTransport({
      baseUrl: "http://node.test",
      credential: { pairingRef: "nope", jti: "nope" },
      fetchImpl,
    })
    await expect(transport.list()).rejects.toThrow()
  })
})
