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

describe("conformance: hydration modes", () => {
  itLivePromise("soft hydration: RouterService.start performs one initial navigation apply", async () => {
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

    const soft: Route<{}> = {
      id: "/soft",
      match: matchExact("/soft"),
      hydration: "soft",
      loader: () =>
        Effect.sync(() => void loaderRuns++).pipe(Effect.as(RouteOutcome.ok({}))),
      view: () => Effect.succeed(html`<div data-page="soft">soft</div>`),
    }

    const history = makeMemoryHistory("https://example.test/soft")

    await Effect.runPromise(
      Effect.gen(function* () {
        const router = yield* makeRouter({
          routes: [soft],
          history,
          shell,
          sessionScopeKey: Effect.succeed("anon"),
        })

        yield* router.start
        yield* Effect.promise(() => swapped)
      }).pipe(withDom(dom))
    )

    expect(loaderRuns).toBe(1)
    expect(swaps).toBe(1)
    expect(root.querySelector('[data-page="soft"]')).not.toBeNull()
    expect(root.querySelector("[data-effuse-outlet]")?.textContent).not.toContain("SSR outlet")

    root.remove()
  })

  itLivePromise("client-only hydration: RouterService.start performs one initial navigation apply", async () => {
    const root = document.createElement("div")
    root.innerHTML = `
      <div data-effuse-shell>
        <div data-effuse-outlet></div>
      </div>
    `
    document.body.appendChild(root)

    const shell = root.querySelector("[data-effuse-shell]")!
    const outlet = root.querySelector("[data-effuse-outlet]")!

    let swaps = 0
    let loaderRuns = 0
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

    const clientOnly: Route<{}> = {
      id: "/client-only",
      match: matchExact("/client-only"),
      hydration: "client-only",
      loader: () =>
        Effect.sync(() => void loaderRuns++).pipe(Effect.as(RouteOutcome.ok({}))),
      view: () => Effect.succeed(html`<div data-page="client-only">client</div>`),
    }

    const history = makeMemoryHistory("https://example.test/client-only")

    await Effect.runPromise(
      Effect.gen(function* () {
        const router = yield* makeRouter({
          routes: [clientOnly],
          history,
          shell,
          sessionScopeKey: Effect.succeed("anon"),
        })

        yield* router.start
        yield* Effect.promise(() => swapped)
      }).pipe(withDom(dom))
    )

    expect(loaderRuns).toBe(1)
    expect(swaps).toBe(1)
    expect(root.querySelector('[data-page="client-only"]')).not.toBeNull()

    root.remove()
  })
})

