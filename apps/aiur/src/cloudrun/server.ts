/**
 * Aiur Cloud Run entrypoint (CFG-11, #8526).
 *
 * A thin Bun adapter that replaces the Cloudflare Worker runtime
 * (`src/server.ts`) with the exact same surface:
 *
 * - static assets from the Vite client build (`dist/client`),
 * - the SPA shell (prerendered by `vite.config.cloudrun.ts` SPA mode) for
 *   page routes — the shell re-checks `/api/aiur/access` client-side before
 *   rendering anything sensitive,
 * - the shared owner-gated surface (`shared-surface.ts`): OpenAuth
 *   sign-in/callback/logout, `/api/aiur/access`, the admin credits proxy,
 *   and the Khala Sync proxy — all FAIL CLOSED on `AIUR_OWNER_USER_IDS`,
 * - a Bun-native WebSocket bridge for `/api/sync/connect` (the Workers
 *   `WebSocketPair` bridge does not exist outside workerd).
 *
 * Config comes from plain env vars; `AIUR_OWNER_USER_IDS` is mounted from
 * GCP Secret Manager (`aiur-owner-user-ids`) by the deploy script.
 */

import { readFile } from 'node:fs/promises'
import path from 'node:path'

import type { AiurEnv } from '../auth/config'
import {
  AIUR_SYNC_CONNECT_PATH,
  resolveAiurSyncConnectTarget,
} from '../khala-sync-proxy'
import {
  applySecurityHeaders,
  routeAiurSharedSurfaceRequest,
} from '../shared-surface'
import { aiurEnvFromProcessEnv } from './env'
import { staticAssetResponse } from './static'

const noStoreJson = (body: unknown, status: number): Response =>
  new Response(JSON.stringify(body), {
    status,
    headers: {
      'content-type': 'application/json; charset=utf-8',
      'cache-control': 'no-store',
    },
  })

/**
 * Cloud Run terminates TLS at its front proxy, so the Bun server receives
 * plain-HTTP request URLs. Origin-derived values (the OpenAuth
 * `redirect_uri`, cookie security) must use the public `https://` origin,
 * which the proxy reports via `X-Forwarded-Proto`.
 */
export const withForwardedProto = (request: Request): Request => {
  if (request.headers.get('x-forwarded-proto') !== 'https') {
    return request
  }

  const url = new URL(request.url)
  if (url.protocol === 'https:') {
    return request
  }

  url.protocol = 'https:'
  return new Request(url, request)
}

export type AiurCloudRunHandlerOptions = Readonly<{
  env: AiurEnv
  clientDir: string
  shellHtml: string
}>

/**
 * The non-WebSocket request handler. Pure w.r.t. its options — unit tests
 * drive it with a fake env/client dir and no Bun runtime.
 */
export const createAiurCloudRunFetchHandler = (
  options: AiurCloudRunHandlerOptions,
): ((request: Request) => Promise<Response>) => {
  return async (incomingRequest: Request): Promise<Response> => {
    const request = withForwardedProto(incomingRequest)
    const sharedSurfaceResponse = await routeAiurSharedSurfaceRequest(
      request,
      options.env,
    )
    if (sharedSurfaceResponse !== undefined) {
      return applySecurityHeaders(sharedSurfaceResponse)
    }

    const pathname = new URL(request.url).pathname

    if (pathname.startsWith('/api/')) {
      return applySecurityHeaders(
        noStoreJson(
          { code: 'not_found', messageSafe: 'Unknown API route.' },
          404,
        ),
      )
    }

    const asset = await staticAssetResponse(request, options.clientDir)
    if (asset !== undefined) {
      return applySecurityHeaders(asset)
    }

    if (request.method !== 'GET' && request.method !== 'HEAD') {
      return applySecurityHeaders(
        noStoreJson(
          { code: 'invalid_request', messageSafe: 'GET required.' },
          405,
        ),
      )
    }

    // SPA shell fallback for every page route. The shell renders the
    // sign-in / access-denied / dashboard states from `/api/aiur/access`,
    // and every data route independently re-checks the owner gate.
    return applySecurityHeaders(
      new Response(request.method === 'HEAD' ? null : options.shellHtml, {
        status: 200,
        headers: {
          'content-type': 'text/html; charset=utf-8',
          'cache-control': 'no-cache',
        },
      }),
    )
  }
}

export const readShellHtml = async (clientDir: string): Promise<string> => {
  // TanStack Start SPA mode prerenders the shell at `/_shell`; depending on
  // `autoSubfolderIndex` that lands as `_shell/index.html` or `_shell.html`.
  const candidates = [
    path.join(clientDir, '_shell', 'index.html'),
    path.join(clientDir, '_shell.html'),
    path.join(clientDir, 'index.html'),
  ]

  for (const candidate of candidates) {
    try {
      return await readFile(candidate, 'utf8')
    } catch {
      // try next candidate
    }
  }

  throw new Error(
    `Aiur Cloud Run server: no SPA shell found under ${clientDir}. ` +
      'Run `bun run build:cloudrun` first.',
  )
}

type SyncBridgeData = {
  targetUrl: string
  bearer: string
  upstream: WebSocket | undefined
  /** Client frames buffered until the upstream socket opens. */
  pending: Array<string | Uint8Array>
  clientClosed: boolean
}

const SAFE_CLOSE_MIN = 1000
const SAFE_CLOSE_MAX = 4999
const RESERVED_CLOSE_CODES = new Set([1004, 1005, 1006, 1015])

export const sanitizeCloseCode = (code: number | undefined): number =>
  code !== undefined &&
  code >= SAFE_CLOSE_MIN &&
  code <= SAFE_CLOSE_MAX &&
  !RESERVED_CLOSE_CODES.has(code)
    ? code
    : 1000

const serveAiurCloudRun = async (): Promise<void> => {
  const appRoot = path.resolve(import.meta.dir, '..', '..')
  const clientDir = path.resolve(
    process.env['AIUR_CLIENT_DIST'] ?? path.join(appRoot, 'dist', 'client'),
  )
  const env = aiurEnvFromProcessEnv(process.env)
  const shellHtml = await readShellHtml(clientDir)
  const handler = createAiurCloudRunFetchHandler({ env, clientDir, shellHtml })
  const port = Number(process.env['PORT'] ?? 8080)

  const server = Bun.serve<SyncBridgeData>({
    port,
    hostname: '0.0.0.0',
    async fetch(request, bunServer) {
      const url = new URL(request.url)

      if (
        url.pathname === AIUR_SYNC_CONNECT_PATH &&
        request.headers.get('upgrade')?.toLowerCase() === 'websocket'
      ) {
        // Owner gate FIRST (fail closed), before any upgrade happens.
        // Note: the gate reads the proto-corrected request, but the
        // `bunServer.upgrade` below must receive the ORIGINAL request.
        const target = await resolveAiurSyncConnectTarget(
          withForwardedProto(request),
          env,
        )
        if (target.kind === 'response') {
          return applySecurityHeaders(target.response)
        }

        const upgraded = bunServer.upgrade(request, {
          data: {
            targetUrl: target.targetUrl,
            bearer: target.bearer,
            upstream: undefined,
            pending: [],
            clientClosed: false,
          },
        })

        if (upgraded) {
          // Bun handles the 101 response itself.
          return undefined as unknown as Response
        }

        return applySecurityHeaders(
          noStoreJson(
            {
              code: 'internal',
              messageSafe: 'Khala Sync live-tail upgrade failed unexpectedly; reconnect.',
            },
            500,
          ),
        )
      }

      return handler(request)
    },
    websocket: {
      // Khala Sync live-tail is quiet between events; rely on Cloud Run's
      // request timeout (set at deploy time) rather than a short idle cut.
      idleTimeout: 960,
      open(ws) {
        // Bun extension: WebSocket client options accept `headers`, which
        // browsers do not — exactly what the Worker relied on workerd
        // `fetch` upgrades for.
        const upstream = new WebSocket(ws.data.targetUrl, {
          headers: { authorization: `Bearer ${ws.data.bearer}` },
        } as unknown as string[])
        upstream.binaryType = 'arraybuffer'
        ws.data.upstream = upstream

        upstream.addEventListener('open', () => {
          if (ws.data.clientClosed) {
            try {
              upstream.close(1000)
            } catch {
              // already closed
            }
            return
          }
          for (const frame of ws.data.pending.splice(0)) {
            try {
              upstream.send(frame)
            } catch {
              // upstream just closed
            }
          }
        })
        upstream.addEventListener('message', event => {
          try {
            const data = (event as MessageEvent).data
            ws.send(
              typeof data === 'string' ? data : new Uint8Array(data as ArrayBuffer),
            )
          } catch {
            // client already closed
          }
        })
        upstream.addEventListener('close', event => {
          try {
            ws.close(
              sanitizeCloseCode((event as CloseEvent).code),
              (event as CloseEvent).reason,
            )
          } catch {
            // already closed
          }
        })
        upstream.addEventListener('error', () => {
          try {
            ws.close(1011, 'upstream error')
          } catch {
            // already closed
          }
        })
      },
      message(ws, message) {
        const upstream = ws.data.upstream
        const frame =
          typeof message === 'string' ? message : new Uint8Array(message)
        if (upstream !== undefined && upstream.readyState === WebSocket.OPEN) {
          try {
            upstream.send(frame)
          } catch {
            // upstream just closed
          }
          return
        }
        ws.data.pending.push(frame)
      },
      close(ws, code, reason) {
        ws.data.clientClosed = true
        const upstream = ws.data.upstream
        if (upstream === undefined) return
        try {
          upstream.close(sanitizeCloseCode(code), reason)
        } catch {
          // already closed
        }
      },
    },
  })

  console.log(`aiur cloudrun server listening on :${server.port}`)
}

if (import.meta.main) {
  await serveAiurCloudRun()
}
