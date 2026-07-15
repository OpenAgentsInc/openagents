/** Retained Start document adapter for the Cloud Run Worker monolith. */
import { existsSync } from 'node:fs'
import { readFile, stat } from 'node:fs/promises'
import path from 'node:path'
import { pathToFileURL } from 'node:url'

import { isKnownStartDocumentPath } from '../../../../apps/start/src/route-table'

const START_CLIENT_DIR = path.resolve(
  process.env['OPENAGENTS_START_CLIENT_DIR'] ??
    path.resolve(import.meta.dirname, '..', '..', '..', '..', 'apps/start/dist/client'),
)
const START_SERVER_ENTRY = path.resolve(
  process.env['OPENAGENTS_START_SERVER_ENTRY'] ??
    path.resolve(import.meta.dirname, '..', '..', '..', '..', 'apps/start/dist/server/server.js'),
)

type StartWorker = Readonly<{
  fetch: (
    request: Request,
    env: Readonly<Record<string, unknown>>,
    ctx: ExecutionContext,
  ) => Promise<Response>
}>

let startWorkerPromise: Promise<StartWorker> | undefined
const loadStartWorker = (): Promise<StartWorker> => {
  startWorkerPromise ??= import(pathToFileURL(START_SERVER_ENTRY).href).then(module => {
    const candidate = module.default as StartWorker | undefined
    if (candidate === undefined || typeof candidate.fetch !== 'function') {
      throw new Error(`Start server has no default fetch handler: ${START_SERVER_ENTRY}`)
    }
    return candidate
  })
  return startWorkerPromise
}

const contentType = (filePath: string): string => {
  switch (path.extname(filePath).toLowerCase()) {
    case '.css': return 'text/css; charset=utf-8'
    case '.html': return 'text/html; charset=utf-8'
    case '.js':
    case '.mjs': return 'text/javascript; charset=utf-8'
    case '.json': return 'application/json'
    case '.md': return 'text/markdown; charset=utf-8'
    case '.png': return 'image/png'
    case '.svg': return 'image/svg+xml'
    case '.txt': return 'text/plain; charset=utf-8'
    case '.webp': return 'image/webp'
    case '.woff': return 'font/woff'
    case '.woff2': return 'font/woff2'
    default: return 'application/octet-stream'
  }
}

const exactClientFile = (pathname: string): string | null => {
  let decoded: string
  try {
    decoded = decodeURIComponent(pathname)
  } catch {
    return null
  }
  const resolved = path.resolve(START_CLIENT_DIR, decoded.replace(/^[/\\]+/, ''))
  return resolved === START_CLIENT_DIR || resolved.startsWith(`${START_CLIENT_DIR}${path.sep}`)
    ? resolved
    : null
}

const serveExactClientAsset = async (request: Request): Promise<Response | undefined> => {
  if (request.method !== 'GET' && request.method !== 'HEAD') return undefined
  const url = new URL(request.url)
  const filePath = exactClientFile(url.pathname)
  if (filePath === null) return undefined
  try {
    const info = await stat(filePath)
    if (!info.isFile()) return undefined
    const immutable = /\/assets\/[^/]+-[A-Za-z0-9_-]{8,}\.[a-z0-9]+$/.test(url.pathname)
    return new Response(request.method === 'HEAD' ? null : await readFile(filePath), {
      headers: {
        'cache-control': immutable
          ? 'public, max-age=31536000, immutable'
          : 'public, max-age=60',
        'content-length': String(info.size),
        'content-type': contentType(filePath),
      },
    })
  } catch {
    return undefined
  }
}

export const assertStartUiArtifactsExist = (): void => {
  if (!existsSync(START_CLIENT_DIR) || !existsSync(START_SERVER_ENTRY)) {
    throw new Error(
      `Start UI artifacts missing (client=${START_CLIENT_DIR}, server=${START_SERVER_ENTRY}). Run \`pnpm run build:start\` first.`,
    )
  }
}

export const isStartDocumentRequestPath = (
  pathname: string,
  allowPublicRoot = false,
): boolean =>
  isKnownStartDocumentPath(pathname) || (allowPublicRoot && pathname === '/')

export const handleStartUiRequest = async (
  request: Request,
  env: Readonly<Record<string, unknown>>,
  ctx: ExecutionContext,
  allowPublicRoot = false,
): Promise<Response | undefined> => {
  const asset = await serveExactClientAsset(request)
  if (asset !== undefined) return asset

  const pathname = new URL(request.url).pathname
  if (
    (request.method !== 'GET' && request.method !== 'HEAD') ||
    !isStartDocumentRequestPath(pathname, allowPublicRoot)
  ) return undefined

  return (await loadStartWorker()).fetch(request, env, ctx)
}
