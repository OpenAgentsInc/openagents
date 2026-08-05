/** Retained Start document adapter for the Cloud Run Worker monolith. */
import { existsSync } from 'node:fs'
import { readFile, stat } from 'node:fs/promises'
import path from 'node:path'
import { pathToFileURL } from 'node:url'

import { isKnownStartDocumentPath } from '../../../../apps/start/src/route-table'

const START_CLIENT_DIR = path.resolve(
  process.env['OPENAGENTS_START_CLIENT_DIR'] ??
    path.resolve(
      import.meta.dirname,
      '..',
      '..',
      '..',
      '..',
      'apps/start/dist/client',
    ),
)
const START_SERVER_ENTRY = path.resolve(
  process.env['OPENAGENTS_START_SERVER_ENTRY'] ??
    path.resolve(
      import.meta.dirname,
      '..',
      '..',
      '..',
      '..',
      'apps/start/dist/server/server.js',
    ),
)
const REQUIRED_DOCS_CLIENT_ARTIFACTS = [
  'docs/index.md',
  'docs/search.json',
  'docs/llms.txt',
  'docs/llms-full.txt',
  'docs/agent-readability.json',
  'docs/sitemap.xml',
] as const

// Server-owned Start endpoints must cross the same Cloud Run adapter as Start
// documents. Keep this list exact: the Worker remains authoritative for every
// other /api path.
const START_SERVER_REQUEST_PATHS = new Set([
  '/api/public/qa-board',
  // DIST-10 (#8923): Desktop download resolver + verified artifact redirect.
  '/api/public/desktop-download',
  '/api/public/desktop-download/artifact',
  // #9280: Omega download resolver + verified artifact redirect — a SEPARATE
  // signed product feed beside the Desktop one, never a relabeling of it.
  '/api/public/omega-download',
  '/api/public/omega-download/artifact',
])
const FORGE_REPOSITORY_ASSET_REQUEST_PATH =
  /^\/internal\/v1\/repositories\/[^/]+\/[^/]+\/web-read-asset\/.+$/u
const START_SERVER_FUNCTION_REQUEST_PATH = /^\/_serverFn\/[^/]+$/u

type StartWorker = Readonly<{
  fetch: (
    request: Request,
    env: Readonly<Record<string, unknown>>,
    ctx: ExecutionContext,
  ) => Promise<Response>
}>

let startWorkerPromise: Promise<StartWorker> | undefined
const loadStartWorker = (): Promise<StartWorker> => {
  startWorkerPromise ??= import(pathToFileURL(START_SERVER_ENTRY).href).then(
    module => {
      const candidate = module.default as StartWorker | undefined
      if (candidate === undefined || typeof candidate.fetch !== 'function') {
        throw new Error(
          `Start server has no default fetch handler: ${START_SERVER_ENTRY}`,
        )
      }
      return candidate
    },
  )
  return startWorkerPromise
}

export const startUiContentType = (filePath: string): string => {
  switch (path.extname(filePath).toLowerCase()) {
    case '.css':
      return 'text/css; charset=utf-8'
    case '.html':
      return 'text/html; charset=utf-8'
    case '.js':
    case '.mjs':
      return 'text/javascript; charset=utf-8'
    case '.json':
      return 'application/json'
    case '.md':
      return 'text/markdown; charset=utf-8'
    case '.png':
      return 'image/png'
    case '.svg':
      return 'image/svg+xml'
    case '.txt':
      return 'text/plain; charset=utf-8'
    case '.wasm':
      return 'application/wasm'
    case '.webp':
      return 'image/webp'
    case '.woff':
      return 'font/woff'
    case '.woff2':
      return 'font/woff2'
    case '.xml':
      return 'application/xml; charset=utf-8'
    default:
      return 'application/octet-stream'
  }
}

export const isDiamondHandsPath = (pathname: string): boolean =>
  pathname === '/dh' || pathname === '/dh/' || pathname.startsWith('/dh/')

export const isMarketDemoPath = (pathname: string): boolean =>
  pathname === '/demo' || pathname === '/demo/' || pathname.startsWith('/demo/')

export const marketDemoDeploymentEnabled = (
  configured = process.env['OPENAGENTS_MARKET_DEMO_ENABLED'],
): boolean => configured === 'true'

export const marketDemoResponseHeaders = (
  pathname: string,
): Readonly<Record<string, string>> =>
  isMarketDemoPath(pathname)
    ? {
        'content-security-policy': [
          "default-src 'none'",
          "base-uri 'none'",
          // 'self' is required: the wasm-bindgen loader fetches the .wasm
          // module over fetch(), which CSP classifies as a connection.
          "connect-src 'self' https://relay.openagents.com wss://relay.openagents.com",
          "font-src 'self' data:",
          "frame-ancestors 'none'",
          "img-src 'self' data:",
          // 'unsafe-eval' is required beside 'wasm-unsafe-eval': wasm_thread
          // bootstraps its workers by evaluating a JS string during init.
          // The document stays isolated (default-src 'none', no framing,
          // connections limited to self plus the public relay).
          "script-src 'self' 'unsafe-inline' 'unsafe-eval' 'wasm-unsafe-eval'",
          "style-src 'unsafe-inline'",
          "worker-src 'self' blob:",
        ].join('; '),
        'cross-origin-embedder-policy': 'require-corp',
        'cross-origin-opener-policy': 'same-origin',
        'cross-origin-resource-policy': 'same-origin',
      }
    : {}

export const isInfraExplainerPath = (pathname: string): boolean =>
  pathname === '/infra' ||
  pathname === '/infra/' ||
  pathname.startsWith('/infra/')

export const infraExplainerDeploymentEnabled = (
  configured = process.env['OPENAGENTS_INFRA_EXPLAINER_ENABLED'],
): boolean => configured === 'true'

export const infraExplainerResponseHeaders = (
  pathname: string,
): Readonly<Record<string, string>> =>
  isInfraExplainerPath(pathname)
    ? {
        'content-security-policy': [
          "default-src 'none'",
          "base-uri 'none'",
          // 'self' is required: the wasm-bindgen loader fetches the .wasm
          // module over fetch(), which CSP classifies as a connection.
          "connect-src 'self' https://relay.openagents.com wss://relay.openagents.com",
          "font-src 'self' data:",
          "frame-ancestors 'none'",
          "img-src 'self' data:",
          // 'unsafe-eval' is required beside 'wasm-unsafe-eval': wasm_thread
          // bootstraps its workers by evaluating a JS string during init.
          // The document stays isolated (default-src 'none', no framing,
          // connections limited to self plus the public relay).
          "script-src 'self' 'unsafe-inline' 'unsafe-eval' 'wasm-unsafe-eval'",
          "style-src 'unsafe-inline'",
          "worker-src 'self' blob:",
        ].join('; '),
        'cross-origin-embedder-policy': 'require-corp',
        'cross-origin-opener-policy': 'same-origin',
        'cross-origin-resource-policy': 'same-origin',
      }
    : {}

export const diamondHandsDeploymentEnabled = (
  configured = process.env['OPENAGENTS_DIAMOND_HANDS_ENABLED'],
): boolean => configured === 'true'

export const diamondHandsResponseHeaders = (
  pathname: string,
): Readonly<Record<string, string>> =>
  isDiamondHandsPath(pathname)
    ? {
        'content-security-policy': [
          "default-src 'none'",
          "base-uri 'none'",
          'connect-src https://relay.openagents.com wss://relay.openagents.com',
          "font-src 'self' data:",
          "frame-ancestors 'none'",
          "img-src 'self' data:",
          "script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'",
          "style-src 'unsafe-inline'",
          "worker-src 'self' blob:",
        ].join('; '),
        'cross-origin-embedder-policy': 'require-corp',
        'cross-origin-opener-policy': 'same-origin',
        'cross-origin-resource-policy': 'same-origin',
      }
    : {}

const exactClientFile = (pathname: string): string | null => {
  const relativePath = startUiAssetRelativePath(pathname)
  if (relativePath === null) return null
  const resolved = path.resolve(START_CLIENT_DIR, relativePath)
  return resolved === START_CLIENT_DIR ||
    resolved.startsWith(`${START_CLIENT_DIR}${path.sep}`)
    ? resolved
    : null
}

export const startUiAssetRelativePath = (pathname: string): string | null => {
  let decoded: string
  try {
    decoded = decodeURIComponent(pathname)
  } catch {
    return null
  }
  const relativePath =
    decoded === '/dh' || decoded === '/dh/'
      ? 'dh/index.html'
      : decoded === '/demo' || decoded === '/demo/'
        ? 'demo/index.html'
        : decoded === '/infra' || decoded === '/infra/'
          ? 'infra/index.html'
          : decoded.replace(/^[/\\]+/, '')
  return relativePath
}

const serveExactClientAsset = async (
  request: Request,
): Promise<Response | undefined> => {
  if (request.method !== 'GET' && request.method !== 'HEAD') return undefined
  const url = new URL(request.url)
  if (isDiamondHandsPath(url.pathname) && !diamondHandsDeploymentEnabled()) {
    return undefined
  }
  if (isMarketDemoPath(url.pathname) && !marketDemoDeploymentEnabled()) {
    return undefined
  }
  if (isInfraExplainerPath(url.pathname) && !infraExplainerDeploymentEnabled()) {
    return undefined
  }
  const filePath = exactClientFile(url.pathname)
  if (filePath === null) return undefined
  try {
    const info = await stat(filePath)
    if (!info.isFile()) return undefined
    const immutable = /\/assets\/[^/]+-[A-Za-z0-9_-]{8,}\.[a-z0-9]+$/.test(
      url.pathname,
    )
    return new Response(
      request.method === 'HEAD' ? null : await readFile(filePath),
      {
        headers: {
          'cache-control': immutable
            ? 'public, max-age=31536000, immutable'
            : 'public, max-age=60',
          'content-length': String(info.size),
          'content-type': startUiContentType(filePath),
          ...diamondHandsResponseHeaders(url.pathname),
          ...marketDemoResponseHeaders(url.pathname),
          ...infraExplainerResponseHeaders(url.pathname),
        },
      },
    )
  } catch {
    return undefined
  }
}

export const assertStartUiArtifactsExist = (): void => {
  const missingDocsArtifacts = REQUIRED_DOCS_CLIENT_ARTIFACTS.filter(
    relativePath => !existsSync(path.join(START_CLIENT_DIR, relativePath)),
  )
  if (
    !existsSync(START_CLIENT_DIR) ||
    !existsSync(START_SERVER_ENTRY) ||
    missingDocsArtifacts.length > 0
  ) {
    throw new Error(
      `Start UI artifacts missing (client=${START_CLIENT_DIR}, server=${START_SERVER_ENTRY}, docs=${missingDocsArtifacts.join(',') || 'ok'}). Run \`pnpm run build:start\` first.`,
    )
  }
}

export const isStartDocumentRequestPath = (
  pathname: string,
  allowPublicRoot = false,
): boolean =>
  isKnownStartDocumentPath(pathname) || (allowPublicRoot && pathname === '/')

export const isStartServerRequestPath = (
  pathname: string,
  allowPublicRoot = false,
): boolean =>
  isStartDocumentRequestPath(pathname, allowPublicRoot) ||
  START_SERVER_REQUEST_PATHS.has(pathname) ||
  FORGE_REPOSITORY_ASSET_REQUEST_PATH.test(pathname) ||
  START_SERVER_FUNCTION_REQUEST_PATH.test(pathname)

export const isStartServerRequest = (
  request: Request,
  allowPublicRoot = false,
): boolean => {
  const pathname = new URL(request.url).pathname
  const isReadRequest = request.method === 'GET' || request.method === 'HEAD'
  const isServerFunctionRequest =
    request.method === 'POST' &&
    START_SERVER_FUNCTION_REQUEST_PATH.test(pathname)
  return (
    (isReadRequest || isServerFunctionRequest) &&
    isStartServerRequestPath(pathname, allowPublicRoot)
  )
}

export const handleStartUiRequest = async (
  request: Request,
  env: Readonly<Record<string, unknown>>,
  ctx: ExecutionContext,
  allowPublicRoot = false,
): Promise<Response | undefined> => {
  const asset = await serveExactClientAsset(request)
  if (asset !== undefined) return asset

  if (!isStartServerRequest(request, allowPublicRoot)) return undefined

  return (await loadStartWorker()).fetch(request, env, ctx)
}
