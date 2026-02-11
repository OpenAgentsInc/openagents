import { describe, expect, it } from "@effect/vitest"
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
import { itLivePromise, withDom } from "./helpers/effectTest.ts"

const matchExact =
  (pathname: string) =>
  (url: URL): RouteMatch | null => {
    if (url.pathname !== pathname) return null
    return { pathname, params: {}, search: url.searchParams }
  }

const makeCountingHistory = (initialHref: string) => {
  let current = new URL(initialHref)
  let pushes = 0
  let replaces = 0
  const listeners = new Set<(url: URL) => void>()

  const history: RouterHistory = {
    current: () => current,
    push: (url) => {
      pushes++
      current = url
    },
    replace: (url) => {
      replaces++
      current = url
    },
    listen: (listener) => {
      listeners.add(listener)
      return () => void listeners.delete(listener)
    },
  }

  return { history, counts: { pushes: () => pushes, replaces: () => replaces, current: () => current, listeners } }
}

describe("RouterService outcomes (contract)", () => {
  itLivePromise("redirect replaces history and commits only the final route", async () => {
    const root = document.createElement("div")
    root.innerHTML = `<div data-effuse-shell><div data-effuse-outlet>SSR</div></div>`
    document.body.appendChild(root)

    const shell = root.querySelector("[data-effuse-shell]")!
    const outlet = root.querySelector("[data-effuse-outlet]")!

    let swaps = 0
    const dom = {
      ...DomServiceLive,
      swap: (target: Element, content: any, mode?: any) => {
        if (target === outlet) swaps++
        return DomServiceLive.swap(target, content, mode)
      },
    }

    let viewRunsA = 0
    const a: Route<{}> = {
      id: "/a",
      match: matchExact("/a"),
      loader: () => Effect.succeed(RouteOutcome.redirect("/b", 302)),
      view: () =>
        Effect.sync(() => void viewRunsA++).pipe(
          Effect.as(html`<div data-page="a">a</div>`)
        ),
    }

    const b: Route<{}> = {
      id: "/b",
      match: matchExact("/b"),
      loader: () => Effect.succeed(RouteOutcome.ok({})),
      view: () => Effect.succeed(html`<div data-page="b">b</div>`),
    }

    const { history, counts } = makeCountingHistory("https://example.test/")

    await Effect.runPromise(
      Effect.gen(function* () {
        const router = yield* makeRouter({
          routes: [a, b],
          history,
          shell,
          sessionScopeKey: Effect.succeed("anon"),
        })

        yield* router.navigate("/a")
      }).pipe(withDom(dom))
    )

    expect(counts.pushes()).toBe(1)
    expect(counts.replaces()).toBe(1)
    expect(counts.current().pathname).toBe("/b")
    expect(swaps).toBe(1)
    expect(root.querySelector('[data-page="b"]')).not.toBeNull()
    expect(root.querySelector('[data-page="a"]')).toBeNull()
    expect(viewRunsA).toBe(0)

    root.remove()
  })

  itLivePromise("redirect loops fail deterministically after maxRedirects", async () => {
    const root = document.createElement("div")
    root.innerHTML = `<div data-effuse-shell><div data-effuse-outlet>SSR</div></div>`
    document.body.appendChild(root)

    const shell = root.querySelector("[data-effuse-shell]")!
    const outlet = root.querySelector("[data-effuse-outlet]")!

    let swaps = 0
    const dom = {
      ...DomServiceLive,
      swap: (target: Element, content: any, mode?: any) => {
        if (target === outlet) swaps++
        return DomServiceLive.swap(target, content, mode)
      },
    }

    const a: Route<{}> = {
      id: "/a",
      match: matchExact("/a"),
      loader: () => Effect.succeed(RouteOutcome.redirect("/a", 302)),
      view: () => Effect.succeed(html`<div data-page="a">a</div>`),
    }

    const { history, counts } = makeCountingHistory("https://example.test/")

    await Effect.runPromise(
      Effect.gen(function* () {
        const router = yield* makeRouter({
          routes: [a],
          history,
          shell,
          maxRedirects: 1,
          sessionScopeKey: Effect.succeed("anon"),
        })

        yield* router.navigate("/a")
      }).pipe(withDom(dom))
    )

    expect(counts.pushes()).toBe(1)
    expect(counts.replaces()).toBe(1)
    expect(counts.current().pathname).toBe("/a")
    expect(swaps).toBe(1)
    expect(root.querySelector('[data-effuse-error="fail"]')).not.toBeNull()
    expect(root.textContent).toContain("Too many redirects")

    root.remove()
  })

  itLivePromise("unknown routes render not-found and do not throw", async () => {
    const root = document.createElement("div")
    root.innerHTML = `<div data-effuse-shell><div data-effuse-outlet>SSR</div></div>`
    document.body.appendChild(root)

    const shell = root.querySelector("[data-effuse-shell]")!
    const outlet = root.querySelector("[data-effuse-outlet]")!

    let swaps = 0
    const dom = {
      ...DomServiceLive,
      swap: (target: Element, content: any, mode?: any) => {
        if (target === outlet) swaps++
        return DomServiceLive.swap(target, content, mode)
      },
    }

    const { history, counts } = makeCountingHistory("https://example.test/")

    await Effect.runPromise(
      Effect.gen(function* () {
        const router = yield* makeRouter({
          routes: [],
          history,
          shell,
          sessionScopeKey: Effect.succeed("anon"),
        })

        yield* router.navigate("/missing")
      }).pipe(withDom(dom))
    )

    expect(counts.pushes()).toBe(1)
    expect(counts.replaces()).toBe(0)
    expect(counts.current().pathname).toBe("/missing")
    expect(swaps).toBe(1)
    expect(root.querySelector('[data-effuse-error="not-found"]')).not.toBeNull()
    expect(root.textContent).toContain("/missing")

    root.remove()
  })

  itLivePromise("loader Fail renders error UI (view is not executed)", async () => {
    const root = document.createElement("div")
    root.innerHTML = `<div data-effuse-shell><div data-effuse-outlet>SSR</div></div>`
    document.body.appendChild(root)

    const shell = root.querySelector("[data-effuse-shell]")!
    const outlet = root.querySelector("[data-effuse-outlet]")!

    let swaps = 0
    const dom = {
      ...DomServiceLive,
      swap: (target: Element, content: any, mode?: any) => {
        if (target === outlet) swaps++
        return DomServiceLive.swap(target, content, mode)
      },
    }

    let viewRuns = 0
    const boom = new Error("boom")
    const fail: Route<{}> = {
      id: "/fail",
      match: matchExact("/fail"),
      loader: () => Effect.succeed(RouteOutcome.fail(boom, 500)),
      view: () =>
        Effect.sync(() => void viewRuns++).pipe(
          Effect.as(html`<div data-page="fail">should-not-render</div>`)
        ),
    }

    const { history, counts } = makeCountingHistory("https://example.test/")

    await Effect.runPromise(
      Effect.gen(function* () {
        const router = yield* makeRouter({
          routes: [fail],
          history,
          shell,
          sessionScopeKey: Effect.succeed("anon"),
        })

        yield* router.navigate("/fail")
      }).pipe(withDom(dom))
    )

    expect(counts.pushes()).toBe(1)
    expect(counts.replaces()).toBe(0)
    expect(counts.current().pathname).toBe("/fail")
    expect(swaps).toBe(1)
    expect(root.querySelector('[data-effuse-error="fail"]')).not.toBeNull()
    expect(root.textContent).toContain("boom")
    expect(root.querySelector('[data-page="fail"]')).toBeNull()
    expect(viewRuns).toBe(0)

    root.remove()
  })
})

