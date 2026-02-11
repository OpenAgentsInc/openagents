import { describe, expect, it } from "@effect/vitest"

import { makeLoaderKey } from "../src/index.ts"

describe("makeLoaderKey (contract)", () => {
  it("is stable across params and search ordering", () => {
    const key1 = makeLoaderKey({
      routeId: "/r",
      match: {
        pathname: "/x",
        params: { b: "2", a: "1" },
        search: new URLSearchParams("b=2&a=1"),
      },
      sessionScopeKey: "anon",
    })

    const key2 = makeLoaderKey({
      routeId: "/r",
      match: {
        pathname: "/x",
        params: { a: "1", b: "2" },
        search: new URLSearchParams("a=1&b=2"),
      },
      sessionScopeKey: "anon",
    })

    expect(key1).toBe(key2)
  })

  it("is stable across repeated search param ordering (same key, different insertion)", () => {
    const s1 = new URLSearchParams()
    s1.append("tag", "b")
    s1.append("tag", "a")

    const s2 = new URLSearchParams()
    s2.append("tag", "a")
    s2.append("tag", "b")

    const key1 = makeLoaderKey({
      routeId: "/r",
      match: { pathname: "/x", params: {}, search: s1 },
      sessionScopeKey: "anon",
    })

    const key2 = makeLoaderKey({
      routeId: "/r",
      match: { pathname: "/x", params: {}, search: s2 },
      sessionScopeKey: "anon",
    })

    expect(key1).toBe(key2)
  })

  it("includes sessionScopeKey", () => {
    const a = makeLoaderKey({
      routeId: "/r",
      match: { pathname: "/x", params: {}, search: new URLSearchParams() },
      sessionScopeKey: "anon",
    })

    const b = makeLoaderKey({
      routeId: "/r",
      match: { pathname: "/x", params: {}, search: new URLSearchParams() },
      sessionScopeKey: "user:123",
    })

    expect(a).not.toBe(b)
  })
})

