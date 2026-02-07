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

export const tryServeAsset = async (
  request: Request,
  env: WorkerEnv,
): Promise<Response | null> => {
  const url = new URL(request.url)
  if (request.method !== "GET" && request.method !== "HEAD") return null
  if (!isLikelyAssetPath(url.pathname)) return null
  if (!env.ASSETS) return null

  const response = await env.ASSETS.fetch(request)
  if (response.status === 404) return null
  return response
}

