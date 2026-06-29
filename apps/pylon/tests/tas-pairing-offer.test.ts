import { describe, expect, spyOn, test } from "bun:test"
import {
  type Capability,
  decodeBootstrapPayload,
} from "@openagentsinc/autopilot-control-protocol"

import { buildPairingOffer } from "../src/node/pairing-offer"

const input = {
  binds: [
    { address: "127.0.0.1", requiresAuth: false },
    { address: "192.168.1.10", requiresAuth: true },
    { address: "100.64.0.10", requiresAuth: true },
  ],
  bootstrapId: "bootstrap-1",
  secret: "secret-1",
  projectionLevel: "private" as const,
  capabilities: ["observe_private", "answer_decision", "read_artifact"] satisfies Capability[],
}

describe("tas pairing offer", () => {
  test("classifies bind addresses by address shape", () => {
    expect(buildPairingOffer(input).payload.addresses).toEqual({
      loopback: "127.0.0.1",
      lan: "192.168.1.10",
      tailnet: "100.64.0.10",
    })
  })

  test("qr round-trips through the bootstrap payload codec", () => {
    const offer = buildPairingOffer(input)
    const decoded = decodeBootstrapPayload(offer.qr)

    expect({
      version: decoded.version,
      addresses: decoded.addresses,
      bootstrapId: decoded.bootstrapId,
      secret: decoded.secret,
      projectionLevel: decoded.projectionLevel,
      capabilities: decoded.capabilities,
    }).toEqual(offer.payload)
  })

  test("carries the secret without logging it", () => {
    const log = spyOn(console, "log").mockImplementation(() => {})
    const info = spyOn(console, "info").mockImplementation(() => {})
    const warn = spyOn(console, "warn").mockImplementation(() => {})
    const error = spyOn(console, "error").mockImplementation(() => {})

    try {
      const offer = buildPairingOffer(input)
      const decoded = decodeBootstrapPayload(offer.qr)

      expect(offer.payload.secret).toBe("secret-1")
      expect(decoded.secret).toBe("secret-1")
      expect(log).not.toHaveBeenCalled()
      expect(info).not.toHaveBeenCalled()
      expect(warn).not.toHaveBeenCalled()
      expect(error).not.toHaveBeenCalled()
    } finally {
      log.mockRestore()
      info.mockRestore()
      warn.mockRestore()
      error.mockRestore()
    }
  })
})
