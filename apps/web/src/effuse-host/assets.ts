import type { WorkerEnv } from "./env"

const isLikelyAssetPath = (pathname: string): boolean => {
  if (pathname === "/favicon.ico") return true
  if (pathname === "/robots.txt") return true
  if (pathname === "/sitemap.xml") return true
  if (pathname.startsWith("/assets/")) return true
  // Vite dev/build often uses dotted filenames for chunks.
  if (pathname.includes(".") && !pathname.endsWith("/")) return true
  return false
}

const NO_CACHE_HEADERS = {
  "Cache-Control": "no-store, no-cache, must-revalidate",
  Pragma: "no-cache",
  Expires: "0",
} as const

/**
 * Serve deck JSON with no-cache headers so editors always see the latest
 * file after refresh (avoids browser and any intermediary caching).
 */
export const tryServeDeckAsset = async (
  request: Request,
  env: WorkerEnv,
): Promise<Response | null> => {
  const url = new URL(request.url)
  if (request.method !== "GET" && request.method !== "HEAD") return null
  if (!url.pathname.startsWith("/decks/") || !url.pathname.endsWith(".json")) return null
  if (!env.ASSETS) return null

  const res = await env.ASSETS.fetch(request)
  if (!res.ok) return res

  const body = res.body
  const headers = new Headers(res.headers)
  for (const [k, v] of Object.entries(NO_CACHE_HEADERS)) headers.set(k, v)
  return new Response(body, { status: res.status, statusText: res.statusText, headers })
}

export const tryServeAsset = async (
  request: Request,
  env: WorkerEnv,
): Promise<Response | null> => {
  const url = new URL(request.url)
  if (request.method !== "GET" && request.method !== "HEAD") return null
  if (!isLikelyAssetPath(url.pathname)) return null
  if (!env.ASSETS) return null

  // Never fall through to SSR for "asset-like" paths. If the asset is missing,
  // return the asset 404 rather than attempting to SSR a fake route.
  return env.ASSETS.fetch(request)
}
