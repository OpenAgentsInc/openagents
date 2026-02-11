import { describe, expect, it } from "@effect/vitest"
import { Effect } from "effect"
import { itLivePromise, withDom } from "./helpers/effectTest.ts"

import { cacheControlForRouteRun, cachePolicyToCacheControlDirectives } from "../src/app/cache-control.js"
import { RouteOutcome, html, runRoute, type Route, type RouteContext, type RouteMatch } from "../src/index.ts"

const matchExact =
  (pathname: string) =>
  (url: URL): RouteMatch | null => {
    if (url.pathname !== pathname) return null
    return { pathname, params: {}, search: url.searchParams }
  }

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

describe("cacheControlForRouteRun", () => {
  itLivePromise("never caches non-Ok outcomes", async () => {
    const route: Route<{}> = {
      id: "/a",
      match: matchExact("/a"),
      loader: () => Effect.succeed(RouteOutcome.redirect("/b", 302)),
      view: () => Effect.succeed(html`<div>never</div>`),
    }

    const url = new URL("https://example.test/a")
    const ctx: RouteContext = {
      _tag: "Server",
      url,
      match: route.match(url)!,
      request: new Request(url),
    }

    const run = await Effect.runPromise(runRoute(route, ctx))
    expect(run._tag).toBe("Redirect")
    expect(cacheControlForRouteRun(run)).toBe("no-store")
  })

  itLivePromise("defaults to no-store when no cache policy is present", async () => {
    const route: Route<{}> = {
      id: "/a",
      match: matchExact("/a"),
      loader: () => Effect.succeed(RouteOutcome.ok({})),
      view: () => Effect.succeed(html`<div>ok</div>`),
    }

    const url = new URL("https://example.test/a")
    const ctx: RouteContext = {
      _tag: "Server",
      url,
      match: route.match(url)!,
      request: new Request(url),
    }

    const run = await Effect.runPromise(runRoute(route, ctx))
    expect(run._tag).toBe("Ok")
    expect(cacheControlForRouteRun(run)).toBe("no-store")
  })

  itLivePromise("uses private caching only when the cache policy maps to durable directives", async () => {
    const route: Route<{}> = {
      id: "/a",
      match: matchExact("/a"),
      loader: () =>
        Effect.succeed(
          RouteOutcome.ok(
            {},
            { cache: { mode: "cache-first", ttlMs: 5_000 } }
          )
        ),
      view: () => Effect.succeed(html`<div>ok</div>`),
    }

    const url = new URL("https://example.test/a")
    const ctx: RouteContext = {
      _tag: "Server",
      url,
      match: route.match(url)!,
      request: new Request(url),
    }

    const run = await Effect.runPromise(runRoute(route, ctx))
    expect(run._tag).toBe("Ok")
    expect(cacheControlForRouteRun(run)).toBe("private, max-age=5")
  })

  itLivePromise("does not cache when cookie mutations are present (implies Set-Cookie)", async () => {
    const route: Route<{}> = {
      id: "/a",
      match: matchExact("/a"),
      loader: () =>
        Effect.succeed(
          RouteOutcome.ok(
            {},
            {
              cache: { mode: "cache-first", ttlMs: 5_000 },
              cookies: [{ _tag: "Set", name: "x", value: "y" }],
            }
          )
        ),
      view: () => Effect.succeed(html`<div>ok</div>`),
    }

    const url = new URL("https://example.test/a")
    const ctx: RouteContext = {
      _tag: "Server",
      url,
      match: route.match(url)!,
      request: new Request(url),
    }

    const run = await Effect.runPromise(runRoute(route, ctx))
    expect(run._tag).toBe("Ok")
    expect(cacheControlForRouteRun(run)).toBe("no-store")
  })
})
