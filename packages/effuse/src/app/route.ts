import type { Effect } from "effect"
import type { TemplateResult } from "../template/types.js"

export type RouteId = string

export type RouteMatch = {
  readonly pathname: string
  readonly params: Readonly<Record<string, string>>
  readonly search: URLSearchParams
}

export type RouteHead = {
  readonly title?: string
  readonly meta?: ReadonlyArray<readonly [name: string, content: string]>
}

export type RouteContext =
  | {
      readonly _tag: "Server"
      readonly url: URL
      readonly match: RouteMatch
      readonly request: Request
    }
  | { readonly _tag: "Client"; readonly url: URL; readonly match: RouteMatch }

export type CachePolicy =
  | { readonly mode: "no-store" }
  | { readonly mode: "cache-first"; readonly ttlMs?: number }
  | {
      readonly mode: "stale-while-revalidate"
      readonly ttlMs: number
      readonly swrMs: number
    }

export type CookieMutation =
  | {
      readonly _tag: "Set"
      readonly name: string
      readonly value: string
      readonly attributes?: string
    }
  | { readonly _tag: "Delete"; readonly name: string; readonly attributes?: string }

// Merge rules:
// - `dehydrate` is stored under a routeId namespace (no deep merge).
// - `receipts` is mergeable as append-only arrays or stable-id maps (no last-write-wins).
export type DehydrateFragment = unknown
export type ReceiptsFragment = unknown

export type RouteOkHints = {
  readonly cache?: CachePolicy
  readonly headers?: ReadonlyArray<readonly [string, string]>
  readonly cookies?: ReadonlyArray<CookieMutation> // server only
  readonly dehydrate?: DehydrateFragment
  readonly receipts?: ReceiptsFragment
}

export type RedirectStatus = 301 | 302 | 303 | 307 | 308

export type RouteOutcome<A> =
  | { readonly _tag: "Ok"; readonly data: A; readonly hints?: RouteOkHints }
  | {
      readonly _tag: "Redirect"
      readonly href: string
      readonly status?: RedirectStatus
    }
  | { readonly _tag: "NotFound" }
  | { readonly _tag: "Fail"; readonly status?: number; readonly error: unknown }

export const RouteOutcome = {
  ok: <A>(data: A, hints?: RouteOkHints): RouteOutcome<A> => ({
    _tag: "Ok",
    data,
    ...(hints ? { hints } : {}),
  }),
  redirect: (href: string, status?: RedirectStatus): RouteOutcome<never> => ({
    _tag: "Redirect",
    href,
    ...(status ? { status } : {}),
  }),
  notFound: (): RouteOutcome<never> => ({ _tag: "NotFound" }),
  fail: (error: unknown, status?: number): RouteOutcome<never> => ({
    _tag: "Fail",
    error,
    ...(status ? { status } : {}),
  }),
} as const

export type HydrationMode = "strict" | "soft" | "client-only"
export type NavigationSwapMode = "outlet" | "document"

export type Route<LoaderData, R = never> = {
  readonly id: RouteId

  // Path matching + param parsing (must be deterministic and shared).
  readonly match: (url: URL) => RouteMatch | null

  // Guards are optional sugar but MUST be standardized to avoid ad hoc redirects in loaders.
  // If a guard returns an outcome, the route run short-circuits (redirect/not-found/fail).
  readonly guard?: (
    ctx: RouteContext
  ) => Effect.Effect<RouteOutcome<never> | void, never, R>

  // Loaders return RouteOutcome so redirect/not-found/cache/dehydrate is standardized.
  readonly loader: (ctx: RouteContext) => Effect.Effect<RouteOutcome<LoaderData>, never, R>

  // Views are pure w.r.t. loader data (side effects belong in loader/subscriptions).
  readonly view: (
    ctx: RouteContext,
    data: LoaderData
  ) => Effect.Effect<TemplateResult, never, R>

  readonly head?: (
    ctx: RouteContext,
    data: LoaderData
  ) => Effect.Effect<RouteHead, never, R>

  // Defaults: hydration="strict", navigation.swap="outlet"
  readonly hydration?: HydrationMode
  readonly navigation?: { readonly swap?: NavigationSwapMode }
}
