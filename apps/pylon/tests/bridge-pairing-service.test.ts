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
})
