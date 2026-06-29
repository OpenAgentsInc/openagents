import { inspect } from "node:util"

import { describe, expect, test } from "bun:test"

import type { BootstrapPayload } from "./bootstrap-payload.js"
import {
  decodeBootstrapPayload,
  encodeBootstrapPayload,
} from "./bootstrap-payload.js"

const payload: BootstrapPayload = {
  version: 1,
  addresses: {
    loopback: "http://127.0.0.1:8787",
    lan: "http://192.168.1.50:8787",
  },
  bootstrapId: "bootstrap.fixture.0001",
  secret: "pairing-secret.fixture.0001",
  projectionLevel: "team",
  capabilities: ["observe_public", "read_artifact", "answer_decision"],
}

describe("bootstrap pairing payload codec", () => {
  test("round-trips a compact base64url JSON payload", () => {
    const encoded = encodeBootstrapPayload(payload)
    const decoded = decodeBootstrapPayload(encoded)

    expect(encoded).toMatch(/^[A-Za-z0-9_-]+$/)
    expect({
      version: decoded.version,
      addresses: decoded.addresses,
      bootstrapId: decoded.bootstrapId,
      secret: decoded.secret,
      projectionLevel: decoded.projectionLevel,
      capabilities: decoded.capabilities,
    }).toEqual(payload)
  })

  test("rejects payloads without a bootstrap address", () => {
    expect(() =>
      encodeBootstrapPayload({
        ...payload,
        addresses: {},
      }),
    ).toThrow()
  })

  test("rejects payloads with a bad version", () => {
    const encoded = Buffer.from(JSON.stringify({
      ...payload,
      version: 2,
    })).toString("base64url")

    expect(() => decodeBootstrapPayload(encoded)).toThrow("Malformed bootstrap payload")
  })

  test("preserves secret access without exposing it in string or inspection output", () => {
    const decoded = decodeBootstrapPayload(encodeBootstrapPayload(payload))

    expect(decoded.secret).toBe(payload.secret)
    expect(String(decoded)).not.toContain(payload.secret)
    expect(inspect(decoded)).not.toContain(payload.secret)
    expect(JSON.stringify(decoded)).not.toContain(payload.secret)
  })
})
