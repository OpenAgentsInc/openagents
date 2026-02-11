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

const makeCountingMemoryHistory = (
  initialHref: string
): {
  readonly history: RouterHistory
  readonly counts: { pushes: () => number; replaces: () => number; current: () => URL }
} => {
  let current = new URL(initialHref)
  let pushes = 0
  let replaces = 0
  const listeners = new Set<(url: URL) => void>()
  return {
    history: {
      current: () => current,
      push: (url) => {
        pushes++
        current = url
        listeners.forEach((l) => l(url))
      },
      replace: (url) => {
        replaces++
        current = url
        listeners.forEach((l) => l(url))
      },
      listen: (listener) => {
        listeners.add(listener)
        return () => void listeners.delete(listener)
      },
    },
    counts: {
      pushes: () => pushes,
      replaces: () => replaces,
      current: () => current,
    },
  }
}

describe("RouterService link interception (contract)", () => {
  itLivePromise("intercepts same-origin left-clicks (no modifiers) and navigates", async () => {
    const root = document.createElement("div")
    root.innerHTML = `
      <div data-effuse-shell>
        <div data-effuse-outlet>SSR</div>
        <a href="/a">Go</a>
      </div>
    `
    document.body.appendChild(root)

    const shell = root.querySelector("[data-effuse-shell]")!
    const outlet = root.querySelector("[data-effuse-outlet]")!
    const link = root.querySelector("a") as HTMLAnchorElement

    const { history, counts } = makeCountingMemoryHistory("https://example.test/")

    let swaps = 0
    let resolveSwapped!: () => void
    const swapped = new Promise<void>((r) => {
      resolveSwapped = r
    })
    const dom = {
      ...DomServiceLive,
      swap: (target: Element, content: any, mode?: any) => {
        if (target === outlet) {
          swaps++
          resolveSwapped()
        }
        return DomServiceLive.swap(target, content, mode)
      },
    }

    const a: Route<{}> = {
      id: "/a",
      match: matchExact("/a"),
      loader: () => Effect.succeed(RouteOutcome.ok({})),
      view: () => Effect.succeed(html`<div data-page="a">a</div>`),
    }

    const evt = new MouseEvent("click", { bubbles: true, cancelable: true })
    let dispatchResult = true

    await Effect.runPromise(
      Effect.gen(function* () {
        const router = yield* makeRouter({
          routes: [a],
          history,
          shell,
          sessionScopeKey: Effect.succeed("anon"),
        })

        yield* router.start
        yield* Effect.sync(() => {
          dispatchResult = link.dispatchEvent(evt)
        })
        yield* Effect.promise(() => swapped)
      }).pipe(withDom(dom))
    )

    expect(dispatchResult).toBe(false)
    expect(evt.defaultPrevented).toBe(true)
    expect(counts.pushes()).toBe(1)
    expect(counts.replaces()).toBe(0)
    expect(counts.current().pathname).toBe("/a")
    expect(swaps).toBe(1)
    expect(root.querySelector('[data-page="a"]')).not.toBeNull()

    root.remove()
  })

  itLivePromise("does not intercept clicks with modifier keys", async () => {
    const root = document.createElement("div")
    root.innerHTML = `
      <div data-effuse-shell>
        <div data-effuse-outlet>SSR</div>
        <a href="/a">Go</a>
      </div>
    `
    document.body.appendChild(root)

    const shell = root.querySelector("[data-effuse-shell]")!
    const outlet = root.querySelector("[data-effuse-outlet]")!
    const link = root.querySelector("a") as HTMLAnchorElement

    const { history, counts } = makeCountingMemoryHistory("https://example.test/")

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
      loader: () => Effect.succeed(RouteOutcome.ok({})),
      view: () => Effect.succeed(html`<div data-page="a">a</div>`),
    }

    const evt = new MouseEvent("click", {
      bubbles: true,
      cancelable: true,
      metaKey: true,
    })
    let dispatchResult = true

    await Effect.runPromise(
      Effect.gen(function* () {
        const router = yield* makeRouter({
          routes: [a],
          history,
          shell,
          sessionScopeKey: Effect.succeed("anon"),
        })

        yield* router.start
        yield* Effect.sync(() => {
          dispatchResult = link.dispatchEvent(evt)
        })
        // Give the router a chance to do something if it incorrectly intercepted.
        yield* Effect.sleep("20 millis")
      }).pipe(withDom(dom))
    )

    expect(dispatchResult).toBe(true)
    expect(evt.defaultPrevented).toBe(false)
    expect(counts.pushes()).toBe(0)
    expect(swaps).toBe(0)
    expect(root.querySelector('[data-page="a"]')).toBeNull()

    root.remove()
  })

  itLivePromise("does not intercept cross-origin links", async () => {
    const root = document.createElement("div")
    root.innerHTML = `
      <div data-effuse-shell>
        <div data-effuse-outlet>SSR</div>
        <a href="https://other.example/a">Offsite</a>
      </div>
    `
    document.body.appendChild(root)

    const shell = root.querySelector("[data-effuse-shell]")!
    const outlet = root.querySelector("[data-effuse-outlet]")!
    const link = root.querySelector("a") as HTMLAnchorElement

    const { history, counts } = makeCountingMemoryHistory("https://example.test/")

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
      loader: () => Effect.succeed(RouteOutcome.ok({})),
      view: () => Effect.succeed(html`<div data-page="a">a</div>`),
    }

    const evt = new MouseEvent("click", { bubbles: true, cancelable: true })
    let dispatchResult = true

    await Effect.runPromise(
      Effect.gen(function* () {
        const router = yield* makeRouter({
          routes: [a],
          history,
          shell,
          sessionScopeKey: Effect.succeed("anon"),
        })

        yield* router.start
        yield* Effect.sync(() => {
          dispatchResult = link.dispatchEvent(evt)
        })
        yield* Effect.sleep("20 millis")
      }).pipe(withDom(dom))
    )

    expect(dispatchResult).toBe(true)
    expect(evt.defaultPrevented).toBe(false)
    expect(counts.pushes()).toBe(0)
    expect(swaps).toBe(0)
    expect(root.querySelector('[data-page="a"]')).toBeNull()

    root.remove()
  })
})

