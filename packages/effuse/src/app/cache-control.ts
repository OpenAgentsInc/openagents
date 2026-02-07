import type { CachePolicy } from "./route.js"
import type { RouteRun } from "./run.js"

const toSeconds = (ms: number): number =>
  Number.isFinite(ms) ? Math.max(0, Math.floor(ms / 1000)) : 0

/**
 * Convert an Effuse `CachePolicy` into HTTP `Cache-Control` directives.
 *
 * Notes:
 * - This intentionally returns directives only (e.g. `max-age=60`) rather than
 *   choosing a scope (`private` vs `public`). Hosts should pick a safe default.
 * - `cache-first` with `ttlMs` omitted is an in-memory router optimization and
 *   does not map safely to a durable HTTP cache; we return `null` so callers can
 *   fall back to `no-store`.
 */
export const cachePolicyToCacheControlDirectives = (
  policy: CachePolicy
): string | null => {
  switch (policy.mode) {
    case "no-store":
      return "no-store"
    case "cache-first": {
      if (policy.ttlMs == null) return null
      return `max-age=${toSeconds(policy.ttlMs)}`
    }
    case "stale-while-revalidate":
      return `max-age=${toSeconds(policy.ttlMs)}, stale-while-revalidate=${toSeconds(
        policy.swrMs
      )}`
  }
}

/**
 * Conservative default `Cache-Control` header value for SSR HTML responses derived from a `RouteRun`.
 *
 * Rules:
 * - Never cache non-Ok outcomes.
 * - Never cache when cookie mutations are present (implies `Set-Cookie`).
 * - Default to `no-store` unless a cache policy maps to durable HTTP directives.
 * - Use `private` scope for cacheable HTML.
 *
 * Note: Hosts may still choose to override this when a route explicitly sets `Cache-Control`.
 */
export const cacheControlForRouteRun = (run: RouteRun<unknown>): string => {
  if (run._tag !== "Ok") return "no-store"

  const hints = run.hints
  if (!hints?.cache) return "no-store"
  if (hints.cookies && hints.cookies.length > 0) return "no-store"

  const directives = cachePolicyToCacheControlDirectives(hints.cache)
  if (!directives || directives === "no-store") return "no-store"
  return `private, ${directives}`
}
