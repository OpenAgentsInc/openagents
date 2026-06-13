import { describe, expect, test } from "bun:test"
import { inspect } from "node:util"

import {
  buildNodeRegistration,
  registrationKey,
} from "../src/node/discovery-register"

const input = {
  nodeRef: "node-1",
  binds: [
    { address: "127.0.0.1" },
    { address: "192.168.1.10" },
    { address: "100.64.0.10" },
  ],
  controlToken: "control-token-1",
  updatedAt: "2026-06-13T12:00:00.000Z",
}

describe("tas discovery register", () => {
  test("classifies bind addresses by address shape", () => {
    expect(buildNodeRegistration(input).addresses).toEqual({
      loopback: "127.0.0.1",
      lan: "192.168.1.10",
      tailnet: "100.64.0.10",
    })
  })

  test("registrationKey is deterministic for owner and node refs", () => {
    expect(registrationKey("owner-1", "node-1")).toBe("owner-1:node-1")
    expect(registrationKey("owner-1", "node-1")).toBe(
      registrationKey("owner-1", "node-1"),
    )
    expect(registrationKey("owner-2", "node-1")).not.toBe(
      registrationKey("owner-1", "node-1"),
    )
  })

  test("carries control token without leaking it through inspect helpers", () => {
    const registration = buildNodeRegistration(input)

    expect(registration.controlToken).toBe("control-token-1")
    expect(String(registration)).not.toContain("control-token-1")
    expect(inspect(registration)).not.toContain("control-token-1")
    expect(JSON.stringify(registration)).not.toContain("control-token-1")
  })
})
