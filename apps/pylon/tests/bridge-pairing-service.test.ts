import { describe, expect, test } from "bun:test"
import { createBridgePairingService, type BridgeExchangeInput } from "../src/node/bridge-pairing-service"

const baseExchange = (
  bootstrapId: string,
  secret: string,
  overrides: Partial<BridgeExchangeInput> = {},
): BridgeExchangeInput => ({
  bootstrapId,
  secret,
  now: new Date("2026-06-13T12:00:00.000Z"),
  ttlSeconds: 3600,
  clientId: "client.phone-1",
  deviceClass: "mobile",
  capabilities: ["observe_public"],
  projectionLevel: "private",
  issuer: "node.test",
  audience: "client.phone-1",
  jti: "jti.1",
  ...overrides,
})

describe("bridge pairing service (CL-14)", () => {
  let seq = 0
  const svc = createBridgePairingService({ rand: () => `r${seq++}` })

  test("issue -> exchange -> validate, single-use, revoke", () => {
    const { bootstrapId, secret } = svc.issueBootstrap()

    const ok = svc.exchange(baseExchange(bootstrapId, secret))
    expect(ok.ok).toBe(true)
    if (!ok.ok) return
    expect(ok.claims.capabilities).toEqual(["observe_public"])
    expect(ok.claims.projectionLevel).toBe("private")

    // valid now, invalid after expiry
    expect(svc.validate(ok.claims, new Date("2026-06-13T12:30:00.000Z"))).toBe(true)
    expect(svc.validate(ok.claims, new Date("2026-06-13T14:00:00.000Z"))).toBe(false)

    // single-use: the bootstrap can't be exchanged again
    const reuse = svc.exchange(baseExchange(bootstrapId, secret))
    expect(reuse).toEqual({ ok: false, reason: "already_used" })

    // revoke kills validation even before expiry
    svc.revoke(ok.claims.pairingRef)
    expect(svc.validate(ok.claims, new Date("2026-06-13T12:30:00.000Z"))).toBe(false)
  })

  test("rejects unknown bootstrap and bad secret", () => {
    expect(svc.exchange(baseExchange("nope", "x"))).toEqual({ ok: false, reason: "unknown_bootstrap" })
    const { bootstrapId } = svc.issueBootstrap()
    expect(svc.exchange(baseExchange(bootstrapId, "wrong-secret"))).toEqual({ ok: false, reason: "bad_secret" })
  })

  test("unpaired credential never validates", () => {
    const fake = {
      pairingRef: "never-paired",
      clientId: "x",
      deviceClass: "mobile",
      issuer: "node.test",
      audience: "x",
      expiresAt: "2030-01-01T00:00:00.000Z",
      jti: "jti.x",
      projectionLevel: "private" as const,
      capabilities: ["observe_public" as const],
    }
    expect(svc.validate(fake, new Date("2026-06-13T12:30:00.000Z"))).toBe(false)
  })

  test("listClients reports paired roster (refs-only) and revoked flag; revoke returns found", () => {
    const svc3 = createBridgePairingService({ rand: () => `c${seq++}` })
    const a = svc3.issueBootstrap()
    const okA = svc3.exchange(
      baseExchange(a.bootstrapId, a.secret, {
        clientId: "phone.a",
        capabilities: ["observe_public", "answer_decision"],
        jti: "jti.a",
      }),
    )
    const b = svc3.issueBootstrap()
    const okB = svc3.exchange(baseExchange(b.bootstrapId, b.secret, { clientId: "phone.b", jti: "jti.b" }))
    if (!okA.ok || !okB.ok) throw new Error("exchange failed")

    const clients = svc3.listClients()
    expect(clients.length).toBe(2)
    const refA = clients.find((c) => c.clientId === "phone.a")
    expect(refA?.capabilities).toEqual(["observe_public", "answer_decision"])
    expect(refA?.projectionLevel).toBe("private")
    expect(refA?.revoked).toBe(false)
    // refs-only: no secret material leaks into the roster
    expect(JSON.stringify(clients)).not.toContain(a.secret)

    // revoke returns true for a known pairing, false for an unknown one, and
    // flips the roster's revoked flag.
    expect(svc3.revoke(okA.claims.pairingRef)).toBe(true)
    expect(svc3.revoke("no-such-pairing")).toBe(false)
    expect(svc3.listClients().find((c) => c.clientId === "phone.a")?.revoked).toBe(true)
  })

  test("authorize returns stored claims; rejects mismatched jti / unknown / revoked", () => {
    const svc2 = createBridgePairingService({ rand: () => `k${seq++}` })
    const { bootstrapId, secret } = svc2.issueBootstrap()
    const ok = svc2.exchange(baseExchange(bootstrapId, secret, { capabilities: ["observe_public"], jti: "jti.auth" }))
    if (!ok.ok) throw new Error("exchange failed")
    const ref = ok.claims.pairingRef
    const now = new Date("2026-06-13T12:30:00.000Z")

    expect(svc2.authorize(ref, "jti.auth", now)?.capabilities).toEqual(["observe_public"])
    expect(svc2.authorize(ref, "wrong-jti", now)).toBeNull()
    expect(svc2.authorize("unknown", "jti.auth", now)).toBeNull()
    svc2.revoke(ref)
    expect(svc2.authorize(ref, "jti.auth", now)).toBeNull()
  })
})
