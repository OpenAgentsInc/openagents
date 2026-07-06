import { readFile } from 'node:fs/promises'
import path from 'node:path'

/**
 * Static asset serving for the Cloud Run Bun server (CFG-11, #8526).
 * Serves the Vite client build (`dist/client`). Implemented on
 * `node:fs/promises` (not `Bun.file`) so the logic is unit-testable under
 * vitest's node environment while running identically under Bun.
 */

const CONTENT_TYPES: Readonly<Record<string, string>> = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.gif': 'image/gif',
  '.webp': 'image/webp',
  '.avif': 'image/avif',
  '.ico': 'image/x-icon',
  '.txt': 'text/plain; charset=utf-8',
  '.xml': 'application/xml; charset=utf-8',
  '.map': 'application/json; charset=utf-8',
  '.woff': 'font/woff',
  '.woff2': 'font/woff2',
  '.ttf': 'font/ttf',
  '.webmanifest': 'application/manifest+json; charset=utf-8',
  '.wasm': 'application/wasm',
}

export const contentTypeForPath = (filePath: string): string =>
  CONTENT_TYPES[path.extname(filePath).toLowerCase()] ??
  'application/octet-stream'

/**
 * Resolves a URL pathname to an absolute file path inside `clientDir`, or
 * `undefined` when the path is not a plausible static file (directory-ish,
 * traversal attempt, or undecodable). Never escapes `clientDir`.
 */
export const resolveStaticFilePath = (
  pathname: string,
  clientDir: string,
): string | undefined => {
  let decoded: string
  try {
    decoded = decodeURIComponent(pathname)
  } catch {
    return undefined
  }

  if (decoded.includes('\0') || !decoded.startsWith('/') || decoded.endsWith('/')) {
    return undefined
  }

  const root = path.resolve(clientDir)
  const resolved = path.resolve(root, `.${path.posix.normalize(decoded)}`)

  if (resolved !== root && !resolved.startsWith(`${root}${path.sep}`)) {
    return undefined
  }

  return resolved
}

/** Hashed Vite build output is immutable; everything else stays revalidated. */
export const cacheControlForPathname = (pathname: string): string =>
  pathname.startsWith('/assets/')
    ? 'public, max-age=31536000, immutable'
    : 'public, max-age=300'

export type StaticFileReader = (filePath: string) => Promise<Uint8Array>

/**
 * Serves one static file from the client build, or returns `undefined` to
 * fall through (to the SPA shell / API 404). Only GET and HEAD are served.
 */
export const staticAssetResponse = async (
  request: Request,
  clientDir: string,
  readFileImpl: StaticFileReader = filePath => readFile(filePath),
): Promise<Response | undefined> => {
  if (request.method !== 'GET' && request.method !== 'HEAD') {
    return undefined
  }

  const pathname = new URL(request.url).pathname
  const filePath = resolveStaticFilePath(pathname, clientDir)
  if (filePath === undefined) {
    return undefined
  }

  let body: Uint8Array
  try {
    body = await readFileImpl(filePath)
  } catch {
    // Missing file (or a directory read error): fall through.
    return undefined
  }

  const headers = new Headers({
    'content-type': contentTypeForPath(filePath),
    'cache-control': cacheControlForPathname(pathname),
    'content-length': String(body.byteLength),
  })

  return new Response(request.method === 'HEAD' ? null : (body as BodyInit), {
    status: 200,
    headers,
  })
}
