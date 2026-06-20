import { describe, expect, test } from "bun:test"

import type { PairingCredentialClaims } from "./bridge.js"
import {
  createCredentialStore,
  hasCapability,
  isCredentialUsable,
  projectionLevelOf,
} from "./pairing-client.js"

const claims: PairingCredentialClaims = {
  pairingRef: "pairing.test",
  clientId: "client.test",
  deviceClass: "browser",
  issuer: "openagents.test",
  audience: "pylon.test",
  expiresAt: "2026-06-13T12:00:00.000Z",
  jti: "credential.test",
  projectionLevel: "team",
  capabilities: ["observe_public", "read_artifact", "answer_decision"],
}

describe("pairing client credential helpers", () => {
  test("credential is usable before expiry and not after", () => {
    expect(isCredentialUsable(claims, Date.parse("2026-06-13T11:59:59.999Z"))).toBe(true)
    expect(isCredentialUsable(claims, Date.parse(claims.expiresAt))).toBe(false)
    expect(isCredentialUsable(claims, Date.parse("2026-06-13T12:00:00.001Z"))).toBe(false)
  })

  test("capability checks reflect granted capabilities", () => {
    expect(hasCapability(claims, "observe_public")).toBe(true)
    expect(hasCapability(claims, "read_artifact")).toBe(true)
    expect(hasCapability(claims, "cancel")).toBe(false)
  })

  test("projection accessor returns the credential projection level", () => {
    expect(projectionLevelOf(claims)).toBe("team")
  })

  test("store round-trips and clears credential claims", () => {
    const store = createCredentialStore()

    expect(store.get()).toBeUndefined()

    store.set(claims)
    expect(store.get()).toEqual(claims)

    store.clear()
    expect(store.get()).toBeUndefined()
  })
})
