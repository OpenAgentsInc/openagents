import { describe, expect, test } from "bun:test"

import { assetKeyFromBytes, createInMemoryAssetStore, verifyAsset } from "./asset-store"

const bytes = (value: string): Uint8Array => new TextEncoder().encode(value)

describe("asset store", () => {
  test("computes base64url sha256 without padding", () => {
    expect(assetKeyFromBytes(bytes("hello"))).toBe("LPJNul-wow4m6DsqxbninhsWHlwfp0JecwQzYpOLmCQ")
  })

  test("put then get round-trips bytes", async () => {
    const store = createInMemoryAssetStore("https://updates.openagents.test")
    const input = bytes("console.log('ota')")
    const asset = await store.put(input)

    expect(asset.url).toBe(`https://updates.openagents.test/assets/${asset.hash}`)
    expect(await store.get(asset.hash)).toEqual(input)
  })

  test("verifyAsset accepts matching bytes and rejects tampered bytes", () => {
    const input = bytes("bundle-v1")
    const hash = assetKeyFromBytes(input)

    expect(verifyAsset(input, hash)).toBe(true)
    expect(verifyAsset(bytes("bundle-v2"), hash)).toBe(false)
  })

  test("put is idempotent for identical bytes", async () => {
    const store = createInMemoryAssetStore("https://updates.openagents.test")

    const first = await store.put(bytes("same asset"))
    const second = await store.put(bytes("same asset"))

    expect(second).toEqual(first)
  })
})
