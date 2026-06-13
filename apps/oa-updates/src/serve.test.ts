import { join } from "node:path"

import { describe, expect, test } from "bun:test"

import { createInMemoryAssetStore } from "./asset-store.ts"
import { seedFromDist } from "./serve.ts"
import { createUpdatesServer } from "./server.ts"

const bytes = (value: string): Uint8Array => new TextEncoder().encode(value)

const manifestRequest = (runtimeVersion: string): Request =>
  new Request("http://updates.openagents.test/openagents/manifest", {
    headers: {
      "Expo-Platform": "ios",
      "Expo-Runtime-Version": runtimeVersion,
      "Expo-Channel-Name": "production",
    },
  })

describe("seedFromDist", () => {
  test("registers a seeded update and serves its assets", async () => {
    const distDir = "/tmp/openagents-seed-export"
    const runtimeVersion = "seed-runtime-1"
    const baseUrl = "http://localhost:8080"
    const server = createUpdatesServer({ port: 8080 })
    const expectedStore = createInMemoryAssetStore(baseUrl)
    const bundleBytes = bytes("console.log('seed bundle')")
    const iconBytes = bytes("png bytes")
    const expectedLaunch = await expectedStore.put(bundleBytes)
    const expectedIcon = await expectedStore.put(iconBytes)
    const files = new Map<string, Uint8Array>([
      [
        join(distDir, "metadata.json"),
        bytes(
          JSON.stringify({
            version: 0,
            bundler: "metro",
            fileMetadata: {
              ios: {
                bundle: "bundles/ios/main.js",
                assets: [{ path: "assets/icon.png", ext: ".png" }],
              },
            },
          }),
        ),
      ],
      [join(distDir, "bundles/ios/main.js"), bundleBytes],
      [join(distDir, "assets/icon.png"), iconBytes],
    ])
    const readFile = async (path: string): Promise<Uint8Array> => {
      const file = files.get(path)

      if (!file) {
        throw new Error(`Missing fake file: ${path}`)
      }

      return file
    }

    await seedFromDist({
      server,
      distDir,
      platform: "ios",
      branch: "production",
      runtimeVersion,
      baseUrl,
      readFile,
    })

    const manifestResponse = await server.fetch(manifestRequest(runtimeVersion))
    // Expo Updates Protocol: manifest is a part of a multipart/mixed body.
    const ct = manifestResponse.headers.get("content-type") ?? ""
    const boundary = (ct.match(/boundary=([^;]+)/) ?? [])[1] ?? ""
    const bodyText = await manifestResponse.text()
    const manifestSeg = bodyText
      .split(`--${boundary}`)
      .map((s) => s.replace(/^\r\n/, "").replace(/\r\n$/, ""))
      .find((s) => s.includes('name="manifest"')) ?? ""
    const manifest = JSON.parse(manifestSeg.slice(manifestSeg.indexOf("\r\n\r\n") + 4)) as Record<string, any>

    expect(manifestResponse.status).toBe(200)
    expect(ct).toContain("multipart/mixed")
    expect(manifest.runtimeVersion).toBe(runtimeVersion)
    expect(manifest.branch).toBe("production")
    expect(manifest.launchAsset.hash).toBe(expectedLaunch.hash)
    expect(manifest.assets[0].hash).toBe(expectedIcon.hash)

    const bundleResponse = await server.fetch(
      new Request(manifest.launchAsset.url),
    )
    const iconResponse = await server.fetch(new Request(manifest.assets[0].url))

    expect(bundleResponse.status).toBe(200)
    expect(bundleResponse.headers.get("content-type")).toBe(
      "application/javascript",
    )
    expect(new Uint8Array(await bundleResponse.arrayBuffer())).toEqual(
      bundleBytes,
    )
    expect(iconResponse.status).toBe(200)
    expect(iconResponse.headers.get("content-type")).toBe("image/png")
    expect(new Uint8Array(await iconResponse.arrayBuffer())).toEqual(iconBytes)
  })
})
