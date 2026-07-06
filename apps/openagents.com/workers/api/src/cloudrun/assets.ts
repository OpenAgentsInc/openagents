/**
 * CFG-9 (#8524): `env.ASSETS` Fetcher for the Cloud Run monolith.
 *
 * Replaces the Workers static-assets binding (wrangler `assets` with
 * `not_found_handling: "single-page-application"`): serve files from the
 * built `apps/web/dist` directory, and fall back to `index.html` for any
 * path without a matching file so client-side routing keeps working.
 *
 * The worker code calls `env.ASSETS.fetch(request)` as its final fallback
 * (index.ts `handle_asset_request`) and for a handful of companion files
 * (HEARTBEAT.md, RULES.md, skill.json, onboarding shell).
 *
 * Uses node:fs (not `Bun.file`) so the unit suite runs under vitest/node.
 */

import { existsSync } from 'node:fs'
import { readFile, stat } from 'node:fs/promises'
import path from 'node:path'

const IMMUTABLE_ASSET_PATTERN = /\/assets\/[^/]+-[A-Za-z0-9_]{8,}\.[a-z0-9]+$/

const CONTENT_TYPES: Readonly<Record<string, string>> = {
  '.css': 'text/css; charset=utf-8',
  '.html': 'text/html; charset=utf-8',
  '.ico': 'image/x-icon',
  '.jpeg': 'image/jpeg',
  '.jpg': 'image/jpeg',
  '.js': 'text/javascript; charset=utf-8',
  '.json': 'application/json',
  '.map': 'application/json',
  '.md': 'text/markdown; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.png': 'image/png',
  '.svg': 'image/svg+xml',
  '.ttf': 'font/ttf',
  '.txt': 'text/plain; charset=utf-8',
  '.wasm': 'application/wasm',
  '.webmanifest': 'application/manifest+json',
  '.webp': 'image/webp',
  '.woff': 'font/woff',
  '.woff2': 'font/woff2',
  '.xml': 'application/xml',
}

const contentTypeFor = (filePath: string): string =>
  CONTENT_TYPES[path.extname(filePath).toLowerCase()] ??
  'application/octet-stream'

/**
 * Resolve a URL pathname to a file inside `distDir`, refusing any resolution
 * that escapes the directory (`..`, encoded traversal, absolute paths).
 */
export const resolveAssetPath = (
  distDir: string,
  pathname: string,
): string | null => {
  let decoded: string
  try {
    decoded = decodeURIComponent(pathname)
  } catch {
    return null
  }

  const normalized = path.normalize(decoded).replace(/^([/\\])+/, '')
  const resolved = path.resolve(distDir, normalized)

  if (resolved !== distDir && !resolved.startsWith(distDir + path.sep)) {
    return null
  }

  return resolved
}

export type AssetsFetcher = Readonly<{
  fetch: (input: Request | string | URL) => Promise<Response>
}>

export const makeAssetsFetcher = (distDirInput: string): AssetsFetcher => {
  const distDir = path.resolve(distDirInput)
  const indexPath = path.join(distDir, 'index.html')

  const respondWithFile = async (
    filePath: string,
    method: string,
    cacheControl: string,
  ): Promise<Response | null> => {
    let size: number
    try {
      const stats = await stat(filePath)
      if (!stats.isFile()) return null
      size = stats.size
    } catch {
      return null
    }

    const headers = new Headers({
      'cache-control': cacheControl,
      'content-type': contentTypeFor(filePath),
    })

    if (method === 'HEAD') {
      headers.set('content-length', String(size))
      return new Response(null, { headers, status: 200 })
    }

    return new Response(await readFile(filePath), { headers, status: 200 })
  }

  return {
    fetch: async (input: Request | string | URL): Promise<Response> => {
      const request =
        input instanceof Request ? input : new Request(String(input))
      const method = request.method.toUpperCase()

      if (method !== 'GET' && method !== 'HEAD') {
        return new Response('method not allowed', { status: 405 })
      }

      const url = new URL(request.url)
      let pathname = url.pathname
      if (pathname.endsWith('/')) pathname = `${pathname}index.html`

      const candidate = resolveAssetPath(distDir, pathname)
      if (candidate !== null) {
        const cacheControl = IMMUTABLE_ASSET_PATTERN.test(pathname)
          ? 'public, max-age=31536000, immutable'
          : 'public, max-age=60'
        const exact = await respondWithFile(candidate, method, cacheControl)
        if (exact !== null) return exact

        // `/route` → `/route.html` (wrangler assets html_handling default).
        if (!path.extname(candidate)) {
          const html = await respondWithFile(
            `${candidate}.html`,
            method,
            'public, max-age=60',
          )
          if (html !== null) return html
        }
      }

      // single-page-application fallback: any unknown path serves the shell.
      const shell = await respondWithFile(indexPath, method, 'no-cache')
      if (shell !== null) return shell

      return new Response('assets directory missing index.html', {
        status: 404,
      })
    },
  }
}

export const assertAssetsDirExists = (distDir: string): void => {
  if (!existsSync(path.join(distDir, 'index.html'))) {
    throw new Error(
      `CFG-9: web assets not found at ${distDir} (expected apps/web/dist with index.html). Run \`bun run build:web\` first.`,
    )
  }
}
