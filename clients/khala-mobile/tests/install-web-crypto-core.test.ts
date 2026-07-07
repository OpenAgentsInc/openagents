import { describe, expect, test } from "bun:test"

import { applyWebCryptoShim, type WebCryptoImpl } from "../src/native/install-web-crypto-core"

// Oracle for the Hermes "No secure random number generator" crash (build 15):
// @tanstack/db's safeRandomUUID needs crypto.getRandomValues/randomUUID, which
// Hermes lacks. The shim must fill the gap without clobbering a real impl.

const impl: WebCryptoImpl = {
  getRandomValues: <T>(typedArray: T): T => typedArray,
  randomUUID: () => "shim-uuid",
}

describe("applyWebCryptoShim", () => {
  test("creates crypto and installs both methods when absent (Hermes)", () => {
    const host: { crypto?: Record<string, unknown> } = {}
    applyWebCryptoShim(host, impl)
    expect(typeof host.crypto?.getRandomValues).toBe("function")
    expect(typeof host.crypto?.randomUUID).toBe("function")
    expect((host.crypto?.randomUUID as () => string)()).toBe("shim-uuid")
  })

  test("does NOT overwrite an already-present real implementation", () => {
    const nativeRandomUUID = () => "native-uuid"
    const nativeGetRandomValues = <T>(a: T): T => a
    const host = {
      crypto: {
        getRandomValues: nativeGetRandomValues as unknown as () => unknown,
        randomUUID: nativeRandomUUID as unknown as () => string,
      } as Record<string, unknown>,
    }
    applyWebCryptoShim(host, impl)
    expect(host.crypto.randomUUID).toBe(nativeRandomUUID as unknown as () => unknown)
    expect(host.crypto.getRandomValues).toBe(nativeGetRandomValues as unknown as () => unknown)
  })

  test("fills only the missing method on a partial native crypto", () => {
    const nativeGetRandomValues = <T>(a: T): T => a
    const host = {
      crypto: {
        getRandomValues: nativeGetRandomValues as unknown as () => unknown,
      } as Record<string, unknown>,
    }
    applyWebCryptoShim(host, impl)
    // existing method preserved
    expect(host.crypto.getRandomValues).toBe(nativeGetRandomValues as unknown as () => unknown)
    // missing method filled from the shim
    expect((host.crypto.randomUUID as () => string)()).toBe("shim-uuid")
  })
})
