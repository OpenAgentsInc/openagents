import { describe, expect, it } from "@effect/vitest"
import { Effect } from "effect"
import { itLivePromise, withDom } from "./helpers/effectTest.ts"

import { RouteOutcome, html, runRoute, type Route, type RouteContext, type RouteMatch } from "../src/index.ts"

const matchExact =
  (pathname: string) =>
  (url: URL): RouteMatch | null => {
    if (url.pathname !== pathname) return null
    return { pathname, params: {}, search: url.searchParams }
  }

describe("runRoute (contract)", () => {
  itLivePromise("runs guard before loader and short-circuits on Redirect", async () => {
    let loaderRuns = 0

    const route: Route<{}> = {
      id: "/a",
      match: matchExact("/a"),
      guard: () => Effect.succeed(RouteOutcome.redirect("/login", 302)),
      loader: () =>
        Effect.sync(() => void loaderRuns++).pipe(Effect.as(RouteOutcome.ok({}))),
      view: () => Effect.succeed(html`<div>ok</div>`),
    }

    const url = new URL("https://example.test/a")
    const ctx: RouteContext = { _tag: "Client", url, match: route.match(url)! }

    const run = await Effect.runPromise(runRoute(route, ctx))
    expect(run._tag).toBe("Redirect")
    expect(run.href).toBe("/login")
    expect(loaderRuns).toBe(0)
  })

  itLivePromise("treats Route.guard returning Ok as a Fail (stage=guard)", async () => {
    const route: Route<{}> = {
      id: "/a",
      match: matchExact("/a"),
      guard: () =>
        // Guard is only allowed to short-circuit with non-Ok outcomes.
        Effect.succeed(RouteOutcome.ok(null as never) as any),
      loader: () => Effect.succeed(RouteOutcome.ok({})),
      view: () => Effect.succeed(html`<div>ok</div>`),
    }

    const url = new URL("https://example.test/a")
    const ctx: RouteContext = { _tag: "Client", url, match: route.match(url)! }

    const run = await Effect.runPromise(runRoute(route, ctx))
    expect(run._tag).toBe("Fail")
    expect(run.stage).toBe("guard")
    expect(String(run.error)).toContain("Route.guard MUST NOT return Ok")
  })

  itLivePromise("normalizes loader defects into Fail (stage=loader)", async () => {
    const route: Route<{}> = {
      id: "/a",
      match: matchExact("/a"),
      loader: () => Effect.die(new Error("boom")),
      view: () => Effect.succeed(html`<div>ok</div>`),
    }

    const url = new URL("https://example.test/a")
    const ctx: RouteContext = { _tag: "Client", url, match: route.match(url)! }

    const run = await Effect.runPromise(runRoute(route, ctx))
    expect(run._tag).toBe("Fail")
    expect(run.stage).toBe("loader")
  })

  itLivePromise("normalizes head defects into Fail (stage=head)", async () => {
    const route: Route<{}> = {
      id: "/a",
      match: matchExact("/a"),
      loader: () => Effect.succeed(RouteOutcome.ok({})),
      head: () => Effect.die(new Error("head")),
      view: () => Effect.succeed(html`<div>ok</div>`),
    }

    const url = new URL("https://example.test/a")
    const ctx: RouteContext = { _tag: "Client", url, match: route.match(url)! }

    const run = await Effect.runPromise(runRoute(route, ctx))
    expect(run._tag).toBe("Fail")
    expect(run.stage).toBe("head")
  })

  itLivePromise("normalizes view defects into Fail (stage=view)", async () => {
    const route: Route<{}> = {
      id: "/a",
      match: matchExact("/a"),
      loader: () => Effect.succeed(RouteOutcome.ok({})),
      view: () => Effect.die(new Error("view")),
    }

    const url = new URL("https://example.test/a")
    const ctx: RouteContext = { _tag: "Client", url, match: route.match(url)! }

    const run = await Effect.runPromise(runRoute(route, ctx))
    expect(run._tag).toBe("Fail")
    expect(run.stage).toBe("view")
  })

  itLivePromise("uses hydration/navigation defaults and respects route overrides", async () => {
    const route: Route<{}> = {
      id: "/a",
      match: matchExact("/a"),
      hydration: "soft",
      navigation: { swap: "document" },
      loader: () => Effect.succeed(RouteOutcome.ok({})),
      view: () => Effect.succeed(html`<div>ok</div>`),
    }

    const url = new URL("https://example.test/a")
    const ctx: RouteContext = { _tag: "Client", url, match: route.match(url)! }

    const run = await Effect.runPromise(runRoute(route, ctx))
    expect(run._tag).toBe("Ok")
    expect(run.hydration).toBe("soft")
    expect(run.navigationSwap).toBe("document")
  })
})

