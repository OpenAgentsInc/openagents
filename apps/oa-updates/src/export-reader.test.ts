import { join } from "node:path"

import { describe, expect, test } from "bun:test"

import { assetKeyFromBytes, createInMemoryAssetStore } from "./asset-store"
import { readExportedUpdate } from "./export-reader"

const bytes = (value: string): Uint8Array => new TextEncoder().encode(value)

describe("readExportedUpdate", () => {
  test("reads an Expo export from injected files and publishes content-addressed assets", async () => {
    const distDir = "/tmp/openagents-export/dist"
    const baseUrl = "https://updates.openagents.test"
    const store = createInMemoryAssetStore(baseUrl)
    const bundleBytes = bytes("console.log('ios bundle')")
    const iconBytes = bytes("png bytes")
    const dbBytes = bytes("sqlite bytes")
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
                assets: [
                  { path: "assets/icon.png", ext: ".png" },
                  { path: "assets/data.db", ext: "db" },
                ],
              },
            },
          }),
        ),
      ],
      [join(distDir, "bundles/ios/main.js"), bundleBytes],
      [join(distDir, "assets/icon.png"), iconBytes],
      [join(distDir, "assets/data.db"), dbBytes],
    ])
    const readFile = async (path: string): Promise<Uint8Array> => {
      const file = files.get(path)

      if (!file) {
        throw new Error(`Missing fake file: ${path}`)
      }

      return file
    }
    const bundleHash = assetKeyFromBytes(bundleBytes)
    const iconHash = assetKeyFromBytes(iconBytes)
    const dbHash = assetKeyFromBytes(dbBytes)

    const result = await readExportedUpdate({
      distDir,
      platform: "ios",
      branch: "production",
      runtimeVersion: "1.0.0",
      id: "update_2026_06_13_001",
      createdAt: "2026-06-13T12:34:56.000Z",
      baseUrl,
      store,
      readFile,
    })

    expect(result.update.launchAsset).toEqual({
      key: "bundles/ios/main.js",
      hash: bundleHash,
      contentType: "application/javascript",
      url: `${baseUrl}/assets/${bundleHash}`,
    })
    expect(result.update.assets).toEqual([
      {
        key: "assets/icon.png",
        hash: iconHash,
        contentType: "image/png",
        fileExtension: ".png",
        url: `${baseUrl}/assets/${iconHash}`,
      },
      {
        key: "assets/data.db",
        hash: dbHash,
        contentType: "application/octet-stream",
        fileExtension: ".db",
        url: `${baseUrl}/assets/${dbHash}`,
      },
    ])
    expect(await store.get(bundleHash)).toEqual(bundleBytes)
    expect(await store.get(iconHash)).toEqual(iconBytes)
    expect(await store.get(dbHash)).toEqual(dbBytes)
  })
})
