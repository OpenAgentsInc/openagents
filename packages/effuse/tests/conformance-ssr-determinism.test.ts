/* @vitest-environment node */
import { itLivePromise, withDom } from "./helpers/effectTest.ts"

import { describe, expect, it } from "@effect/vitest"
import { Effect } from "effect"
import {
  RouteOutcome,
  renderToString,
  runRoute,
  html,
  type Route,
  type RouteContext,
  type RouteMatch,
} from "../src/index.ts"

const matchExact =
  (pathname: string) =>
  (url: URL): RouteMatch | null => {
    if (url.pathname !== pathname) return null
    return { pathname, params: {}, search: url.searchParams }
  }

describe("conformance: SSR determinism", () => {
  itLivePromise("renderToString is stable for a fixed route run (no DOM required)", async () => {
    const route: Route<{ readonly hello: string }> = {
      id: "/ssr",
      match: matchExact("/ssr"),
      loader: () =>
        Effect.succeed(
          RouteOutcome.ok(
            { hello: "world" },
            { dehydrate: { example: true }, cache: { mode: "no-store" } }
          )
        ),
      head: (_ctx, data) =>
        Effect.succeed({
          title: `Hello ${data.hello}`,
          meta: [["description", data.hello]],
        }),
      view: (_ctx, data) =>
        Effect.succeed(
          html`<div data-hello="${data.hello}">Hello ${data.hello}</div>`
        ),
    }

    const url = new URL("https://example.test/ssr?x=1")
    const match: RouteMatch = {
      pathname: "/ssr",
      params: {},
      search: url.searchParams,
    }
    const ctx: RouteContext = {
      _tag: "Server",
      url,
      match,
      request: new Request(url),
    }

    const run1 = await Effect.runPromise(runRoute(route, ctx))
    const run2 = await Effect.runPromise(runRoute(route, ctx))

    expect(run1._tag).toBe("Ok")
    expect(run2._tag).toBe("Ok")

    if (run1._tag !== "Ok" || run2._tag !== "Ok") return

    const html1 = renderToString(run1.template)
    const html2 = renderToString(run2.template)
    expect(html1).toBe(html2)
    expect(html1).toBe(`<div data-hello="world">Hello world</div>`)

    expect(run1.head).toEqual(run2.head)
    expect(run1.head?.title).toBe("Hello world")
  })
})

