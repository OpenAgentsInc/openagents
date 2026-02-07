import { describe, expect, it } from "vitest"

import { cachePolicyToCacheControlDirectives } from "../src/app/cache-control.js"

describe("cachePolicyToCacheControlDirectives", () => {
  it("maps no-store", () => {
    expect(cachePolicyToCacheControlDirectives({ mode: "no-store" })).toBe("no-store")
  })

  it("maps cache-first with ttlMs", () => {
    expect(cachePolicyToCacheControlDirectives({ mode: "cache-first", ttlMs: 5_000 })).toBe(
      "max-age=5"
    )
  })

  it("does not map cache-first without ttlMs", () => {
    expect(cachePolicyToCacheControlDirectives({ mode: "cache-first" })).toBeNull()
  })

  it("maps stale-while-revalidate", () => {
    expect(
      cachePolicyToCacheControlDirectives({
        mode: "stale-while-revalidate",
        ttlMs: 5_000,
        swrMs: 2_000,
      })
    ).toBe("max-age=5, stale-while-revalidate=2")
  })
})

