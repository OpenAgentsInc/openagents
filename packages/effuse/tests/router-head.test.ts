import { describe, expect, it } from "vitest"
import { Effect } from "effect"
import {
  DomServiceLive,
  DomServiceTag,
  RouteOutcome,
  html,
  makeRouter,
  type History as RouterHistory,
  type Route,
  type RouteMatch,
} from "../src/index.ts"

const matchExact =
  (pathname: string) =>
  (url: URL): RouteMatch | null => {
    if (url.pathname !== pathname) return null
    return { pathname, params: {}, search: url.searchParams }
  }

const makeMemoryHistory = (initialHref: string): RouterHistory => {
  let current = new URL(initialHref)
  const listeners = new Set<(url: URL) => void>()
  return {
    current: () => current,
    push: (url) => {
      current = url
    },
    replace: (url) => {
      current = url
    },
    listen: (listener) => {
      listeners.add(listener)
      return () => void listeners.delete(listener)
    },
  }
}

describe("RouterService head management (contract)", () => {
  it("clears previous router-managed meta tags on navigation even when the next route has no head", async () => {
    const root = document.createElement("div")
    root.innerHTML = `
      <div data-effuse-shell>
        <div data-effuse-outlet>SSR</div>
      </div>
    `
    document.body.appendChild(root)

    const shell = root.querySelector("[data-effuse-shell]")!
    // SSR meta tags must be removable on the first SPA navigation.
    const ssrMeta = document.createElement("meta")
    ssrMeta.setAttribute("name", "description")
    ssrMeta.setAttribute("content", "ssr")
    ssrMeta.setAttribute("data-effuse-meta", "1")
    document.head.appendChild(ssrMeta)

    const persistent = document.createElement("meta")
    persistent.setAttribute("name", "viewport")
    persistent.setAttribute("content", "width=device-width")
    document.head.appendChild(persistent)

    const withHead: Route<{}> = {
      id: "/a",
      match: matchExact("/a"),
      loader: () => Effect.succeed(RouteOutcome.ok({})),
      head: () =>
        Effect.succeed({
          title: "A",
          meta: [
            ["og:title", "A"],
            ["description", "route-a"],
          ],
        }),
      view: () => Effect.succeed(html`<div data-page="a">a</div>`),
    }

    const noHead: Route<{}> = {
      id: "/b",
      match: matchExact("/b"),
      loader: () => Effect.succeed(RouteOutcome.ok({})),
      view: () => Effect.succeed(html`<div data-page="b">b</div>`),
    }

    const history = makeMemoryHistory("https://example.test/")

    await Effect.runPromise(
      Effect.gen(function* () {
        const router = yield* makeRouter({
          routes: [withHead, noHead],
          history,
          shell,
          sessionScopeKey: Effect.succeed("anon"),
        })

        // First navigation replaces SSR meta tags with route A meta tags.
        yield* router.navigate("/a")
        expect(document.title).toBe("A")
        expect(document.head.querySelectorAll('meta[data-effuse-meta="1"]').length).toBe(2)
        expect(
          document.head.querySelector('meta[name="description"][content="route-a"]')
        ).not.toBeNull()

        // Navigating to a route without head must clear A's meta tags (no stale meta).
        yield* router.navigate("/b")
        expect(document.head.querySelectorAll('meta[data-effuse-meta="1"]').length).toBe(0)

        // Non-router meta must remain.
        expect(
          document.head.querySelector('meta[name="viewport"][content="width=device-width"]')
        ).not.toBeNull()
      }).pipe(Effect.provideService(DomServiceTag, DomServiceLive))
    )

    ssrMeta.remove()
    persistent.remove()
    root.remove()
  })

  it("replaces router-managed meta tags (no duplicates) when navigating between routes with head", async () => {
    const root = document.createElement("div")
    root.innerHTML = `
      <div data-effuse-shell>
        <div data-effuse-outlet>SSR</div>
      </div>
    `
    document.body.appendChild(root)

    const shell = root.querySelector("[data-effuse-shell]")!

    const a: Route<{}> = {
      id: "/a",
      match: matchExact("/a"),
      loader: () => Effect.succeed(RouteOutcome.ok({})),
      head: () =>
        Effect.succeed({
          title: "A",
          meta: [["description", "a"]],
        }),
      view: () => Effect.succeed(html`<div data-page="a">a</div>`),
    }

    const b: Route<{}> = {
      id: "/b",
      match: matchExact("/b"),
      loader: () => Effect.succeed(RouteOutcome.ok({})),
      head: () =>
        Effect.succeed({
          title: "B",
          meta: [
            ["description", "b"],
            ["og:title", "B"],
          ],
        }),
      view: () => Effect.succeed(html`<div data-page="b">b</div>`),
    }

    const history = makeMemoryHistory("https://example.test/")

    await Effect.runPromise(
      Effect.gen(function* () {
        const router = yield* makeRouter({
          routes: [a, b],
          history,
          shell,
          sessionScopeKey: Effect.succeed("anon"),
        })

        yield* router.navigate("/a")
        expect(document.title).toBe("A")
        expect(
          document.head.querySelector('meta[name="description"][content="a"]')
        ).not.toBeNull()
        expect(document.head.querySelectorAll('meta[data-effuse-meta="1"]').length).toBe(1)

        yield* router.navigate("/b")
        expect(document.title).toBe("B")
        expect(
          document.head.querySelector('meta[name="description"][content="a"]')
        ).toBeNull()
        expect(
          document.head.querySelector('meta[name="description"][content="b"]')
        ).not.toBeNull()
        expect(
          document.head.querySelector('meta[name="og:title"][content="B"]')
        ).not.toBeNull()
        expect(document.head.querySelectorAll('meta[data-effuse-meta="1"]').length).toBe(2)
      }).pipe(Effect.provideService(DomServiceTag, DomServiceLive))
    )

    document.head.querySelectorAll('meta[data-effuse-meta="1"]').forEach((m) => m.remove())
    root.remove()
  })
})

