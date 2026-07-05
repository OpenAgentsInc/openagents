// Best-effort per-IP window rate limiting for the Khala Sync anonymous-read
// exception (KS-8.x, docs/khala-sync/RUNBOOK.md "Anonymous read scopes"):
// `scope.public.*` reads on GET/WS /api/sync/connect, GET /api/sync/log, and
// POST /api/sync/bootstrap that proceed WITHOUT an authenticated actor.
//
// SCOPE OF THIS LIMITER: it applies ONLY to requests that are actually
// anonymous (no session cookie, no agent bearer token). Authenticated
// requests — including authenticated reads of `scope.public.*`, which were
// already unrestricted for ANY signed-in user before this change — are
// NEVER subject to it and see zero behavior change. This limiter exists
// purely to bound the NEW surface this change opens: a caller with no
// account or token at all can now reach these routes.
//
// SAME SHAPE AS THE REPO'S OTHER PUBLIC-SURFACE LIMITERS (deliberately, not
// reinvented): a fixed-window per-`CF-Connecting-IP` counter, best-effort
// and per-isolate — see `business-intake-chat-routes.ts` ("per-IP window
// rate limiting (best-effort, per isolate)") and the sibling public
// `/api/khala/chat` limiter. This is NOT a durable, cross-isolate, or
// cross-colo limiter: a distributed attacker is not fully bounded by it.
// It raises the floor above "completely unbounded," matching this repo's
// established, reviewed tradeoff for anonymous public surfaces, rather than
// introducing new infrastructure (a Cloudflare Rate Limiting binding, a
// dedicated Durable Object, or KV-backed counters) for this change.
//
// Read routes (log, bootstrap) get a more generous window than the
// WebSocket connect route: a connect upgrade holds a live per-scope
// KhalaSyncHubDO socket for the connection's lifetime, so an admitted
// connect request is heavier than an admitted log/bootstrap read.

import { currentEpochMillis } from './runtime-primitives'

const RATE_MAX_TRACKED_IPS = 10_000

type WindowCounter = { count: number; windowStartedAt: number }

/**
 * Admits when the tracked count for `key` is under `limit` inside the
 * current `windowMs` window (from `windowStartedAt`); resets to a fresh
 * window on expiry. Counter maps are capped at `RATE_MAX_TRACKED_IPS` so an
 * isolate under wide-IP-range abuse cannot grow memory without bound —
 * clearing only ever ADMITS more traffic, never denies more, so this bound
 * cannot itself become a false-positive denial source.
 */
const admitWindow = (
  counters: Map<string, WindowCounter>,
  key: string,
  limit: number,
  windowMs: number,
  now: number,
): boolean => {
  if (counters.size > RATE_MAX_TRACKED_IPS) {
    counters.clear()
  }
  const existing = counters.get(key)
  if (existing === undefined || now - existing.windowStartedAt >= windowMs) {
    counters.set(key, { count: 1, windowStartedAt: now })
    return true
  }
  if (existing.count >= limit) {
    return false
  }
  existing.count = existing.count + 1
  return true
}

const clientIp = (request: Request): string =>
  request.headers.get('cf-connecting-ip') ??
  request.headers.get('x-forwarded-for')?.split(',')[0]?.trim() ??
  'unknown'

export type KhalaSyncAnonymousRateLimiter = (request: Request) => boolean

const makeWindowedIpRateLimiter = (
  input: Readonly<{
    minuteLimit: number
    dayLimit: number
    now?: (() => number) | undefined
  }>,
): KhalaSyncAnonymousRateLimiter => {
  const minuteCounters = new Map<string, WindowCounter>()
  const dayCounters = new Map<string, WindowCounter>()
  const now = input.now ?? currentEpochMillis
  return request => {
    const ip = clientIp(request)
    const at = now()
    const minuteOk = admitWindow(
      minuteCounters,
      ip,
      input.minuteLimit,
      60_000,
      at,
    )
    const dayOk = admitWindow(dayCounters, ip, input.dayLimit, 86_400_000, at)
    return minuteOk && dayOk
  }
}

/**
 * GET /api/sync/log and POST /api/sync/bootstrap, anonymous requests only
 * (`scope.public.*`, no authenticated actor): 120/minute, 20,000/day per IP.
 */
export const makeKhalaSyncAnonymousReadRateLimiter = (
  now?: (() => number) | undefined,
): KhalaSyncAnonymousRateLimiter =>
  makeWindowedIpRateLimiter({ dayLimit: 20_000, minuteLimit: 120, now })

/**
 * GET/WS /api/sync/connect, anonymous requests only (`scope.public.*`, no
 * authenticated actor): 20/minute, 2,000/day per IP — tighter than the read
 * limiter because an admitted connect holds a live hub Durable Object
 * socket for the connection's lifetime.
 */
export const makeKhalaSyncAnonymousConnectRateLimiter = (
  now?: (() => number) | undefined,
): KhalaSyncAnonymousRateLimiter =>
  makeWindowedIpRateLimiter({ dayLimit: 2_000, minuteLimit: 20, now })
