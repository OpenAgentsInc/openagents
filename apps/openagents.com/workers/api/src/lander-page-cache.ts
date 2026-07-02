import { Effect } from 'effect'

import {
  type OpenAgentsWorkerExecutionContext,
  scheduleBackgroundWork,
} from './runtime'

// Edge cache for the server-rendered lander family. The family's remaining
// cost is the per-request live-at-read D1 ledger SUM (~100 ms median, 2 s
// outliers). This helper serves the rendered document from the Cloudflare
// Cache API for a short window (20 s) so the hit path skips the Worker render
// AND the D1 read entirely, while the browser itself never caches
// (`no-store` to the client): every navigation revalidates at the edge, and
// the inline counter refresher keeps the number live within 5 s regardless.
// Where the Cache API is unavailable (vitest, workers.dev previews), it
// degrades to a plain render.

const EDGE_TTL_SECONDS = 20

const clientHeaders = (cacheState: 'hit' | 'miss' | 'bypass'): HeadersInit => ({
  'cache-control': 'no-store',
  'content-type': 'text/html; charset=utf-8',
  'x-lander-edge-cache': cacheState,
})

const edgeCache = (): Cache | undefined => {
  const store = (globalThis as { caches?: { default?: Cache } }).caches
  return store?.default
}

export const edgeCachedLanderHtml = (
  request: Request,
  ctx: OpenAgentsWorkerExecutionContext | undefined,
  render: Effect.Effect<string, never, never>,
): Effect.Effect<Response, never, never> =>
  Effect.promise(async () => {
    const cache = edgeCache()
    // Key on the bare path: query strings must not fragment or poison the
    // cached document.
    const url = new URL(request.url)
    const key = new Request(`${url.origin}${url.pathname}`)
    if (cache !== undefined) {
      try {
        const hit = await cache.match(key)
        if (hit !== undefined) {
          const body = await hit.text()
          return new Response(body, { headers: clientHeaders('hit'), status: 200 })
        }
      } catch {
        // Cache read failures degrade to a render.
      }
    }
    return null
  }).pipe(
    Effect.flatMap(hit =>
      hit !== null
        ? Effect.succeed(hit)
        : render.pipe(
            Effect.map(html => {
              const cache = edgeCache()
              const url = new URL(request.url)
              const key = new Request(`${url.origin}${url.pathname}`)
              if (cache !== undefined) {
                const stored = new Response(html, {
                  headers: {
                    'cache-control': `s-maxage=${EDGE_TTL_SECONDS}`,
                    'content-type': 'text/html; charset=utf-8',
                  },
                })
                const put = cache.put(key, stored).catch(() => undefined)
                if (ctx !== undefined) scheduleBackgroundWork(ctx, put)
              }
              return new Response(html, {
                headers: clientHeaders(edgeCache() === undefined ? 'bypass' : 'miss'),
                status: 200,
              })
            }),
          ),
    ),
  )
