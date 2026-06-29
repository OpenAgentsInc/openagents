import { describe, expect, test } from "bun:test"

import { createBridgePairingService } from "../src/node/bridge-pairing-service"
import {
  pairBridge,
  createBridgeTransport,
  verbAllowedByCapabilities,
  type BridgeCredential,
  type Capability,
} from "@openagentsinc/autopilot-control-protocol"

// #5494 (epic #5492 G1) end-to-end proof: the four steer-actions that mobile
// previously reached only over the dev-token /command path — spawn,
// submit-intent, turn.steer, pause/resume, deploy — now ride the shared
// capability-scoped bridge transport (pairBridge + createBridgeTransport) against the REAL node
// pairing service + capability gating. Only HTTP routing + the action results
// are stubbed; the bootstrap exchange, authorize, and capability enforcement are
// the exact node code paths from apps/pylon/src/node/control-server.ts.
describe("bridge steer transport (#5494)", () => {
  const STEER_CAPS: Capability[] = [
    "observe_public",
    "spawn_session",
    "send_instruction",
    "pause_resume",
    "deploy_cloud",
  ]

  // Mirrors the new /bridge steer branches in control-server.ts: the verb is
  // gated against the STORED claims, then routed to a node action stub. Records
  // each routed call so we can assert the broker passed the payload through.
  const makeFetch = (
    service: ReturnType<typeof createBridgePairingService>,
    calls: Array<Record<string, unknown>>,
  ) =>
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
          jti: "test-jti-steer",
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
        if (verb === "session.spawn") {
          calls.push({ verb, adapter: body.adapter, objective: body.objective, lane: body.lane })
          return Response.json({ ok: true, result: { sessionRef: "session.spawned.1", state: "running" } })
        }
        if (verb === "intent.submit") {
          calls.push({ verb, title: body.title, body: body.body, submittedByClientRef: body.submittedByClientRef })
          return Response.json({ ok: true, result: { status: "received" } })
        }
        if (verb === "turn.steer") {
          calls.push({ verb, sessionRef: body.sessionRef, instruction: body.instruction, timeoutSeconds: body.timeoutSeconds })
          return Response.json({ ok: true, result: { sessionRef: "session.child.1", parentSessionRef: body.sessionRef, state: "queued" } })
        }
        if (verb === "coordinator.pause") {
          calls.push({ verb })
          return Response.json({ ok: true, result: { paused: true } })
        }
        if (verb === "coordinator.resume") {
          calls.push({ verb })
          return Response.json({ ok: true, result: { paused: false } })
        }
        if (verb === "deploy.cloud") {
          calls.push({ verb, target: body.target, ref: body.ref, env: body.env })
          return Response.json({ ok: true, result: { accepted: true, reason: "deploy command ready", errors: [] } })
        }
        return Response.json({ error: "unsupported bridge verb", verb }, { status: 501 })
      }
      return Response.json({ error: "not found" }, { status: 404 })
    }) as unknown as typeof fetch

  async function pairWith(
    service: ReturnType<typeof createBridgePairingService>,
    fetchImpl: typeof fetch,
    capabilities: Capability[],
  ): Promise<BridgeCredential> {
    const boot = service.issueBootstrap()
    const pair = await pairBridge({
      baseUrl: "http://node.test",
      bootstrapId: boot.bootstrapId,
      secret: boot.secret,
      clientId: "mobile",
      deviceClass: "ios",
      capabilities,
      projectionLevel: "public_safe",
      fetchImpl,
    })
    if (!pair.ok) throw new Error(`pair failed: ${pair.reason}`)
    return { pairingRef: pair.claims.pairingRef, jti: pair.claims.jti }
  }

  test("a steer-scoped credential reaches all promoted steer verbs", async () => {
    const service = createBridgePairingService()
    const calls: Array<Record<string, unknown>> = []
    const fetchImpl = makeFetch(service, calls)
    const credential = await pairWith(service, fetchImpl, STEER_CAPS)
    const transport = createBridgeTransport({ baseUrl: "http://node.test", credential, fetchImpl })

    const spawned = (await transport.spawn({ adapter: "codex", objective: "ship it", lane: "auto" })) as {
      sessionRef: string
    }
    expect(spawned.sessionRef).toBe("session.spawned.1")

    const intent = (await transport.submitIntent({
      title: "Ask",
      body: "do it",
      submittedByClientRef: "mobile",
    })) as { status: string }
    expect(intent.status).toBe("received")

    const turn = (await transport.steerTurn({
      sessionRef: "session.spawned.1",
      instruction: "continue with tests",
      timeoutSeconds: 120,
    })) as { sessionRef: string; parentSessionRef: string }
    expect(turn.sessionRef).toBe("session.child.1")
    expect(turn.parentSessionRef).toBe("session.spawned.1")

    const paused = (await transport.pauseCoordinator()) as { paused: boolean }
    expect(paused.paused).toBe(true)
    const resumed = (await transport.resumeCoordinator()) as { paused: boolean }
    expect(resumed.paused).toBe(false)

    const deployed = (await transport.deployCloud({ target: "cloudrun", ref: "main", env: "production" })) as {
      accepted: boolean
    }
    expect(deployed.accepted).toBe(true)

    expect(calls.map((c) => c.verb)).toEqual([
      "session.spawn",
      "intent.submit",
      "turn.steer",
      "coordinator.pause",
      "coordinator.resume",
      "deploy.cloud",
    ])
    // The broker received the payloads (capability scoping carries the data, not
    // the dev token).
    expect(calls[0]).toMatchObject({ adapter: "codex", objective: "ship it" })
    expect(calls[1]).toMatchObject({ title: "Ask", body: "do it", submittedByClientRef: "mobile" })
    expect(calls[2]).toMatchObject({ sessionRef: "session.spawned.1", instruction: "continue with tests", timeoutSeconds: 120 })
    expect(calls[5]).toMatchObject({ target: "cloudrun", ref: "main", env: "production" })
  })

  test("a read-only credential is denied every promoted steer verb", async () => {
    const service = createBridgePairingService()
    const calls: Array<Record<string, unknown>> = []
    const fetchImpl = makeFetch(service, calls)
    const credential = await pairWith(service, fetchImpl, ["observe_public", "read_artifact"])
    const transport = createBridgeTransport({ baseUrl: "http://node.test", credential, fetchImpl })

    await expect(transport.spawn({ adapter: "codex", objective: "x" })).rejects.toThrow()
    await expect(transport.submitIntent({ title: "t", body: "b" })).rejects.toThrow()
    await expect(transport.steerTurn({ sessionRef: "s1", instruction: "next" })).rejects.toThrow()
    await expect(transport.pauseCoordinator()).rejects.toThrow()
    await expect(transport.resumeCoordinator()).rejects.toThrow()
    await expect(transport.deployCloud({ target: "cloudrun", ref: "main" })).rejects.toThrow()
    // Nothing was routed to a node action.
    expect(calls).toHaveLength(0)
  })

  test("a cancel-only credential cannot spawn or deploy (capability isolation)", async () => {
    const service = createBridgePairingService()
    const calls: Array<Record<string, unknown>> = []
    const fetchImpl = makeFetch(service, calls)
    const credential = await pairWith(service, fetchImpl, ["observe_public", "cancel"])
    const transport = createBridgeTransport({ baseUrl: "http://node.test", credential, fetchImpl })

    await expect(transport.spawn({ adapter: "codex", objective: "x" })).rejects.toThrow()
    await expect(transport.deployCloud({ target: "cloudrun", ref: "main" })).rejects.toThrow()
    expect(calls).toHaveLength(0)
  })
})
