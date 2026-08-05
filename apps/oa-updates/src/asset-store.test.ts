import { describe, expect, test } from "vite-plus/test"

import { assetKeyFromBytes } from "./asset-store"

const bytes = (value: string): Uint8Array => new TextEncoder().encode(value)

describe("asset store", () => {
  test("computes base64url sha256 without padding", () => {
    expect(assetKeyFromBytes(bytes("hello"))).toBe("LPJNul-wow4m6DsqxbninhsWHlwfp0JecwQzYpOLmCQ")
  })

  test("is a pure content address: identical bytes hash identically", () => {
    expect(assetKeyFromBytes(bytes("same asset"))).toBe(
      assetKeyFromBytes(bytes("same asset")),
    )
    expect(assetKeyFromBytes(bytes("bundle-v1"))).not.toBe(
      assetKeyFromBytes(bytes("bundle-v2")),
    )
  })
})
