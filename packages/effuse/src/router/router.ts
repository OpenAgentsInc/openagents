/**
 * RouterService - Effuse-owned navigation + loader pipeline.
 *
 * v1 goals (see MASTER-PLAN-EFFECT-EFFUSE-COMPLETE.md §3.4.1):
 * - Stable loader keying (routeId + path + params + search + sessionScopeKey)
 * - In-flight dedupe (same key shares one fiber)
 * - Cancellation (switch-latest navigation; shared fibers only interrupted when unused)
 * - CachePolicy-aware caching (explicitly enabled by RouteOutcome.Ok hints)
 * - Shell/outlet swap invariant (default swaps outlet only)
 */

import { Clock, Deferred, Effect, Fiber, Ref, Runtime, SubscriptionRef } from "effect"
import type { CachePolicy, Route, RouteContext, RouteMatch } from "../app/route.js"
import { runRoute } from "../app/run.js"
import type { RouteRun } from "../app/run.js"
import type { DomService } from "../services/dom.js"
import { DomServiceTag } from "../services/dom.js"
import { html } from "../template/html.js"
import type { TemplateResult } from "../template/types.js"
import type { History } from "./history.js"
import { makeLoaderKey, type LoaderKey } from "./key.js"

export class RouterError {
  readonly _tag = "RouterError"
  constructor(
    readonly message: string,
    readonly cause?: unknown
  ) {}
}

export type RouterStatus = "idle" | "navigating" | "prefetching"

export type RouterState = {
  readonly status: RouterStatus
  readonly url: URL
  readonly key: LoaderKey | null
  readonly run: RouteRun<unknown> | null
}

export type NavigateOptions = {
  readonly replace?: boolean
}

export type RouterConfig<R> = {
  readonly routes: ReadonlyArray<Route<any, R>>
  readonly history: History
  readonly shell: Element
  readonly outletSelector?: string
  readonly maxRedirects?: number
  readonly sessionScopeKey: Effect.Effect<string, never, R>
  readonly renderNotFound?: (input: { readonly url: URL }) => Effect.Effect<TemplateResult, never, R>
  readonly renderError?: (input: {
    readonly url: URL
    readonly error: unknown
  }) => Effect.Effect<TemplateResult, never, R>
}

export type RouterService<R> = {
  readonly state: SubscriptionRef.SubscriptionRef<RouterState>

  /**
   * Start browser listeners (popstate + link interception).
   *
   * Strict hydration default: `start` MUST NOT call DomService.swap.
   */
  readonly start: Effect.Effect<void, RouterError, DomService | R>

  /**
   * Navigate to an internal URL (SPA).
   *
   * - switch-latest: interrupts previous navigation apply
   * - swaps outlet by default
   */
  readonly navigate: (
    href: string,
    options?: NavigateOptions
  ) => Effect.Effect<void, RouterError, DomService | R>

  /**
   * Prefetch route data (same loader pipeline + keying + cache rules),
   * without mutating history or DOM.
   */
  readonly prefetch: (href: string) => Effect.Effect<void, RouterError, DomService | R>
}

type AnyRoute<R> = Route<any, R>
type AnyRun = RouteRun<unknown>
type OkRun = Extract<AnyRun, { readonly _tag: "Ok" }>

type CacheEntry = {
  readonly storedAt: number
  readonly policy: CachePolicy
  readonly run: OkRun
}

type InflightEntry = {
  refCount: number
  readonly deferred: Deferred.Deferred<AnyRun>
  fiber: Fiber.RuntimeFiber<AnyRun, never> | null
}

type HistoryMode = "push" | "replace" | "none"

const defaultNotFound = ({ url }: { readonly url: URL }) =>
  Effect.succeed(
    html`<div data-effuse-error="not-found"><h1>Not found</h1><p>${url.pathname}</p></div>`
  )

const defaultError = ({ url, error }: { readonly url: URL; readonly error: unknown }) =>
  Effect.succeed(
    html`<div data-effuse-error="fail"><h1>Error</h1><p>${url.pathname}</p><pre>${String(
      error
    )}</pre></div>`
  )

const matchRoute = <R>(
  routes: ReadonlyArray<AnyRoute<R>>,
  url: URL
): { readonly route: AnyRoute<R>; readonly match: RouteMatch } | null => {
  for (const route of routes) {
    try {
      const match = route.match(url)
      if (match) {
        return { route, match }
      }
    } catch {
      // Ignore route.match exceptions (treated as no match).
    }
  }
  return null
}

const parseHref = (href: string, base: URL): URL | null => {
  try {
    return new URL(href, base)
  } catch {
    return null
  }
}

const applyHead = (run: OkRun) =>
  Effect.sync(() => {
    const head = run.head
    if (!head) return

    if (head.title !== undefined) {
      document.title = head.title
    }

    // Clear previous router-managed meta tags.
    document.head
      .querySelectorAll('meta[data-effuse-meta="1"]')
      .forEach((node) => node.remove())

    if (head.meta) {
      for (const [name, content] of head.meta) {
        const meta = document.createElement("meta")
        meta.setAttribute("name", name)
        meta.setAttribute("content", content)
        meta.setAttribute("data-effuse-meta", "1")
        document.head.appendChild(meta)
      }
    }
  })

const cacheKey = (run: OkRun): CachePolicy => run.hints?.cache ?? { mode: "no-store" }

const isCacheFresh = (policy: CachePolicy, ageMs: number): boolean => {
  switch (policy.mode) {
    case "no-store":
      return false
    case "cache-first":
      return policy.ttlMs == null ? true : ageMs <= policy.ttlMs
    case "stale-while-revalidate":
      return ageMs <= policy.ttlMs
  }
}

const isCacheStaleButAllowed = (policy: CachePolicy, ageMs: number): boolean => {
  if (policy.mode !== "stale-while-revalidate") return false
  return ageMs > policy.ttlMs && ageMs <= policy.ttlMs + policy.swrMs
}

const isCacheExpired = (policy: CachePolicy, ageMs: number): boolean => {
  switch (policy.mode) {
    case "no-store":
      return true
    case "cache-first":
      return policy.ttlMs == null ? false : ageMs > policy.ttlMs
    case "stale-while-revalidate":
      return ageMs > policy.ttlMs + policy.swrMs
  }
}

export const makeRouter = <R>(config: RouterConfig<R>): Effect.Effect<RouterService<R>> =>
  Effect.gen(function* () {
    const outletSelector = config.outletSelector ?? "[data-effuse-outlet]"
    const maxRedirects = config.maxRedirects ?? 10

    const cache = new Map<LoaderKey, CacheEntry>()
    const inflight = new Map<LoaderKey, InflightEntry>()
    const debug = { inflight, cache, lastCancelKey: null as LoaderKey | null }

    const navFiber = yield* Ref.make<Fiber.RuntimeFiber<void, RouterError> | null>(null)
    const navToken = yield* Ref.make(0)
    const currentNavKey = yield* Ref.make<LoaderKey | null>(null)
    const started = yield* Ref.make(false)

    const initialUrl = config.history.current()
    const state = yield* SubscriptionRef.make<RouterState>({
      status: "idle",
      url: initialUrl,
      key: null,
      run: null,
    })

    const swap = (run: AnyRun, template: TemplateResult) =>
      Effect.gen(function* () {
        const dom = yield* DomServiceTag

        const target =
          run._tag === "Ok" || run._tag === "Redirect" || run._tag === "NotFound" || run._tag === "Fail"
            ? run.navigationSwap === "document"
              ? config.shell
              : (config.shell.querySelector(outletSelector) ?? null)
            : (config.shell.querySelector(outletSelector) ?? null)

        if (!target) {
          return yield* Effect.fail(
            new RouterError(`Outlet element not found: ${outletSelector}`)
          )
        }

        yield* dom
          .swap(target, template, "inner")
          .pipe(Effect.mapError((e) => new RouterError(e.message, e)))
      })

    const putCacheIfEnabled = (key: LoaderKey, run: AnyRun) =>
      Effect.gen(function* () {
        if (run._tag !== "Ok") {
          return
        }

        const policy = cacheKey(run)
        if (policy.mode === "no-store") {
          return
        }

        const now = yield* Clock.currentTimeMillis
        yield* Effect.sync(() => {
          cache.set(key, { storedAt: now, policy, run })
        })
      })

    const runMatched = (
      url: URL,
      route: AnyRoute<R>,
      match: RouteMatch
    ): Effect.Effect<AnyRun, never, R> =>
      runRoute(route, { _tag: "Client", url, match } satisfies RouteContext)

    const acquireInflight = (
      key: LoaderKey,
      effect: Effect.Effect<AnyRun, never, R>
    ): Effect.Effect<AnyRun, never, R> =>
      Effect.uninterruptibleMask((restore) =>
        Effect.gen(function* () {
          const existing = yield* Effect.sync(() => inflight.get(key) ?? null)
          if (existing) {
            yield* Effect.sync(() => {
              existing.refCount++
            })

            return yield* restore(Deferred.await(existing.deferred)).pipe(
              Effect.ensuring(
                Effect.uninterruptible(
                  Effect.gen(function* () {
                    const fiberToInterrupt = yield* Effect.sync(() => {
                      const current = inflight.get(key)
                      if (current !== existing) return null
                      current.refCount--
                      if (current.refCount <= 0) {
                        inflight.delete(key)
                        return current.fiber
                      }
                      return null
                    })

                    if (fiberToInterrupt) {
                      // Interrupt the shared request only when unused.
                      yield* Fiber.interrupt(fiberToInterrupt).pipe(Effect.asVoid)
                    }
                  })
                )
              )
            )
          }

          const deferred = yield* Deferred.make<AnyRun>()
          const entry: InflightEntry = { refCount: 1, deferred, fiber: null }
          yield* Effect.sync(() => {
            inflight.set(key, entry)
          })

          // IMPORTANT: we are inside an uninterruptibleMask to make inflight
          // bookkeeping deterministic. The shared producer fiber itself MUST
          // remain interruptible (so switch-latest navigation can cancel it).
          yield* restore(effect).pipe(
            Effect.tap((run) => putCacheIfEnabled(key, run)),
            Effect.tap((run) => Deferred.complete(deferred, Effect.succeed(run))),
            Effect.ensuring(
              // Clean up inflight map on completion/interrupt.
              Effect.sync(() => {
                const current = inflight.get(key)
                if (current === entry) {
                  inflight.delete(key)
                }
              })
            ),
            // IMPORTANT: this fiber must not be auto-supervised by a single caller,
            // otherwise a cancelled navigation could kill a shared in-flight request
            // that a prefetch is still awaiting (violates §3.4.1).
            Effect.forkDaemon,
            // Avoid a yield point between forking and publishing the fiber handle.
            Effect.tap((fiber) =>
              Effect.sync(() => {
                entry.fiber = fiber
              })
            )
          )

          return yield* restore(Deferred.await(deferred)).pipe(
            Effect.ensuring(
              Effect.uninterruptible(
                Effect.gen(function* () {
                  const fiberToInterrupt = yield* Effect.sync(() => {
                    const current = inflight.get(key)
                    if (current !== entry) return null
                    current.refCount--
                    if (current.refCount <= 0) {
                      inflight.delete(key)
                      return current.fiber
                    }
                    return null
                  })

                  if (fiberToInterrupt) {
                    yield* Fiber.interrupt(fiberToInterrupt).pipe(Effect.asVoid)
                  }
                })
              )
            )
          )
        })
      )

    const resolveRun = (url: URL, mode: "navigate" | "prefetch") =>
      Effect.gen(function* () {
        const matched = matchRoute(config.routes, url)
        if (!matched) {
          const synthetic: AnyRun = {
            _tag: "NotFound",
            routeId: "(not-found)",
            match: { pathname: url.pathname, params: {}, search: url.searchParams },
            hydration: "strict",
            navigationSwap: "outlet",
          }
          return { key: null as LoaderKey | null, run: synthetic, refresh: null as null | Effect.Effect<AnyRun, never, R> }
        }

        const sessionScopeKey = yield* config.sessionScopeKey
        const key = makeLoaderKey({
          routeId: matched.route.id,
          match: matched.match,
          sessionScopeKey,
        })

        const now = yield* Clock.currentTimeMillis
        const cached = yield* Effect.sync(() => cache.get(key) ?? null)

        if (cached) {
          const ageMs = now - cached.storedAt
          const policy = cached.policy

          if (!isCacheExpired(policy, ageMs)) {
            if (mode === "prefetch") {
              // Prefetch: do nothing if still valid (fresh or within SWR window).
              if (!isCacheStaleButAllowed(policy, ageMs)) {
                return { key, run: cached.run as AnyRun, refresh: null as null | Effect.Effect<AnyRun, never, R> }
              }
            }

            if (mode === "navigate" && isCacheStaleButAllowed(policy, ageMs)) {
              // Navigate: render stale immediately, refresh in background.
              const refresh = acquireInflight(
                key,
                runMatched(url, matched.route, matched.match)
              )
              return { key, run: cached.run as AnyRun, refresh }
            }

            if (isCacheFresh(policy, ageMs)) {
              return { key, run: cached.run as AnyRun, refresh: null as null | Effect.Effect<AnyRun, never, R> }
            }
          }
        }

        const run = yield* acquireInflight(
          key,
          runMatched(url, matched.route, matched.match)
        )
        return { key, run, refresh: null as null | Effect.Effect<AnyRun, never, R> }
      })

    const computeLoaderKeyForUrl = (url: URL): Effect.Effect<LoaderKey | null, never, R> =>
      Effect.gen(function* () {
        const matched = matchRoute(config.routes, url)
        if (!matched) return null
        const sessionScopeKey = yield* config.sessionScopeKey
        return makeLoaderKey({
          routeId: matched.route.id,
          match: matched.match,
          sessionScopeKey,
        })
      })

    const cancelInflightIfSoleConsumer = (key: LoaderKey | null) =>
      Effect.gen(function* () {
        if (!key) return

        const fiberToInterrupt = yield* Effect.sync(() => {
          const entry = inflight.get(key)
          if (!entry) return null
          if (entry.refCount > 1) return null
          inflight.delete(key)
          return entry.fiber
        })

        if (fiberToInterrupt) {
          yield* Effect.sync(() => {
            debug.lastCancelKey = key
          })
          yield* Fiber.interrupt(fiberToInterrupt).pipe(Effect.asVoid)
        }
      })

    const applyOutcome = (
      token: number,
      url: URL,
      key: LoaderKey | null,
      run: AnyRun,
      redirectsLeft: number
    ): Effect.Effect<void, RouterError, DomService | R> =>
      Effect.gen(function* () {
        // Drop stale navigations (belt-and-suspenders with Fiber.interrupt).
        const currentToken = yield* Ref.get(navToken)
        if (token !== currentToken) {
          return
        }

        switch (run._tag) {
          case "Ok": {
            yield* applyHead(run as OkRun)
            yield* swap(run, run.template)
            const nextState: RouterState = {
              status: "idle",
              url,
              key,
              run,
            }
            yield* SubscriptionRef.set(state, nextState)
            return
          }
          case "NotFound": {
            const render = config.renderNotFound ?? defaultNotFound
            const t = yield* render({ url })
            yield* swap(run, t)
            const nextState: RouterState = {
              status: "idle",
              url,
              key,
              run,
            }
            yield* SubscriptionRef.set(state, nextState)
            return
          }
          case "Fail": {
            const render = config.renderError ?? defaultError
            const t = yield* render({ url, error: run.error })
            yield* swap(run, t)
            const nextState: RouterState = {
              status: "idle",
              url,
              key,
              run,
            }
            yield* SubscriptionRef.set(state, nextState)
            return
          }
          case "Redirect": {
            if (redirectsLeft <= 0) {
              const render = config.renderError ?? defaultError
              const t = yield* render({
                url,
                error: new RouterError("Too many redirects"),
              })
              yield* swap(run, t)
              const nextState: RouterState = {
                status: "idle",
                url,
                key,
                run: {
                  _tag: "Fail",
                  routeId: run.routeId,
                  match: run.match,
                  hydration: run.hydration,
                  navigationSwap: run.navigationSwap,
                  status: 500,
                  error: new RouterError("Too many redirects"),
                  stage: "loader",
                } satisfies AnyRun,
              }
              yield* SubscriptionRef.set(state, nextState)
              return
            }

            const next = parseHref(run.href, url)
            if (!next) {
              return yield* Effect.fail(
                new RouterError(`Invalid redirect href: ${run.href}`)
              )
            }

            // Redirect is always history.replace (avoid stacking entries).
            config.history.replace(next)
            const nextState: RouterState = {
              status: "navigating",
              url: next,
              key: null,
              run,
            }
            yield* SubscriptionRef.set(state, nextState)

            const resolved = yield* resolveRun(next, "navigate")
            yield* applyOutcome(token, next, resolved.key, resolved.run, redirectsLeft - 1)

            // NOTE: If we rendered stale and spawned a refresh, redirects from the refresh are ignored in v1.
            if (resolved.refresh) {
              yield* forkRefresh(token, next, resolved.key, resolved.refresh)
            }

            return
          }
        }
      })

    const forkRefresh = (
      token: number,
      url: URL,
      key: LoaderKey | null,
      refresh: Effect.Effect<AnyRun, never, R>
    ) =>
      Effect.gen(function* () {
        if (!key) return

        const runtime = yield* Effect.runtime<DomService | R>()
        const runFork = Runtime.runFork(runtime)

        // Background refresh: apply only if still on the same key.
        runFork(
          refresh.pipe(
            Effect.flatMap((nextRun) =>
              Effect.gen(function* () {
                const current = yield* SubscriptionRef.get(state)
                if (current.key !== key) return
                if (nextRun._tag !== "Ok") return
                yield* applyOutcome(token, url, key, nextRun, 0)
              })
            ),
            Effect.catchAll((e) =>
              // Best-effort; SWR refresh failures should not crash.
              Effect.sync(() => console.warn("[Effuse/Router] refresh failed", e))
            )
          )
        )
      })

    const navigateUrl = (url: URL, historyMode: HistoryMode) =>
      Effect.gen(function* () {
        const token = yield* Ref.updateAndGet(navToken, (n) => n + 1)

        // Record the current loader key early so a new navigation can cancel in-flight work
        // deterministically (see §3.4.1 "Cancellation").
        const key = yield* computeLoaderKeyForUrl(url)
        yield* Ref.set(currentNavKey, key)

        if (historyMode === "push") {
          config.history.push(url)
        } else if (historyMode === "replace") {
          config.history.replace(url)
        }

        const nextState: RouterState = {
          status: "navigating",
          url,
          key,
          run: null,
        }
        yield* SubscriptionRef.set(state, nextState)

        const resolved = yield* resolveRun(url, "navigate")
        yield* applyOutcome(token, url, resolved.key, resolved.run, maxRedirects)
        if (resolved.refresh) {
          yield* forkRefresh(token, url, resolved.key, resolved.refresh)
        }
      })

    const startNavigation = (url: URL, historyMode: HistoryMode) =>
      Effect.gen(function* () {
        const previousState = yield* SubscriptionRef.get(state)
        const previousKey = previousState.key

        const fiber = yield* navigateUrl(url, historyMode).pipe(Effect.fork)
        const previous = yield* Ref.getAndSet(navFiber, fiber)

        // Cancel any previous in-flight loader fiber if navigation was its only consumer.
        // We intentionally do this BEFORE interrupting the previous navigation fiber so
        // the inflight refCount still reflects all current consumers (including prefetch).
        yield* cancelInflightIfSoleConsumer(previousKey)

        if (previous) {
          yield* Fiber.interrupt(previous)
        }
        yield* Fiber.join(fiber)
      })

    const navigate = (href: string, options?: NavigateOptions) =>
      Effect.gen(function* () {
        const base = config.history.current()
        const url = parseHref(href, base)
        if (!url) {
          return yield* Effect.fail(new RouterError(`Invalid href: ${href}`))
        }

        if (url.origin !== base.origin) {
          return yield* Effect.fail(
            new RouterError(`Refusing cross-origin navigation: ${url.href}`)
          )
        }

        yield* startNavigation(url, options?.replace ? "replace" : "push")
      })

    const prefetch = (href: string) =>
      Effect.gen(function* () {
        const base = config.history.current()
        const url = parseHref(href, base)
        if (!url) {
          return
        }
        if (url.origin !== base.origin) {
          return
        }

        yield* SubscriptionRef.update(state, (s): RouterState =>
          s.status === "idle" ? { ...s, status: "prefetching" as const } : s
        )

        yield* resolveRun(url, "prefetch")

        yield* SubscriptionRef.update(state, (s): RouterState =>
          s.status === "prefetching" ? { ...s, status: "idle" as const } : s
        )
      })

    const start: RouterService<R>["start"] = Effect.gen(function* () {
      const already = yield* Ref.get(started)
      if (already) return
      yield* Ref.set(started, true)

      const runtime = yield* Effect.runtime<DomService | R>()
      const runFork = Runtime.runFork(runtime)

      const onClick = (evt: Event) => {
        if (!(evt instanceof MouseEvent)) return
        if (evt.defaultPrevented) return
        if (evt.button !== 0) return
        if (evt.metaKey || evt.altKey || evt.ctrlKey || evt.shiftKey) return

        const target = evt.target
        if (!(target instanceof Element)) return

        const anchor = target.closest("a[href]")
        if (!(anchor instanceof HTMLAnchorElement)) return

        if (anchor.hasAttribute("data-router-ignore")) return
        const hrefAttr = anchor.getAttribute("href")
        if (!hrefAttr) return
        if (hrefAttr.startsWith("#")) return
        if (hrefAttr.startsWith("mailto:") || hrefAttr.startsWith("tel:")) return
        if (anchor.hasAttribute("download")) return
        if (anchor.target && anchor.target !== "_self") return

        const base = config.history.current()
        const url = parseHref(hrefAttr, base)
        if (!url) return
        if (url.origin !== base.origin) return

        evt.preventDefault()
        runFork(
          navigate(`${url.pathname}${url.search}${url.hash}`).pipe(
            Effect.catchAll((err) =>
              Effect.sync(() => console.error("[Effuse/Router] navigate failed", err))
            )
          )
        )
      }

      config.shell.addEventListener("click", onClick)

      const stopHistory = config.history.listen((url) => {
        runFork(
          startNavigation(url, "none").pipe(
            Effect.catchAll((err) =>
              Effect.sync(() => console.error("[Effuse/Router] popstate failed", err))
            )
          )
        )
      })

      // Scope-less cleanup: callers can rely on module lifetime in the browser.
      // Tests can call stopHistory() / removeEventListener via the returned cleanup
      // once we add explicit disposal APIs.
      void stopHistory
    })

    const service = { state, start, navigate, prefetch } satisfies RouterService<R>
    // Non-normative: allow tests/dev tooling to introspect inflight/cache state.
    return Object.assign(service, { __debug: debug })
  })
