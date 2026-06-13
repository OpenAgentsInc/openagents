import { describe, expect, test } from "bun:test"
import { generateKeyPairSync } from "node:crypto"

import { verifyManifestSignature } from "./code-signing"
import { buildSignedManifestResponse } from "./signed-response"

describe("signed manifest response", () => {
  test("signs the exact response body bytes", () => {
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

    const response = buildSignedManifestResponse({
      manifest: {
        id: "sample-update",
        createdAt: "2026-06-13T00:00:00.000Z",
        runtimeVersion: "1.0.0",
        launchAsset: {
          key: "bundle",
          url: "https://updates.openagents.com/assets/bundle.js",
        },
      },
      privateKeyPem,
    })

    expect(response.headers["content-type"]).toBe("application/json")
    expect(response.headers["expo-protocol-version"]).toBe("1")
    expect(response.headers["expo-sfv-version"]).toBe("0")
    expect(
      verifyManifestSignature(
        response.body,
        response.headers["expo-signature"],
        publicKeyPem,
      ),
    ).toBe(true)
    expect(
      verifyManifestSignature(
        response.body.replace("sample-update", "tampered-update"),
        response.headers["expo-signature"],
        publicKeyPem,
      ),
    ).toBe(false)
  })
})
