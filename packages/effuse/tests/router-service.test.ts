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
