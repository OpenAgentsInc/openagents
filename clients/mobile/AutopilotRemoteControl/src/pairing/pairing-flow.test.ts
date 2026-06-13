import { describe, expect, test } from "bun:test"

import type { PairingCredentialClaims } from "@openagentsinc/autopilot-control-protocol"
import {
  createCredentialStore,
  encodeBootstrapPayload,
} from "@openagentsinc/autopilot-control-protocol"

import { createPairingFlow } from "./pairing-flow"

const claims: PairingCredentialClaims = {
  pairingRef: "pairing.mobile.test",
  clientId: "mobile-client-1",
  deviceClass: "mobile",
  issuer: "openagents.test",
  audience: "pylon.test",
  expiresAt: "2026-06-13T12:00:00.000Z",
  jti: "credential.mobile.test",
  projectionLevel: "team",
  capabilities: ["observe_public", "read_artifact", "answer_decision"],
}

describe("pairing flow", () => {
  test("decodes bootstrap payload, resolves tailnet first, and returns exchange descriptor", () => {
    const qrOrCode = encodeBootstrapPayload({
      version: 1,
      addresses: {
        loopback: "http://127.0.0.1:8787",
        lan: "http://192.168.1.50:8787",
        tailnet: "https://pylon.tailnet.test",
      },
      bootstrapId: "bootstrap.mobile.test",
      secret: "pairing-secret.mobile.test",
      projectionLevel: "team",
      capabilities: ["observe_public", "read_artifact"],
    })
    const flow = createPairingFlow({ clientId: "mobile-client-1" })

    expect(flow.startPairing(qrOrCode)).toEqual({
      status: { phase: "pairing" },
      statusView: { label: "Pairing", tone: "info" },
      baseUrls: [
        "https://pylon.tailnet.test",
        "http://192.168.1.50:8787",
        "http://127.0.0.1:8787",
      ],
      exchangeRequest: {
        url: "https://pylon.tailnet.test/bridge/pair/exchange",
        method: "POST",
        headers: {
          Authorization: "Bearer pairing-secret.mobile.test",
          "content-type": "application/json",
        },
        body: {
          verb: "bridge.pair.exchange",
          bootstrapId: "bootstrap.mobile.test",
          clientId: "mobile-client-1",
        },
      },
    })
  })

  test("stores completed credentials and reports paired usable state", () => {
    const store = createCredentialStore()
    const flow = createPairingFlow({ clientId: "mobile-client-1", credentialStore: store })

    expect(flow.completePairing(claims)).toEqual({
      status: { phase: "paired", pairingRef: "pairing.mobile.test" },
      statusView: { label: "Paired: pairing.mobile.test", tone: "success" },
      credential: claims,
    })
    expect(store.get()).toEqual(claims)
    expect(flow.getCredential()).toEqual(claims)
    expect(flow.isCredentialUsable(Date.parse("2026-06-13T11:59:59.999Z"))).toBe(true)
  })

  test("returns error state for bad input", () => {
    const flow = createPairingFlow({ clientId: "mobile-client-1" })

    expect(flow.startPairing("not a bootstrap payload")).toEqual({
      status: { phase: "error", error: "Malformed bootstrap payload" },
      statusView: {
        label: "Pairing failed: Malformed bootstrap payload",
        tone: "danger",
      },
    })
  })
})
