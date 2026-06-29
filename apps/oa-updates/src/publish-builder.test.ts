import { describe, expect, test } from "bun:test"

import { buildUpdateFromExport } from "./publish-builder"

const input = {
  id: "update_2026_06_13_001",
  platform: "ios" as const,
  branch: "production",
  runtimeVersion: "1.0.0",
  createdAt: "2026-06-13T12:34:56.000Z",
  baseUrl: "https://updates.openagents.com",
  launchBundle: {
    key: "bundles/ios/main.js",
    hash: "launch_hash_abc123",
  },
  assets: [
    {
      key: "assets/icon.png",
      hash: "icon_hash_def456",
      contentType: "image/png",
      fileExtension: ".png",
    },
    {
      key: "assets/font.ttf",
      hash: "font_hash_ghi789",
      contentType: "font/ttf",
      fileExtension: ".ttf",
    },
  ],
}

describe("buildUpdateFromExport", () => {
  test("builds content-addressed launch and asset urls", () => {
    const update = buildUpdateFromExport(input)

    expect(update.launchAsset).toEqual({
      key: "bundles/ios/main.js",
      hash: "launch_hash_abc123",
      contentType: "application/javascript",
      url: "https://updates.openagents.com/assets/launch_hash_abc123",
    })
    expect(update.assets).toEqual([
      {
        key: "assets/icon.png",
        hash: "icon_hash_def456",
        contentType: "image/png",
        fileExtension: ".png",
        url: "https://updates.openagents.com/assets/icon_hash_def456",
      },
      {
        key: "assets/font.ttf",
        hash: "font_hash_ghi789",
        contentType: "font/ttf",
        fileExtension: ".ttf",
        url: "https://updates.openagents.com/assets/font_hash_ghi789",
      },
    ])
  })

  test("carries manifest routing fields", () => {
    const update = buildUpdateFromExport(input)

    expect(update.platform).toBe("ios")
    expect(update.branch).toBe("production")
    expect(update.runtimeVersion).toBe("1.0.0")
    expect(update.createdAt).toBe("2026-06-13T12:34:56.000Z")
    expect(update.metadata).toEqual({})
    expect(update.extra).toEqual({})
  })

  test("returns deterministic output for fixed input", () => {
    const first = buildUpdateFromExport(input)
    const second = buildUpdateFromExport(input)

    expect(second).toEqual(first)
  })
})
