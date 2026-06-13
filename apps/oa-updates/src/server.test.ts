import { generateKeyPairSync } from "node:crypto"
import { describe, expect, test } from "bun:test"

import { verifyManifestSignature } from "./code-signing.ts"
import { createUpdatesServer } from "./server.ts"
import type { Update } from "./manifest-resolver.ts"

const manifestRequest = (runtimeVersion: string): Request =>
  new Request("http://updates.openagents.test/openagents/manifest", {
    headers: {
      "Expo-Platform": "ios",
      "Expo-Runtime-Version": runtimeVersion,
      "Expo-Channel-Name": "production",
    },
  })

describe("updates server", () => {
  test("serves a resolved manifest from registered updates", async () => {
    const server = createUpdatesServer({ port: 4321 })
    const launchBytes = new TextEncoder().encode("console.log('launch')")
    const imageBytes = new Uint8Array([1, 2, 3, 4])
    const launchAsset = await server.putAsset(launchBytes)
    const imageAsset = await server.putAsset(imageBytes)
    const update: Update = {
      id: "update-1",
      platform: "ios",
      branch: "production",
      runtimeVersion: "1.0.0",
      createdAt: "2026-06-13T12:00:00.000Z",
      launchAsset: {
        key: "bundles/ios/main.js",
        hash: launchAsset.hash,
        contentType: "application/javascript",
        fileExtension: ".js",
        url: launchAsset.url,
      },
      assets: [
        {
          key: "assets/icon.png",
          hash: imageAsset.hash,
          contentType: "image/png",
          fileExtension: ".png",
          url: imageAsset.url,
        },
      ],
      metadata: {},
      extra: {},
    }

    server.registerUpdate(update)

    const response = await server.fetch(manifestRequest("1.0.0"))

    expect(response.status).toBe(200)
    expect(response.headers.get("content-type")).toContain("application/json")
    expect(response.headers.get("expo-protocol-version")).toBe("1")
    expect(response.headers.get("expo-sfv-version")).toBe("0")
    expect(await response.json()).toEqual(update)
  })

  test("returns noUpdateAvailable for a runtimeVersion mismatch", async () => {
    const server = createUpdatesServer()
    const launchAsset = await server.putAsset(new TextEncoder().encode("launch"))

    server.registerUpdate({
      id: "update-1",
      platform: "ios",
      branch: "production",
      runtimeVersion: "1.0.0",
      createdAt: "2026-06-13T12:00:00.000Z",
      launchAsset: {
        key: "bundles/ios/main.js",
        hash: launchAsset.hash,
        contentType: "application/javascript",
        fileExtension: ".js",
        url: launchAsset.url,
      },
      assets: [],
      metadata: {},
      extra: {},
    })

    const response = await server.fetch(manifestRequest("2.0.0"))

    expect(response.status).toBe(200)
    expect(await response.json()).toEqual({ type: "noUpdateAvailable" })
  })

  test("signs manifest responses when a signing key is configured", async () => {
    const { privateKey, publicKey } = generateKeyPairSync("rsa", {
      modulusLength: 2048,
    })
    const signingKeyPem = privateKey.export({
      type: "pkcs1",
      format: "pem",
    })
    const publicKeyPem = publicKey.export({
      type: "pkcs1",
      format: "pem",
    })
    const server = createUpdatesServer({
      signingKeyPem,
      keyid: "test-key",
    })
    const launchBytes = new TextEncoder().encode("console.log('launch')")
    const imageBytes = new Uint8Array([1, 2, 3, 4])
    const launchAsset = await server.putAsset(launchBytes)
    const imageAsset = await server.putAsset(imageBytes)
    const update: Update = {
      id: "update-1",
      platform: "ios",
      branch: "production",
      runtimeVersion: "1.0.0",
      createdAt: "2026-06-13T12:00:00.000Z",
      launchAsset: {
        key: "bundles/ios/main.js",
        hash: launchAsset.hash,
        contentType: "application/javascript",
        fileExtension: ".js",
        url: launchAsset.url,
      },
      assets: [
        {
          key: "assets/icon.png",
          hash: imageAsset.hash,
          contentType: "image/png",
          fileExtension: ".png",
          url: imageAsset.url,
        },
      ],
      metadata: {},
      extra: {},
    }

    server.registerUpdate(update)

    const response = await server.fetch(manifestRequest("1.0.0"))
    const responseBodyText = await response.text()
    const signature = response.headers.get("expo-signature")

    expect(response.status).toBe(200)
    expect(response.headers.get("content-type")).toContain("application/json")
    expect(response.headers.get("expo-protocol-version")).toBe("1")
    expect(response.headers.get("expo-sfv-version")).toBe("0")
    expect(signature).not.toBeNull()
    expect(
      verifyManifestSignature(responseBodyText, signature ?? "", publicKeyPem),
    ).toBe(true)
    expect(JSON.parse(responseBodyText)).toEqual(update)
  })

  test("serves registered asset bytes by hash", async () => {
    const server = createUpdatesServer()
    const bytes = new Uint8Array([9, 8, 7, 6])
    const storedAsset = await server.putAsset(bytes)

    server.registerUpdate({
      id: "update-1",
      platform: "ios",
      branch: "production",
      runtimeVersion: "1.0.0",
      createdAt: "2026-06-13T12:00:00.000Z",
      launchAsset: {
        key: "bundles/ios/main.js",
        hash: "different-launch-hash",
        contentType: "application/javascript",
        fileExtension: ".js",
        url: "http://localhost:3000/assets/different-launch-hash",
      },
      assets: [
        {
          key: "assets/icon.png",
          hash: storedAsset.hash,
          contentType: "image/png",
          fileExtension: ".png",
          url: storedAsset.url,
        },
      ],
      metadata: {},
      extra: {},
    })

    const response = await server.fetch(
      new Request(`http://updates.openagents.test/assets/${storedAsset.hash}`),
    )

    expect(response.status).toBe(200)
    expect(response.headers.get("content-type")).toBe("image/png")
    expect(response.headers.get("cache-control")).toBe(
      "public, max-age=31536000, immutable",
    )
    expect(new Uint8Array(await response.arrayBuffer())).toEqual(bytes)
  })
})
