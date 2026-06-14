import { generateKeyPairSync } from "node:crypto"
import { describe, expect, test } from "bun:test"

import { verifyManifestSignature } from "./code-signing.ts"
import { createUpdatesServer } from "./server.ts"
import type { Update } from "./manifest-resolver.ts"

// Parse a named part out of an expo-updates multipart/mixed response: returns
// the part's JSON body + its part headers. Mirrors what expo-updates does.
const parsePart = async (
  response: Response,
  name: string,
): Promise<{ json: unknown; rawBody: string; headers: Record<string, string> }> => {
  const contentType = response.headers.get("content-type") ?? ""
  const match = contentType.match(/boundary=([^;]+)/)
  if (!match) throw new Error(`not multipart: ${contentType}`)
  const boundary = match[1]
  const text = await response.text()
  const segments = text.split(`--${boundary}`)
  for (const seg of segments) {
    const trimmed = seg.replace(/^\r\n/, "").replace(/\r\n$/, "")
    if (trimmed === "" || trimmed === "--") continue
    const sep = trimmed.indexOf("\r\n\r\n")
    if (sep < 0) continue
    const headerBlock = trimmed.slice(0, sep)
    const body = trimmed.slice(sep + 4)
    const headers: Record<string, string> = {}
    for (const line of headerBlock.split("\r\n")) {
      const i = line.indexOf(":")
      if (i > 0) headers[line.slice(0, i).trim().toLowerCase()] = line.slice(i + 1).trim()
    }
    const disposition = headers["content-disposition"] ?? ""
    if (disposition.includes(`name="${name}"`)) {
      return { json: JSON.parse(body), rawBody: body, headers }
    }
  }
  throw new Error(`part "${name}" not found`)
}

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
    expect(response.headers.get("content-type")).toContain("multipart/mixed")
    expect(response.headers.get("expo-protocol-version")).toBe("1")
    expect(response.headers.get("expo-sfv-version")).toBe("0")
    const part = await parsePart(response, "manifest")
    expect(part.json).toEqual(update)
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
    expect(response.headers.get("content-type")).toContain("multipart/mixed")
    const part = await parsePart(response, "directive")
    expect(part.json).toEqual({ type: "noUpdateAvailable" })
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

    expect(response.status).toBe(200)
    expect(response.headers.get("content-type")).toContain("multipart/mixed")
    expect(response.headers.get("expo-protocol-version")).toBe("1")
    expect(response.headers.get("expo-sfv-version")).toBe("0")
    // Signature travels as a part header on the manifest part, over the part body.
    const part = await parsePart(response, "manifest")
    const signature = part.headers["expo-signature"]
    expect(signature).toBeDefined()
    expect(verifyManifestSignature(part.rawBody, signature ?? "", publicKeyPem)).toBe(true)
    expect(part.json).toEqual(update)
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

  test("serves desktop update feeds by channel", async () => {
    const server = createUpdatesServer()

    server.registerDesktopUpdate("stable", {
      version: "1.2.0",
      artifactUrl: "https://updates.openagents.test/assets/full-1.2.0",
      sha256: "full-sha",
      bsdiffFromVersion: "1.1.0",
      bsdiffUrl: "https://updates.openagents.test/assets/delta-1.1.0-1.2.0",
      bsdiffSha256: "delta-sha",
    })

    const response = await server.fetch(
      new Request("https://updates.openagents.test/desktop/stable/feed.json"),
    )

    expect(response.status).toBe(200)
    expect(response.headers.get("cache-control")).toBe("no-store")
    expect(await response.json()).toEqual([
      {
        version: "1.2.0",
        artifactUrl: "https://updates.openagents.test/assets/full-1.2.0",
        sha256: "full-sha",
        bsdiffFromVersion: "1.1.0",
        bsdiffUrl: "https://updates.openagents.test/assets/delta-1.1.0-1.2.0",
        bsdiffSha256: "delta-sha",
      },
    ])
  })
})
