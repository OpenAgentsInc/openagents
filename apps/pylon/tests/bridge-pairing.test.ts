import { describe, expect, test } from "bun:test"
import type { Capability, PairingCredentialClaims } from "@openagentsinc/autopilot-control-protocol"
import {
  exchangeBootstrap,
  hashSecret,
  isCredentialValid,
  isPairingActive,
  mintBootstrapSecret,
  type ExchangeBootstrapInput,
  type PairingRecord,
} from "../src/node/bridge-pairing"

const now = new Date("2026-06-13T12:00:00.000Z")
const capabilities: Capability[] = ["observe_private", "answer_decision", "read_artifact"]

function successfulInput(overrides: Partial<ExchangeBootstrapInput> = {}): ExchangeBootstrapInput {
  return {
    bootstrapId: "bootstrap-1",
    secret: "secret-1",
    now,
    ttlSeconds: 60,
    clientId: "client-1",
    deviceClass: "desktop",
    capabilities,
    projectionLevel: "private",
    issuer: "pylon-node",
    audience: "autopilot-client",
    jti: "jti-1",
    stored: {
      bootstrapId: "bootstrap-1",
      secretHash: hashSecret("secret-1"),
      used: false,
    },
    ...overrides,
  }
}

function claims(overrides: Partial<PairingCredentialClaims> = {}): PairingCredentialClaims {
  const result = exchangeBootstrap(successfulInput())
  if (!result.ok) {
    throw new Error(`expected bootstrap exchange success, got ${result.reason}`)
  }
  return { ...result.claims, ...overrides }
}

describe("bridge pairing", () => {
  test("bootstrap minting is deterministic through injected randomness", () => {
    const values = ["bootstrap-1", "secret-1"]
    const minted = mintBootstrapSecret(() => {
      const value = values.shift()
      if (!value) {
        throw new Error("unexpected rand call")
      }
      return value
    })

    expect(minted).toEqual({ bootstrapId: "bootstrap-1", secret: "secret-1" })
  })

  test("bootstrap exchange success returns claims with capabilities and future expiry", () => {
    const result = exchangeBootstrap(successfulInput())

    expect(result.ok).toBe(true)
    if (!result.ok) return

    expect(result.claims).toEqual({
      pairingRef: "bootstrap-1",
      clientId: "client-1",
      deviceClass: "desktop",
      issuer: "pylon-node",
      audience: "autopilot-client",
      expiresAt: "2026-06-13T12:01:00.000Z",
      jti: "jti-1",
      projectionLevel: "private",
      capabilities,
    })
    expect(new Date(result.claims.expiresAt).getTime()).toBeGreaterThan(now.getTime())
  })

  test("wrong secret returns bad_secret", () => {
    expect(exchangeBootstrap(successfulInput({ secret: "wrong-secret" }))).toEqual({
      ok: false,
      reason: "bad_secret",
    })
  })

  test("reused bootstrap returns already_used", () => {
    expect(
      exchangeBootstrap(
        successfulInput({
          stored: {
            bootstrapId: "bootstrap-1",
            secretHash: hashSecret("secret-1"),
            used: true,
          },
        }),
      ),
    ).toEqual({
      ok: false,
      reason: "already_used",
    })
  })

  test("unknown bootstrap id returns unknown_bootstrap", () => {
    expect(exchangeBootstrap(successfulInput({ bootstrapId: "missing-bootstrap" }))).toEqual({
      ok: false,
      reason: "unknown_bootstrap",
    })
  })

  test("expired credential is invalid", () => {
    expect(
      isCredentialValid(
        claims({ expiresAt: "2026-06-13T11:59:59.000Z" }),
        now,
      ),
    ).toBe(false)
  })

  test("revoked record is inactive", () => {
    const validClaims = claims()
    const record: PairingRecord = {
      pairingRef: validClaims.pairingRef,
      jti: validClaims.jti,
      revoked: true,
    }

    expect(isPairingActive(record, validClaims, now)).toBe(false)
  })
})
