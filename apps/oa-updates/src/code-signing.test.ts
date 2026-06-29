import { describe, expect, test } from "bun:test"
import { generateKeyPairSync } from "node:crypto"

import {
  parseSignatureHeader,
  signManifest,
  verifyManifestSignature,
} from "./code-signing"

describe("OpenAgents Updates code signing", () => {
  test("signs and verifies an Expo manifest signature header", () => {
    const { privateKey, publicKey } = generateKeyPairSync("rsa", {
      modulusLength: 2048,
    })
    const privateKeyPem = privateKey.export({
      format: "pem",
      type: "pkcs8",
    }) as string
    const publicKeyPem = publicKey.export({
      format: "pem",
      type: "spki",
    }) as string
    const manifestBytes = Buffer.from(
      JSON.stringify({
        id: "sample-update",
        createdAt: "2026-06-13T00:00:00.000Z",
        runtimeVersion: "1.0.0",
      }),
    )

    const headerValue = signManifest(manifestBytes, privateKeyPem)
    const parsed = parseSignatureHeader(headerValue)

    expect(parsed.sig).toBeString()
    expect(parsed.sig.length).toBeGreaterThan(0)
    expect(parsed.keyid).toBe("main")
    expect(parsed.alg).toBe("rsa-v1_5-sha256")
    expect(
      verifyManifestSignature(manifestBytes, headerValue, publicKeyPem),
    ).toBe(true)
    expect(
      verifyManifestSignature(
        Buffer.from(`${manifestBytes.toString("utf8")} `),
        headerValue,
        publicKeyPem,
      ),
    ).toBe(false)
  })
})
