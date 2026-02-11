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

describe("conformance: shell/outlet invariants", () => {
  itLivePromise("RouterService.start does not call DomService.swap (strict hydration)", async () => {
    const root = document.createElement("div")
    root.innerHTML = `
      <div data-effuse-shell>
        <div data-effuse-outlet>SSR outlet</div>
        <a href="/a">Go</a>
      </div>
    `
    document.body.appendChild(root)

    const shell = root.querySelector("[data-effuse-shell]")!

    let swaps = 0
    let loaderRuns = 0
    let viewRuns = 0
    const dom = {
      ...DomServiceLive,
      swap: (target: Element, content: any, mode?: any) => {
        swaps++
        return DomServiceLive.swap(target, content, mode)
      },
    }

    const a: Route<{}> = {
      id: "/a",
      match: matchExact("/a"),
      loader: () =>
        Effect.sync(() => void loaderRuns++).pipe(Effect.as(RouteOutcome.ok({}))),
      view: () =>
        Effect.sync(() => void viewRuns++).pipe(
          Effect.as(html`<div data-page="a">a</div>`)
        ),
    }

    const history = makeMemoryHistory("https://example.test/")

    await Effect.runPromise(
      Effect.gen(function* () {
        const router = yield* makeRouter({
          routes: [a],
          history,
          shell,
          sessionScopeKey: Effect.succeed("anon"),
        })

        yield* router.start
      }).pipe(withDom(dom))
    )

    expect(swaps).toBe(0)
    // MUST NOT re-run initial loader during strict hydration boot.
    expect(loaderRuns).toBe(0)
    expect(viewRuns).toBe(0)

    root.remove()
  })

  itLivePromise("strict hydration: RouterService.start does not re-run the initial loader even when the current URL matches a route", async () => {
    const root = document.createElement("div")
    root.innerHTML = `
      <div data-effuse-shell>
        <div data-effuse-outlet>SSR outlet</div>
      </div>
    `
    document.body.appendChild(root)

    const shell = root.querySelector("[data-effuse-shell]")!
    const outlet = root.querySelector("[data-effuse-outlet]")!

    let swaps = 0
    let loaderRuns = 0
    let viewRuns = 0
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
      loader: () =>
        Effect.sync(() => void loaderRuns++).pipe(Effect.as(RouteOutcome.ok({}))),
      view: () =>
        Effect.sync(() => void viewRuns++).pipe(
          Effect.as(html`<div data-page="a">a</div>`)
        ),
      hydration: "strict",
    }

    const history = makeMemoryHistory("https://example.test/a")

    await Effect.runPromise(
      Effect.gen(function* () {
        const router = yield* makeRouter({
          routes: [a],
          history,
          shell,
          sessionScopeKey: Effect.succeed("anon"),
        })

        yield* router.start
        // Give any mistakenly-forked initial navigation a chance to run.
        yield* Effect.sleep("20 millis")
      }).pipe(withDom(dom))
    )

    expect(swaps).toBe(0)
    expect(loaderRuns).toBe(0)
    expect(viewRuns).toBe(0)
    expect(root.innerHTML).toContain("SSR outlet")

    root.remove()
  })

  itLivePromise("navigations swap the outlet only by default (shell remains stable)", async () => {
    const root = document.createElement("div")
    root.innerHTML = `
      <div data-effuse-shell>
        <header data-sentinel="1">SHELL</header>
        <div data-effuse-outlet><div data-page="ssr">SSR</div></div>
      </div>
    `
    document.body.appendChild(root)

    const shell = root.querySelector("[data-effuse-shell]")!
    const outlet = root.querySelector("[data-effuse-outlet]")!
    const sentinel = root.querySelector("[data-sentinel]")!

    let shellSwaps = 0
    let outletSwaps = 0
    const dom = {
      ...DomServiceLive,
      swap: (target: Element, content: any, mode?: any) => {
        if (target === shell) shellSwaps++
        if (target === outlet) outletSwaps++
        return DomServiceLive.swap(target, content, mode)
      },
    }

    const a: Route<{}> = {
      id: "/a",
      match: matchExact("/a"),
      loader: () => Effect.succeed(RouteOutcome.ok({})),
      view: () => Effect.succeed(html`<div data-page="a">a</div>`),
    }

    const history = makeMemoryHistory("https://example.test/")

    await Effect.runPromise(
      Effect.gen(function* () {
        const router = yield* makeRouter({
          routes: [a],
          history,
          shell,
          sessionScopeKey: Effect.succeed("anon"),
        })

        yield* router.navigate("/a")
      }).pipe(withDom(dom))
    )

    expect(shellSwaps).toBe(0)
    expect(outletSwaps).toBe(1)
    expect(root.querySelector("[data-sentinel]")).toBe(sentinel)
    expect(root.querySelector('[data-page="a"]')).not.toBeNull()
    expect(root.querySelector('[data-page="ssr"]')).toBeNull()

    root.remove()
  })

  itLivePromise("routes can opt into document swaps explicitly", async () => {
    const root = document.createElement("div")
    root.innerHTML = `
      <div data-effuse-shell>
        <header data-sentinel="1">SHELL</header>
        <div data-effuse-outlet><div data-page="ssr">SSR</div></div>
      </div>
    `
    document.body.appendChild(root)

    const shell = root.querySelector("[data-effuse-shell]")!
    const outlet = root.querySelector("[data-effuse-outlet]")!

    let shellSwaps = 0
    let outletSwaps = 0
    const dom = {
      ...DomServiceLive,
      swap: (target: Element, content: any, mode?: any) => {
        if (target === shell) shellSwaps++
        if (target === outlet) outletSwaps++
        return DomServiceLive.swap(target, content, mode)
      },
    }

    const doc: Route<{}> = {
      id: "/doc",
      match: matchExact("/doc"),
      navigation: { swap: "document" },
      loader: () => Effect.succeed(RouteOutcome.ok({})),
      view: () =>
        Effect.succeed(
          html`<div data-page="doc"><div data-effuse-outlet>doc</div></div>`
        ),
    }

    const history = makeMemoryHistory("https://example.test/")

    await Effect.runPromise(
      Effect.gen(function* () {
        const router = yield* makeRouter({
          routes: [doc],
          history,
          shell,
          sessionScopeKey: Effect.succeed("anon"),
        })

        yield* router.navigate("/doc")
      }).pipe(withDom(dom))
    )

    expect(shellSwaps).toBe(1)
    expect(outletSwaps).toBe(0)
    expect(root.querySelector('[data-page="doc"]')).not.toBeNull()

    root.remove()
  })
})
