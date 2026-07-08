import { existsSync } from 'node:fs'
import { readFile, stat } from 'node:fs/promises'
import path from 'node:path'
import startWorker from '../dist/server/index.js'

const CLIENT_DIR = path.resolve(
  process.env.OPENAGENTS_START_CLIENT_DIR ?? './dist/client',
)
const PUBLIC_API_ORIGIN = (
  process.env.OPENAGENTS_PUBLIC_API_ORIGIN ?? 'https://openagents.com'
).replace(/\/$/, '')
const PORT = Number(process.env.PORT ?? 8080)

const CONTENT_TYPES = {
  '.css': 'text/css; charset=utf-8',
  '.html': 'text/html; charset=utf-8',
  '.ico': 'image/x-icon',
  '.jpeg': 'image/jpeg',
  '.jpg': 'image/jpeg',
  '.js': 'text/javascript; charset=utf-8',
  '.json': 'application/json',
  '.map': 'application/json',
  '.png': 'image/png',
  '.svg': 'image/svg+xml',
  '.txt': 'text/plain; charset=utf-8',
  '.webmanifest': 'application/manifest+json',
  '.webp': 'image/webp',
  '.woff': 'font/woff',
  '.woff2': 'font/woff2',
}

const contentTypeFor = (filePath) =>
  CONTENT_TYPES[path.extname(filePath).toLowerCase()] ??
  'application/octet-stream'

const resolveClientPath = (pathname) => {
  let decoded
  try {
    decoded = decodeURIComponent(pathname)
  } catch {
    return null
  }

  const normalized = path.normalize(decoded).replace(/^[/\\]+/, '')
  const resolved = path.resolve(CLIENT_DIR, normalized)
  if (resolved !== CLIENT_DIR && !resolved.startsWith(CLIENT_DIR + path.sep)) {
    return null
  }
  return resolved
}

const respondWithFile = async (filePath, method) => {
  let fileStat
  try {
    fileStat = await stat(filePath)
  } catch {
    return null
  }
  if (!fileStat.isFile()) return null

  const immutable = /\/assets\/[^/]+-[A-Za-z0-9_]{8,}\.[a-z0-9]+$/.test(
    filePath,
  )
  const headers = new Headers({
    'cache-control': immutable
      ? 'public, max-age=31536000, immutable'
      : 'public, max-age=60',
    'content-length': String(fileStat.size),
    'content-type': contentTypeFor(filePath),
  })

  return new Response(method === 'HEAD' ? null : await readFile(filePath), {
    headers,
    status: 200,
  })
}

const serveClientAsset = async (request, url) => {
  if (request.method !== 'GET' && request.method !== 'HEAD') return undefined
  const filePath = resolveClientPath(url.pathname)
  if (filePath === null) return new Response('not found', { status: 404 })
  return (await respondWithFile(filePath, request.method)) ?? undefined
}

const proxyPublicApi = async (request, url) => {
  if (!url.pathname.startsWith('/api/public/')) return undefined
  if (request.method !== 'GET' && request.method !== 'HEAD') {
    return new Response('method not allowed', {
      headers: { allow: 'GET, HEAD' },
      status: 405,
    })
  }

  const upstream = new URL(`${url.pathname}${url.search}`, PUBLIC_API_ORIGIN)
  const headers = new Headers()
  const accept = request.headers.get('accept')
  if (accept !== null) headers.set('accept', accept)

  const response = await fetch(upstream, {
    headers,
    method: request.method,
  })
  const nextHeaders = new Headers(response.headers)
  nextHeaders.set('cache-control', 'no-store')
  return new Response(request.method === 'HEAD' ? null : response.body, {
    headers: nextHeaders,
    status: response.status,
    statusText: response.statusText,
  })
}

const executionCtx = {
  waitUntil(promise) {
    void Promise.resolve(promise).catch((error) => {
      console.error(
        JSON.stringify({
          at: new Date().toISOString(),
          error: error instanceof Error ? error.message : String(error),
          event: 'start_cloudrun.wait_until_failed',
        }),
      )
    })
  },
}

if (!existsSync(CLIENT_DIR)) {
  throw new Error(`OPENAGENTS_START_CLIENT_DIR missing: ${CLIENT_DIR}`)
}

Bun.serve({
  fetch: async (request) => {
    const url = new URL(request.url)

    if (url.pathname === '/internal/healthz') {
      return Response.json({ ok: true, service: 'openagents-com-start' })
    }

    const publicApi = await proxyPublicApi(request, url)
    if (publicApi !== undefined) return publicApi

    const clientAsset = await serveClientAsset(request, url)
    if (clientAsset !== undefined) return clientAsset

    return startWorker.fetch(
      request,
      {
        KHALA_SYNC_UPSTREAM_BASE_URL:
          process.env.KHALA_SYNC_UPSTREAM_BASE_URL ?? PUBLIC_API_ORIGIN,
      },
      executionCtx,
    )
  },
  hostname: '0.0.0.0',
  port: PORT,
})

console.log(
  JSON.stringify({
    at: new Date().toISOString(),
    clientDir: CLIENT_DIR,
    event: 'start_cloudrun.listening',
    port: PORT,
    publicApiOrigin: PUBLIC_API_ORIGIN,
  }),
)
