import { describe, expect, test } from "bun:test"

import { assetKeyFromBytes, createInMemoryAssetStore } from "./asset-store"
import { publishExport } from "./publish"

const bytes = (value: string): Uint8Array => new TextEncoder().encode(value)

describe("publishExport", () => {
  test("stores launch bundle and assets, then builds a content-addressed update", async () => {
    const baseUrl = "https://updates.openagents.test"
    const store = createInMemoryAssetStore(baseUrl)
    const launchBytes = bytes("console.log('launch')")
    const iconBytes = bytes("png bytes")
    const fontBytes = bytes("font bytes")
    const launchHash = assetKeyFromBytes(launchBytes)
    const iconHash = assetKeyFromBytes(iconBytes)
    const fontHash = assetKeyFromBytes(fontBytes)

    const result = await publishExport({
      platform: "ios",
      branch: "production",
      runtimeVersion: "1.0.0",
      id: "update_2026_06_13_001",
      createdAt: "2026-06-13T12:34:56.000Z",
      baseUrl,
      store,
      launchBundle: {
        key: "bundles/ios/main.js",
        bytes: launchBytes,
      },
      assets: [
        {
          key: "assets/icon.png",
          bytes: iconBytes,
          contentType: "image/png",
          fileExtension: ".png",
        },
        {
          key: "assets/font.ttf",
          bytes: fontBytes,
          contentType: "font/ttf",
          fileExtension: ".ttf",
        },
      ],
    })

    expect(result.assetHashes).toEqual({
      launchBundle: launchHash,
      assets: [
        { key: "assets/icon.png", hash: iconHash },
        { key: "assets/font.ttf", hash: fontHash },
      ],
    })
    expect(result.update.launchAsset).toEqual({
      key: "bundles/ios/main.js",
      hash: launchHash,
      contentType: "application/javascript",
      url: `${baseUrl}/assets/${launchHash}`,
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
        key: "assets/font.ttf",
        hash: fontHash,
        contentType: "font/ttf",
        fileExtension: ".ttf",
        url: `${baseUrl}/assets/${fontHash}`,
      },
    ])
    expect(await store.get(launchHash)).toEqual(launchBytes)
    expect(await store.get(iconHash)).toEqual(iconBytes)
    expect(await store.get(fontHash)).toEqual(fontBytes)
  })
})
