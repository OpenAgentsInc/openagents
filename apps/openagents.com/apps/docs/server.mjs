import { createReadStream, existsSync, statSync } from 'node:fs'
import { createServer } from 'node:http'
import { extname, join, resolve, sep } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const docsPrefix = '/docs'
const defaultRoot = fileURLToPath(new URL('./dist', import.meta.url))

const contentTypes = new Map([
  ['.css', 'text/css; charset=utf-8'],
  ['.gif', 'image/gif'],
  ['.html', 'text/html; charset=utf-8'],
  ['.ico', 'image/x-icon'],
  ['.jpeg', 'image/jpeg'],
  ['.jpg', 'image/jpeg'],
  ['.js', 'text/javascript; charset=utf-8'],
  ['.json', 'application/json; charset=utf-8'],
  ['.md', 'text/markdown; charset=utf-8'],
  ['.mdx', 'text/markdown; charset=utf-8'],
  ['.png', 'image/png'],
  ['.svg', 'image/svg+xml'],
  ['.txt', 'text/plain; charset=utf-8'],
  ['.webp', 'image/webp'],
  ['.woff2', 'font/woff2'],
  ['.xml', 'application/xml; charset=utf-8'],
])

const securityHeaders = {
  'Content-Security-Policy': "default-src 'self'; base-uri 'none'; connect-src 'self'; font-src 'self'; form-action 'none'; frame-ancestors 'none'; img-src 'self' data:; object-src 'none'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'",
  'Cross-Origin-Opener-Policy': 'same-origin',
  'Permissions-Policy': 'camera=(), geolocation=(), microphone=(), payment=(), usb=()',
  'Referrer-Policy': 'strict-origin-when-cross-origin',
  'X-Content-Type-Options': 'nosniff',
  'X-Frame-Options': 'DENY',
}

const cacheControl = (pathname) =>
  pathname.startsWith('/docs/_astro/')
    ? 'public, max-age=31536000, immutable'
    : pathname.endsWith('.html') || pathname === '/docs' || pathname === '/docs/'
      ? 'public, max-age=0, must-revalidate'
      : 'public, max-age=300'

const containedPath = (root, relativePath) => {
  const candidate = resolve(root, relativePath)
  return candidate === root || candidate.startsWith(`${root}${sep}`)
    ? candidate
    : undefined
}

const existingFile = (path) =>
  path !== undefined && existsSync(path) && statSync(path).isFile() ? path : undefined

export const resolveDocsFile = (root, pathname) => {
  if (pathname !== docsPrefix && !pathname.startsWith(`${docsPrefix}/`)) {
    return undefined
  }

  let decoded
  try {
    decoded = decodeURIComponent(pathname)
  } catch {
    return undefined
  }

  if (decoded.includes('\0') || decoded.split('/').includes('..')) {
    return undefined
  }

  const relative = decoded.slice(docsPrefix.length).replace(/^\/+/, '')
  const exact = existingFile(containedPath(root, relative))
  if (exact !== undefined) {
    return exact
  }

  return existingFile(containedPath(root, join(relative, 'index.html')))
}

const sendFile = (response, requestMethod, filePath, pathname, statusCode = 200) => {
  const headers = {
    ...securityHeaders,
    'Cache-Control': cacheControl(pathname),
    'Content-Type': contentTypes.get(extname(filePath).toLowerCase()) ?? 'application/octet-stream',
  }

  response.writeHead(statusCode, headers)
  if (requestMethod === 'HEAD') {
    response.end()
    return
  }
  createReadStream(filePath).pipe(response)
}

export const makeRequestHandler = (root = defaultRoot) => (request, response) => {
  if (request.method !== 'GET' && request.method !== 'HEAD') {
    response.writeHead(405, { ...securityHeaders, Allow: 'GET, HEAD' })
    response.end()
    return
  }

  const url = new URL(request.url ?? '/', 'http://openagents-docs.local')
  if (url.pathname === '/internal/healthz') {
    response.writeHead(200, { ...securityHeaders, 'Content-Type': 'application/json; charset=utf-8' })
    response.end(request.method === 'HEAD' ? undefined : JSON.stringify({ ok: true, service: 'openagents-docs' }))
    return
  }

  const filePath = resolveDocsFile(root, url.pathname)
  if (filePath !== undefined) {
    sendFile(response, request.method, filePath, url.pathname)
    return
  }

  const notFound = existingFile(join(root, '404.html'))
  if (notFound !== undefined && url.pathname.startsWith(`${docsPrefix}/`)) {
    sendFile(response, request.method, notFound, url.pathname, 404)
    return
  }

  response.writeHead(404, { ...securityHeaders, 'Content-Type': 'application/json; charset=utf-8' })
  response.end(request.method === 'HEAD' ? undefined : JSON.stringify({ error: 'not_found' }))
}

const isEntrypoint = process.argv[1] !== undefined && pathToFileURL(process.argv[1]).href === import.meta.url

if (isEntrypoint) {
  const port = Number.parseInt(process.env.PORT ?? '8080', 10)
  createServer(makeRequestHandler()).listen(port, '0.0.0.0', () => {
    console.log(JSON.stringify({ event: 'openagents_docs.ready', port }))
  })
}
