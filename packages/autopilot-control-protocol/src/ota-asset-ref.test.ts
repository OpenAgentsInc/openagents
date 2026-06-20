import { describe, expect, test } from "bun:test"

import { normalizeAsset } from "./ota-asset-ref.js"

describe("OTA asset reference normalization", () => {
  test("normalizes a complete expo-updates asset entry", () => {
    expect(normalizeAsset({
      key: "asset-key",
      contentType: "image/png",
      url: "https://updates.openagents.com/assets/asset-key",
      fileExtension: ".png",
    })).toEqual({
      key: "asset-key",
      contentType: "image/png",
      url: "https://updates.openagents.com/assets/asset-key",
      fileExtension: ".png",
    })
  })

  test("trims string fields before returning the asset reference", () => {
    expect(normalizeAsset({
      key: " asset-key ",
      contentType: " application/javascript ",
      url: " https://updates.openagents.com/assets/bundle ",
      fileExtension: " js ",
    })).toEqual({
      key: "asset-key",
      contentType: "application/javascript",
      url: "https://updates.openagents.com/assets/bundle",
      fileExtension: "js",
    })
  })

  test("returns null when the asset key is missing", () => {
    expect(normalizeAsset({
      contentType: "image/png",
      url: "https://updates.openagents.com/assets/asset-key",
      fileExtension: ".png",
    })).toBeNull()
  })

  test("returns null when the content type is missing", () => {
    expect(normalizeAsset({
      key: "asset-key",
      url: "https://updates.openagents.com/assets/asset-key",
      fileExtension: ".png",
    })).toBeNull()
  })

  test("returns null when the asset URL is missing", () => {
    expect(normalizeAsset({
      key: "asset-key",
      contentType: "image/png",
      fileExtension: ".png",
    })).toBeNull()
  })

  test("returns null for blank required strings", () => {
    expect(normalizeAsset({
      key: " ",
      contentType: "image/png",
      url: "https://updates.openagents.com/assets/asset-key",
    })).toBeNull()
  })

  test("normalizes absent or invalid file extensions to null", () => {
    expect(normalizeAsset({
      key: "asset-key",
      contentType: "image/png",
      url: "https://updates.openagents.com/assets/asset-key",
      fileExtension: "",
    })).toEqual({
      key: "asset-key",
      contentType: "image/png",
      url: "https://updates.openagents.com/assets/asset-key",
      fileExtension: null,
    })

    expect(normalizeAsset({
      key: "asset-key",
      contentType: "image/png",
      url: "https://updates.openagents.com/assets/asset-key",
      fileExtension: 42,
    })).toEqual({
      key: "asset-key",
      contentType: "image/png",
      url: "https://updates.openagents.com/assets/asset-key",
      fileExtension: null,
    })
  })

  test("returns null for non-object asset entries", () => {
    expect(normalizeAsset(null)).toBeNull()
    expect(normalizeAsset("asset-key")).toBeNull()
    expect(normalizeAsset([])).toBeNull()
  })
})
