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

  test("serves Khala Code desktop feeds on a product-specific path", async () => {
    const server = createUpdatesServer()

    server.registerDesktopUpdate(
      "rc",
      {
        version: "0.1.0-rc.1",
        artifactUrl: "https://updates.openagents.test/assets/khala-code-rc",
        sha256: "khala-sha",
      },
      "khala-code-desktop",
    )

    const khalaResponse = await server.fetch(
      new Request(
        "https://updates.openagents.test/desktop/khala-code-desktop/rc/feed.json",
      ),
    )
    const defaultResponse = await server.fetch(
      new Request("https://updates.openagents.test/desktop/rc/feed.json"),
    )

    expect(khalaResponse.status).toBe(200)
    expect(await khalaResponse.json()).toEqual([
      {
        version: "0.1.0-rc.1",
        artifactUrl: "https://updates.openagents.test/assets/khala-code-rc",
        sha256: "khala-sha",
      },
    ])
    expect(await defaultResponse.json()).toEqual([])
  })

  test("serves signed pylon feeds by channel + platform, drops yanked", async () => {
    const server = createUpdatesServer()
    const release = (version: string, extra = {}) => ({
      version,
      channel: "rc" as const,
      platform: "darwin-arm64" as const,
      artifactUrl: `https://updates.openagents.test/assets/${version}`,
      sha256: "0".repeat(64),
      signature: "sig",
      kid: "2dbe811d19f67528",
      ...extra,
    })
    server.registerPylonUpdate(release("1.0.0-rc.1"))
    server.registerPylonUpdate(release("1.0.0-rc.2"))
    server.registerPylonUpdate(release("1.0.0-rc.3", { yanked: true }))
    // off-platform entry must not leak into the darwin feed
    server.registerPylonUpdate({ ...release("9.9.9"), platform: "linux-x64" })

    const response = await server.fetch(
      new Request(
        "https://updates.openagents.test/pylon/rc/darwin-arm64/feed.json",
      ),
    )
    expect(response.status).toBe(200)
    expect(response.headers.get("cache-control")).toBe("no-store")
    const feed = (await response.json()) as {
      schema: string
      platform: string
      releases: { version: string }[]
    }
    expect(feed.schema).toBe("openagents.pylon.feed.v1")
    expect(feed.platform).toBe("darwin-arm64")
    expect(feed.releases.map((r) => r.version)).toEqual([
      "1.0.0-rc.2",
      "1.0.0-rc.1",
    ])
  })

  test("rejects unknown pylon platform with 404", async () => {
    const server = createUpdatesServer()
    const response = await server.fetch(
      new Request("https://updates.openagents.test/pylon/rc/windows-x64/feed.json"),
    )
    expect(response.status).toBe(404)
  })
})
