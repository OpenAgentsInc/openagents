/**
 * CFG-9 (#8524) × CFG-5 (#8520): the `/api/sync/connect` WebSocket bridge
 * for the Cloud Run monolith.
 *
 * On workerd the connect route proxies the WebSocket upgrade to LiveHub via
 * plain `fetch` (the 101 carries `webSocket` back). Bun's `fetch` cannot
 * carry an upgrade, so the monolith bridges the socket itself:
 *
 * 1. AUTH REUSE, ZERO DUPLICATION: the inbound upgrade request is re-sent
 *    through the worker's own route WITH THE UPGRADE HEADERS STRIPPED. The
 *    route runs its full pre-upgrade pipeline (scope parse, `?token=`
 *    bearer promotion, actor auth, KS-7.1 scope-read gate, anonymous rate
 *    limit) and — only when EVERYTHING passed — answers the documented
 *    426 "requires a WebSocket upgrade" sentinel. Any other status is a
 *    real pre-upgrade denial and is returned to the client unchanged.
 * 2. On the 426 sentinel, Bun upgrades the client socket and bridges
 *    frames to the LiveHub service `/connect?scope&cursor` with the shared
 *    service bearer (same header discipline as
 *    `makeKhalaSyncLiveHubNamespace`: the service bearer always replaces
 *    any end-user bearer).
 */

export const SYNC_CONNECT_PATH = '/api/sync/connect'

export type SyncBridgeData = {
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

export const isSyncConnectUpgrade = (request: Request): boolean =>
  new URL(request.url).pathname === SYNC_CONNECT_PATH &&
  request.headers.get('upgrade')?.toLowerCase() === 'websocket'

/**
 * The same request without its upgrade negotiation headers, so the worker
 * route runs the full pre-upgrade pipeline and (on success) answers the
 * 426 sentinel instead of attempting the fetch-based proxy Bun lacks.
 */
export const withoutUpgradeHeaders = (request: Request): Request => {
  const headers = new Headers(request.headers)
  headers.delete('upgrade')
  headers.delete('connection')
  headers.delete('sec-websocket-key')
  headers.delete('sec-websocket-version')
  headers.delete('sec-websocket-protocol')
  headers.delete('sec-websocket-extensions')
  return new Request(request.url, { headers, method: request.method })
}

export type LiveHubTarget = Readonly<{ targetUrl: string; bearer: string }>

/** Build the LiveHub connect target for an authorized upgrade request. */
export const liveHubConnectTarget = (
  request: Request,
  config: Readonly<{ baseUrl: string; token: string }>,
): LiveHubTarget => {
  const inbound = new URL(request.url)
  const target = new URL(
    `${config.baseUrl.replace(/\/+$/, '')}/connect`,
  )
  const scope = inbound.searchParams.get('scope')
  if (scope !== null) target.searchParams.set('scope', scope)
  const cursor = inbound.searchParams.get('cursor')
  target.searchParams.set('cursor', cursor ?? '0')
  target.protocol = target.protocol === 'http:' ? 'ws:' : 'wss:'
  return { bearer: config.token, targetUrl: target.toString() }
}

/**
 * Bun `websocket` handler set implementing the client↔LiveHub frame bridge
 * (same shape as the aiur CFG-11 bridge).
 */
export const makeSyncBridgeWebSocketHandlers = () => ({
  // Khala Sync live-tail is quiet between events; rely on Cloud Run's
  // request timeout rather than a short idle cut.
  idleTimeout: 960 as const,
  open(ws: {
    data: SyncBridgeData
    send: (frame: string | Uint8Array) => void
    close: (code?: number, reason?: string) => void
  }) {
    const upstream = new WebSocket(ws.data.targetUrl, {
      // Bun extension: WebSocket client options accept `headers`.
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
          typeof data === 'string'
            ? data
            : new Uint8Array(data as ArrayBuffer),
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
  message(
    ws: { data: SyncBridgeData },
    message: string | Uint8Array | ArrayBuffer,
  ) {
    const upstream = ws.data.upstream
    const frame =
      typeof message === 'string'
        ? message
        : message instanceof Uint8Array
          ? message
          : new Uint8Array(message)
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
  close(ws: { data: SyncBridgeData }, code?: number, reason?: string) {
    ws.data.clientClosed = true
    const upstream = ws.data.upstream
    if (upstream === undefined) return
    try {
      upstream.close(sanitizeCloseCode(code), reason)
    } catch {
      // already closed
    }
  },
})
