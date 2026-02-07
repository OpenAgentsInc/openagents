import { describe, expect, it } from "vitest"
import { Effect, Fiber, SubscriptionRef } from "effect"
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

describe("RouterService (contract)", () => {
  it("stop removes navigation listeners (no click interception, no popstate listener)", async () => {
    const root = document.createElement("div")
    root.innerHTML = `
      <div data-effuse-shell>
        <div data-effuse-outlet>SSR</div>
        <a href="/a">Go</a>
      </div>
    `
    document.body.appendChild(root)

    const shell = root.querySelector("[data-effuse-shell]")!

    let pushes = 0
    let replaces = 0
    const listeners = new Set<(url: URL) => void>()
    let current = new URL("https://example.test/")

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

    let swaps = 0
    const outlet = root.querySelector("[data-effuse-outlet]")!
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

    await Effect.runPromise(
      Effect.gen(function* () {
        const router = yield* makeRouter({
          routes: [a],
          history,
          shell,
          sessionScopeKey: Effect.succeed("anon"),
        })

        yield* router.start
        expect(listeners.size).toBe(1)

        yield* router.stop
        expect(listeners.size).toBe(0)

        // Click after stop must not be intercepted.
        const link = root.querySelector("a") as HTMLAnchorElement
        link.dispatchEvent(new MouseEvent("click", { bubbles: true }))
        yield* Effect.sleep("20 millis")

        expect(pushes).toBe(0)
        expect(replaces).toBe(0)
        expect(swaps).toBe(0)
        expect(root.querySelector('[data-page="a"]')).toBeNull()
        expect(root.querySelector("[data-effuse-outlet]")?.textContent).toContain("SSR")
      }).pipe(Effect.provideService(DomServiceTag, dom))
    )

    root.remove()
  })

  it("prefetch does not mutate history or swap DOM, and warms the cache for cache-first routes", async () => {
    const root = document.createElement("div")
    root.innerHTML = `<div data-effuse-shell><div data-effuse-outlet>SSR</div></div>`
    document.body.appendChild(root)

    let started = 0
    let finished = 0
    let swaps = 0
    let pushes = 0
    let replaces = 0

    const dom = {
      ...DomServiceLive,
      swap: (target: Element, content: any, mode?: any) => {
        swaps++
        return DomServiceLive.swap(target, content, mode)
      },
    }

    const cached: Route<{ readonly n: number }> = {
      id: "/prefetch-cached",
      match: matchExact("/prefetch-cached"),
      loader: () =>
        Effect.gen(function* () {
          started++
          yield* Effect.sleep("10 millis")
          finished++
          return RouteOutcome.ok(
            { n: finished },
            { cache: { mode: "cache-first" } } // ttlMs undefined => cache forever
          )
        }),
      view: (_ctx, data) =>
        Effect.succeed(
          html`<div data-page="prefetch-cached" data-n="${String(data.n)}">cached</div>`
        ),
    }

    const baseHref = "https://example.test/"
    const history: RouterHistory = {
      ...makeMemoryHistory(baseHref),
      push: (url) => {
        pushes++
        ;(history as any)._current = url
      },
      replace: (url) => {
        replaces++
        ;(history as any)._current = url
      },
      current: () => (history as any)._current ?? new URL(baseHref),
    }
    ;(history as any)._current = new URL(baseHref)

    const shell = root.querySelector("[data-effuse-shell]")!

    await Effect.runPromise(
      Effect.gen(function* () {
        const router = yield* makeRouter({
          routes: [cached],
          history,
          shell,
          sessionScopeKey: Effect.succeed("anon"),
        })

        yield* router.prefetch("/prefetch-cached")
        expect(started).toBe(1)
        expect(finished).toBe(1)
        expect(swaps).toBe(0)
        expect(pushes).toBe(0)
        expect(replaces).toBe(0)
        expect(history.current().href).toBe(baseHref)

        // Navigate should use the warmed cache (no re-run).
        yield* router.navigate("/prefetch-cached")
        expect(started).toBe(1)
        expect(finished).toBe(1)
        expect(swaps).toBe(1)
        expect(pushes).toBe(1)
        expect(replaces).toBe(0)
        expect(root.querySelector('[data-page="prefetch-cached"]')?.getAttribute("data-n")).toBe(
          "1"
        )
      }).pipe(Effect.provideService(DomServiceTag, dom))
    )

    root.remove()
  })

  it("cache-first: uses cached RouteRun without re-running loader (ttlMs undefined)", async () => {
    const root = document.createElement("div")
    root.innerHTML = `<div data-effuse-shell><div data-effuse-outlet>SSR</div></div>`
    document.body.appendChild(root)

    let started = 0
    let finished = 0

    const dom = DomServiceLive

    const cached: Route<{ readonly n: number }> = {
      id: "/cached",
      match: matchExact("/cached"),
      loader: () =>
        Effect.gen(function* () {
          started++
          yield* Effect.sleep("20 millis")
          finished++
          return RouteOutcome.ok(
            { n: finished },
            { cache: { mode: "cache-first" } } // ttlMs undefined => cache forever
          )
        }),
      view: (_ctx, data) =>
        Effect.succeed(html`<div data-page="cached" data-n="${String(data.n)}">cached</div>`),
    }

    const history = makeMemoryHistory("https://example.test/")
    const shell = root.querySelector("[data-effuse-shell]")!

    await Effect.runPromise(
      Effect.gen(function* () {
        const router = yield* makeRouter({
          routes: [cached],
          history,
          shell,
          sessionScopeKey: Effect.succeed("anon"),
        })

        yield* router.navigate("/cached")
        expect(started).toBe(1)
        expect(finished).toBe(1)
        expect(root.querySelector('[data-page="cached"]')?.getAttribute("data-n")).toBe(
          "1"
        )

        // Second navigation should reuse cached run immediately (no new loader).
        yield* router.navigate("/cached")
        expect(started).toBe(1)
        expect(finished).toBe(1)
        expect(root.querySelector('[data-page="cached"]')?.getAttribute("data-n")).toBe(
          "1"
        )
      }).pipe(Effect.provideService(DomServiceTag, dom))
    )

    root.remove()
  })

  it("cache-first: re-runs loader after ttlMs expires", async () => {
    const root = document.createElement("div")
    root.innerHTML = `<div data-effuse-shell><div data-effuse-outlet>SSR</div></div>`
    document.body.appendChild(root)

    let started = 0
    let finished = 0

    const dom = DomServiceLive

    const cached: Route<{ readonly n: number }> = {
      id: "/cached-ttl",
      match: matchExact("/cached-ttl"),
      loader: () =>
        Effect.gen(function* () {
          started++
          yield* Effect.sleep("10 millis")
          finished++
          return RouteOutcome.ok(
            { n: finished },
            { cache: { mode: "cache-first", ttlMs: 5 } }
          )
        }),
      view: (_ctx, data) =>
        Effect.succeed(html`<div data-page="cached-ttl" data-n="${String(data.n)}">cached</div>`),
    }

    const history = makeMemoryHistory("https://example.test/")
    const shell = root.querySelector("[data-effuse-shell]")!

    await Effect.runPromise(
      Effect.gen(function* () {
        const router = yield* makeRouter({
          routes: [cached],
          history,
          shell,
          sessionScopeKey: Effect.succeed("anon"),
        })

        yield* router.navigate("/cached-ttl")
        expect(started).toBe(1)
        expect(finished).toBe(1)
        expect(root.querySelector('[data-page="cached-ttl"]')?.getAttribute("data-n")).toBe(
          "1"
        )

        // Wait long enough for ttl to expire.
        yield* Effect.sleep("20 millis")

        // Next navigation should re-run loader.
        yield* router.navigate("/cached-ttl")
        expect(started).toBe(2)
        expect(finished).toBe(2)
        expect(root.querySelector('[data-page="cached-ttl"]')?.getAttribute("data-n")).toBe(
          "2"
        )
      }).pipe(Effect.provideService(DomServiceTag, dom))
    )

    root.remove()
  })

  it("stale-while-revalidate: renders stale immediately, then refreshes in background", async () => {
    const root = document.createElement("div")
    root.innerHTML = `<div data-effuse-shell><div data-effuse-outlet>SSR</div></div>`
    document.body.appendChild(root)

    const shell = root.querySelector("[data-effuse-shell]")!
    const outlet = root.querySelector("[data-effuse-outlet]")!

    let started = 0
    let finished = 0
    let outletSwaps = 0

    const dom = {
      ...DomServiceLive,
      swap: (target: Element, content: any, mode?: any) => {
        if (target === outlet) outletSwaps++
        return DomServiceLive.swap(target, content, mode)
      },
    }

    const swr: Route<{ readonly n: number }> = {
      id: "/swr",
      match: matchExact("/swr"),
      loader: () =>
        Effect.gen(function* () {
          started++
          yield* Effect.sleep("50 millis")
          finished++
          return RouteOutcome.ok(
            { n: finished },
            { cache: { mode: "stale-while-revalidate", ttlMs: 0, swrMs: 1_000 } }
          )
        }),
      view: (_ctx, data) =>
        Effect.succeed(html`<div data-page="swr" data-n="${String(data.n)}">swr</div>`),
    }

    const history = makeMemoryHistory("https://example.test/")

    await Effect.runPromise(
      Effect.gen(function* () {
        const router = yield* makeRouter({
          routes: [swr],
          history,
          shell,
          sessionScopeKey: Effect.succeed("anon"),
        })

        // First navigation populates cache.
        yield* router.navigate("/swr")
        expect(started).toBe(1)
        expect(finished).toBe(1)
        expect(root.querySelector('[data-page="swr"]')?.getAttribute("data-n")).toBe(
          "1"
        )
        expect(outletSwaps).toBe(1)

        // Ensure cache age > ttlMs (0) so SWR stale path triggers.
        yield* Effect.sleep("5 millis")

        // Second navigation should render stale immediately and schedule a refresh.
        yield* router.navigate("/swr")
        expect(finished).toBe(1)
        expect(root.querySelector('[data-page="swr"]')?.getAttribute("data-n")).toBe(
          "1"
        )
        expect(outletSwaps).toBe(2)

        // After refresh completes, outlet should update to n=2.
        yield* Effect.sleep("80 millis")
        expect(started).toBe(2)
        expect(finished).toBe(2)
        expect(root.querySelector('[data-page="swr"]')?.getAttribute("data-n")).toBe(
          "2"
        )
        expect(outletSwaps).toBe(3)
      }).pipe(Effect.provideService(DomServiceTag, dom))
    )

    root.remove()
  })

  it("stale-while-revalidate: refresh MUST NOT apply after navigating away", async () => {
    const root = document.createElement("div")
    root.innerHTML = `<div data-effuse-shell><div data-effuse-outlet>SSR</div></div>`
    document.body.appendChild(root)

    const shell = root.querySelector("[data-effuse-shell]")!
    const outlet = root.querySelector("[data-effuse-outlet]")!

    let started = 0
    let finished = 0
    let interrupted = 0
    let outletSwaps = 0

    const dom = {
      ...DomServiceLive,
      swap: (target: Element, content: any, mode?: any) => {
        if (target === outlet) outletSwaps++
        return DomServiceLive.swap(target, content, mode)
      },
    }

    let resolveRefreshStarted!: () => void
    const refreshStarted = new Promise<void>((r) => {
      resolveRefreshStarted = r
    })

    const swr: Route<{ readonly n: number }> = {
      id: "/swr-away",
      match: matchExact("/swr-away"),
      loader: () =>
        Effect.gen(function* () {
          started++
          if (started === 2) resolveRefreshStarted()
          yield* Effect.sleep("50 millis")
          finished++
          return RouteOutcome.ok(
            { n: finished },
            { cache: { mode: "stale-while-revalidate", ttlMs: 0, swrMs: 1_000 } }
          )
        }).pipe(
          Effect.onInterrupt(() => Effect.sync(() => void interrupted++))
        ),
      view: (_ctx, data) =>
        Effect.succeed(
          html`<div data-page="swr-away" data-n="${String(data.n)}">swr</div>`
        ),
    }

    const fast: Route<{}> = {
      id: "/fast-away",
      match: matchExact("/fast-away"),
      loader: () => Effect.succeed(RouteOutcome.ok({})),
      view: () => Effect.succeed(html`<div data-page="fast-away">fast</div>`),
    }

    const history = makeMemoryHistory("https://example.test/")

    await Effect.runPromise(
      Effect.gen(function* () {
        const router = yield* makeRouter({
          routes: [swr, fast],
          history,
          shell,
          sessionScopeKey: Effect.succeed("anon"),
        })

        // First navigation populates cache.
        yield* router.navigate("/swr-away")
        expect(finished).toBe(1)
        expect(outletSwaps).toBe(1)

        yield* Effect.sleep("5 millis")

        // Second navigation triggers SWR stale render + refresh in background.
        yield* router.navigate("/swr-away")
        expect(outletSwaps).toBe(2)

        // Ensure the refresh loader actually started (so we are testing the
        // "don't apply after navigate away" case, not "no refresh scheduled").
        yield* Effect.promise(() => refreshStarted)

        // Navigate away before refresh completes.
        yield* router.navigate("/fast-away")
        expect(root.querySelector('[data-page=\"fast-away\"]')).not.toBeNull()
        expect(outletSwaps).toBe(3)

        // Wait long enough for the refresh loader to finish.
        yield* Effect.sleep("80 millis")
        expect(started).toBe(2)
        // Refresh should be canceled because its loader key is no longer needed.
        expect(finished).toBe(1)
        expect(interrupted).toBe(1)

        // Refresh MUST NOT have swapped us back to /swr-away.
        expect(root.querySelector('[data-page=\"fast-away\"]')).not.toBeNull()
        expect(root.querySelector('[data-page=\"swr-away\"]')).toBeNull()
        expect(outletSwaps).toBe(3)
      }).pipe(Effect.provideService(DomServiceTag, dom))
    )

    root.remove()
  })

  it("dedupes concurrent prefetches for the same loader key", async () => {
    const root = document.createElement("div")
    root.innerHTML = `<div data-effuse-shell><div data-effuse-outlet>SSR</div></div>`
    document.body.appendChild(root)

    let started = 0
    let finished = 0
    let swaps = 0

    const dom = {
      ...DomServiceLive,
      swap: (target: Element, content: any, mode?: any) => {
        swaps++
        return DomServiceLive.swap(target, content, mode)
      },
    }

    const slow: Route<{}> = {
      id: "/slow",
      match: matchExact("/slow"),
      loader: () =>
        Effect.gen(function* () {
          started++
          yield* Effect.sleep("50 millis")
          finished++
          return RouteOutcome.ok({})
        }),
      view: () => Effect.succeed(html`<div data-page="slow">slow</div>`),
    }

    const history = makeMemoryHistory("https://example.test/")
    const shell = root.querySelector("[data-effuse-shell]")!

    await Effect.runPromise(
      Effect.gen(function* () {
        const router = yield* makeRouter({
          routes: [slow],
          history,
          shell,
          sessionScopeKey: Effect.succeed("anon"),
        })

        yield* Effect.all([router.prefetch("/slow"), router.prefetch("/slow")], {
          concurrency: "unbounded",
        })
      }).pipe(Effect.provideService(DomServiceTag, dom))
    )

    expect(started).toBe(1)
    expect(finished).toBe(1)
    expect(swaps).toBe(0)

    root.remove()
  })

  it("switch-latest navigation cancels the previous navigation apply", async () => {
    const root = document.createElement("div")
    root.innerHTML = `<div data-effuse-shell><div data-effuse-outlet>SSR</div></div>`
    document.body.appendChild(root)

    let slowStarted = 0
    let slowFinished = 0
    let slowInterrupted = 0
    let resolveStarted!: () => void
    const started = new Promise<void>((r) => {
      resolveStarted = r
    })

    const dom = DomServiceLive

    const slow: Route<{}> = {
      id: "/slow",
      match: matchExact("/slow"),
      loader: () =>
        Effect.gen(function* () {
          slowStarted++
          resolveStarted()
          yield* Effect.sleep("50 millis")
          slowFinished++
          return RouteOutcome.ok({})
        }).pipe(
          Effect.onInterrupt(() => Effect.sync(() => void slowInterrupted++))
        ),
      view: () => Effect.succeed(html`<div data-page="slow">slow</div>`),
    }

    const fast: Route<{}> = {
      id: "/fast",
      match: matchExact("/fast"),
      loader: () => Effect.succeed(RouteOutcome.ok({})),
      view: () => Effect.succeed(html`<div data-page="fast">fast</div>`),
    }

    const history = makeMemoryHistory("https://example.test/")
    const shell = root.querySelector("[data-effuse-shell]")!

    await Effect.runPromise(
      Effect.gen(function* () {
        const router = yield* makeRouter({
          routes: [slow, fast],
          history,
          shell,
          sessionScopeKey: Effect.succeed("anon"),
        })

        // Start a slow navigation, then immediately navigate somewhere else.
        const slowFiber = yield* router
          .navigate("/slow")
          .pipe(Effect.catchAll(() => Effect.void), Effect.fork)

        // Ensure the slow loader actually starts before triggering a new navigation.
        yield* Effect.promise(() => started)

        const snapshot = yield* SubscriptionRef.get(router.state)
        expect(snapshot.key).not.toBeNull()
        const dbg = (router as any).__debug
        expect(dbg).toBeTruthy()
        expect(dbg.inflight.size).toBe(1)
        expect(dbg.inflight.has(snapshot.key)).toBe(true)
        const entry = dbg.inflight.get(snapshot.key)
        expect(entry?.fiber).not.toBeNull()
        expect(entry?.refCount).toBe(1)

        yield* router.navigate("/fast")

        // Give the interrupted slow loader time to finish if it wasn't canceled.
        yield* Effect.sleep("100 millis")

        yield* Fiber.interrupt(slowFiber)
      }).pipe(Effect.provideService(DomServiceTag, dom))
    )

    expect(slowStarted).toBe(1)
    expect(slowFinished).toBe(0)
    expect(slowInterrupted).toBe(1)
    expect(root.querySelector('[data-page="fast"]')).not.toBeNull()
    expect(root.querySelector('[data-page="slow"]')).toBeNull()

    root.remove()
  })

  it("does not cancel a shared in-flight loader fiber while a prefetch is still awaiting", async () => {
    const root = document.createElement("div")
    root.innerHTML = `<div data-effuse-shell><div data-effuse-outlet>SSR</div></div>`
    document.body.appendChild(root)

    let started = 0
    let finished = 0
    let resolveStarted!: () => void
    const startedPromise = new Promise<void>((r) => {
      resolveStarted = r
    })

    const dom = DomServiceLive

    const slow: Route<{}> = {
      id: "/slow",
      match: matchExact("/slow"),
      loader: () =>
        Effect.gen(function* () {
          started++
          resolveStarted()
          yield* Effect.sleep("50 millis")
          finished++
          return RouteOutcome.ok({})
        }),
      view: () => Effect.succeed(html`<div data-page="slow">slow</div>`),
    }

    const fast: Route<{}> = {
      id: "/fast",
      match: matchExact("/fast"),
      loader: () => Effect.succeed(RouteOutcome.ok({})),
      view: () => Effect.succeed(html`<div data-page="fast">fast</div>`),
    }

    const history = makeMemoryHistory("https://example.test/")
    const shell = root.querySelector("[data-effuse-shell]")!

    try {
      await Effect.runPromise(
        Effect.gen(function* () {
          const router = yield* makeRouter({
            routes: [slow, fast],
            history,
            shell,
            sessionScopeKey: Effect.succeed("anon"),
          })

          // Navigation starts the in-flight loader.
          const navFiber = yield* router
            .navigate("/slow")
            .pipe(Effect.catchAll(() => Effect.void), Effect.fork)

          yield* Effect.promise(() => startedPromise)

          // Prefetch joins the in-flight loader key/fiber.
          const prefetchFiber = yield* router.prefetch("/slow").pipe(Effect.fork)
          yield* Effect.sleep("1 millis")

          // Cancel navigation by navigating elsewhere, while prefetch is still waiting.
          yield* router.navigate("/fast")

          // Prefetch should still complete (and thus the shared loader must not be canceled).
          yield* Fiber.join(prefetchFiber)
          yield* Fiber.interrupt(navFiber)
        }).pipe(Effect.provideService(DomServiceTag, dom))
      )
    } catch (e) {
      // eslint-disable-next-line no-console
      console.error("router shared inflight failure:", e, (e as any)?.cause)
      throw e
    }

    expect(started).toBe(1)
    expect(finished).toBe(1)
    expect(root.querySelector('[data-page="fast"]')).not.toBeNull()
    expect(root.querySelector('[data-page="slow"]')).toBeNull()

    root.remove()
  })
})
